//! Built-in HTTP forward proxy for the runtime VM.
//!
//! When VZ.framework NAT lacks outbound connectivity (missing
//! `com.apple.vm.networking` entitlement), this proxy bridges the gap:
//! it binds on the host bridge interface (192.168.64.1:3128) so the
//! guest can route HTTP/HTTPS traffic through the host's network stack.
//!
//! Supports two request modes:
//! - **CONNECT tunnel** — for HTTPS (e.g. `CONNECT host:443 HTTP/1.1`)
//! - **Plain HTTP forward** — for HTTP (e.g. `GET http://host/path HTTP/1.1`)

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Default port for the built-in HTTP forward proxy.
const DEFAULT_PROXY_PORT: u16 = 3128;

/// Timeout for establishing an outbound connection to the target.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Timeout for idle read/write on a proxied connection.
const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum size of the initial request line + headers we buffer.
const MAX_HEADER_SIZE: usize = 8192;

/// Start the built-in HTTP forward proxy in the background.
///
/// Spawns a **dedicated OS thread** with its own tokio runtime so the proxy
/// survives even after the caller's async runtime is dropped (e.g. when the
/// CLI `main()` returns while the VZ runner keeps running in the background).
///
/// Returns the address it is listening on.
pub async fn start_builtin_proxy(bind_addr: Option<SocketAddr>) -> Result<SocketAddr, String> {
    let addr =
        bind_addr.unwrap_or_else(|| SocketAddr::from(([192, 168, 64, 1], DEFAULT_PROXY_PORT)));

    // Bind on the current runtime first so we can report the actual address.
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind proxy on {}: {}", addr, e))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local addr: {}", e))?;

    tracing::info!("Built-in HTTP proxy listening on {}", local_addr);

    // Transfer the listener's std handle to the background thread.
    let std_listener = listener
        .into_std()
        .map_err(|e| format!("Failed to convert listener: {}", e))?;

    std::thread::Builder::new()
        .name("builtin-proxy".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create proxy tokio runtime: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                // Re-wrap the std listener into a tokio listener on this runtime.
                std_listener.set_nonblocking(true).ok();
                let listener = match TcpListener::from_std(std_listener) {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("Failed to re-wrap listener: {}", e);
                        return;
                    }
                };
                loop {
                    match listener.accept().await {
                        Ok((stream, peer)) => {
                            tokio::spawn(handle_proxy_connection(stream, peer));
                        }
                        Err(e) => {
                            tracing::warn!("Proxy accept error: {}", e);
                        }
                    }
                }
            });
        })
        .map_err(|e| format!("Failed to spawn proxy thread: {}", e))?;

    Ok(local_addr)
}

/// Handle a single proxied connection.
///
/// Reads the first request line to determine the method, then dispatches
/// to either CONNECT tunnel or plain HTTP forwarding.
async fn handle_proxy_connection(mut client: TcpStream, peer: SocketAddr) {
    tracing::debug!("Proxy connection from {}", peer);

    // Read enough to parse the first request line and headers.
    let mut buf = vec![0u8; MAX_HEADER_SIZE];
    let mut filled = 0usize;

    let header_end = loop {
        if filled >= MAX_HEADER_SIZE {
            tracing::debug!("Proxy: header too large from {}", peer);
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        }

        let n = match tokio::time::timeout(IO_TIMEOUT, client.read(&mut buf[filled..])).await {
            Ok(Ok(0)) | Err(_) => {
                tracing::debug!(
                    "Proxy: client {} disconnected or timed out during header read",
                    peer
                );
                return;
            }
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                tracing::debug!("Proxy: read error from {}: {}", peer, e);
                return;
            }
        };
        filled += n;

        // Look for end of headers (\r\n\r\n).
        if let Some(pos) = find_header_end(&buf[..filled]) {
            break pos;
        }
    };

    let header_bytes = &buf[..header_end];
    let header_str = match std::str::from_utf8(header_bytes) {
        Ok(s) => s,
        Err(_) => {
            tracing::debug!("Proxy: non-UTF8 header from {}", peer);
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        }
    };

    // Parse the request line (first line).
    let request_line = match header_str.lines().next() {
        Some(line) => line,
        None => {
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        }
    };

    let parts: Vec<&str> = request_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        tracing::debug!(
            "Proxy: malformed request line from {}: {:?}",
            peer,
            request_line
        );
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        return;
    }

    let method = parts[0];
    let target = parts[1];

    if method.eq_ignore_ascii_case("CONNECT") {
        handle_connect(client, peer, target).await;
    } else {
        // Remaining buffered data past the header (could be body of a POST, etc.)
        let remaining = &buf[header_end..filled];
        handle_plain_http(client, peer, method, target, header_bytes, remaining).await;
    }
}

/// Handle a CONNECT tunnel request.
///
/// Connects to the target, sends `200 Connection established`, then
/// bidirectionally copies data between client and target.
async fn handle_connect(mut client: TcpStream, peer: SocketAddr, target: &str) {
    tracing::debug!("Proxy CONNECT {} from {}", target, peer);

    let addr = match resolve_target(target, 443) {
        Some(a) => a,
        None => {
            tracing::debug!("Proxy: bad CONNECT target {:?} from {}", target, peer);
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        }
    };

    let upstream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::debug!("Proxy: failed to connect to {} for {}: {}", addr, peer, e);
            let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
            return;
        }
        Err(_) => {
            tracing::debug!("Proxy: connect timeout to {} for {}", addr, peer);
            let _ = client
                .write_all(b"HTTP/1.1 504 Gateway Timeout\r\n\r\n")
                .await;
            return;
        }
    };

    // Tell the client the tunnel is established.
    if let Err(e) = client
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await
    {
        tracing::debug!("Proxy: failed to send 200 to {}: {}", peer, e);
        return;
    }

    bidirectional_copy(client, upstream, peer).await;
}

/// Handle a plain HTTP forwarding request (non-CONNECT).
///
/// Parses the absolute URI to extract host and port, connects to the
/// target, rewrites the request line to a relative path, and forwards
/// the full request+response.
async fn handle_plain_http(
    mut client: TcpStream,
    peer: SocketAddr,
    method: &str,
    target: &str,
    header_bytes: &[u8],
    remaining: &[u8],
) {
    tracing::debug!("Proxy {} {} from {}", method, target, peer);

    // Parse absolute URI: http://host[:port]/path
    let (host_port, path) = match parse_absolute_uri(target) {
        Some(v) => v,
        None => {
            tracing::debug!("Proxy: bad URI {:?} from {}", target, peer);
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        }
    };

    let addr = match resolve_target(&host_port, 80) {
        Some(a) => a,
        None => {
            tracing::debug!("Proxy: bad host {:?} from {}", host_port, peer);
            let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
            return;
        }
    };

    let mut upstream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::debug!("Proxy: failed to connect to {} for {}: {}", addr, peer, e);
            let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
            return;
        }
        Err(_) => {
            tracing::debug!("Proxy: connect timeout to {} for {}", addr, peer);
            let _ = client
                .write_all(b"HTTP/1.1 504 Gateway Timeout\r\n\r\n")
                .await;
            return;
        }
    };

    // Rewrite the request line from absolute to relative URI and forward headers.
    let header_str = match std::str::from_utf8(header_bytes) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut lines = header_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    // Build new request line: METHOD /path HTTP/1.1
    let version = request_line.splitn(3, ' ').nth(2).unwrap_or("HTTP/1.1");
    let new_request_line = format!("{} {} {}\r\n", method, path, version);

    let mut forwarded_header = new_request_line;
    for line in lines {
        // Skip proxy-specific headers
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("proxy-connection:") || lower.starts_with("proxy-authorization:") {
            continue;
        }
        forwarded_header.push_str(line);
        forwarded_header.push_str("\r\n");
    }
    forwarded_header.push_str("\r\n");

    // Send rewritten headers to upstream.
    if let Err(e) = upstream.write_all(forwarded_header.as_bytes()).await {
        tracing::debug!("Proxy: upstream write error for {}: {}", peer, e);
        let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
        return;
    }

    // Send any remaining body data that was buffered.
    if !remaining.is_empty() {
        if let Err(e) = upstream.write_all(remaining).await {
            tracing::debug!("Proxy: upstream body write error for {}: {}", peer, e);
            return;
        }
    }

    // Now bidirectionally copy the rest.
    bidirectional_copy(client, upstream, peer).await;
}

/// Bidirectionally copy data between two TCP streams until one side closes
/// or a timeout occurs.
async fn bidirectional_copy(client: TcpStream, upstream: TcpStream, peer: SocketAddr) {
    let (mut cr, mut cw) = tokio::io::split(client);
    let (mut ur, mut uw) = tokio::io::split(upstream);

    let c2u = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = match tokio::time::timeout(IO_TIMEOUT, cr.read(&mut buf)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => n,
                Ok(Err(_)) => break,
            };
            if uw.write_all(&buf[..n]).await.is_err() {
                break;
            }
        }
        let _ = uw.shutdown().await;
    };

    let u2c = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = match tokio::time::timeout(IO_TIMEOUT, ur.read(&mut buf)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => n,
                Ok(Err(_)) => break,
            };
            if cw.write_all(&buf[..n]).await.is_err() {
                break;
            }
        }
        let _ = cw.shutdown().await;
    };

    tokio::join!(c2u, u2c);
    tracing::debug!("Proxy: connection from {} closed", peer);
}

/// Find the byte offset right after `\r\n\r\n` in the buffer.
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Parse an absolute URI like `http://host[:port]/path` into `(host:port, /path)`.
///
/// Returns `None` if the URI is not a valid absolute HTTP URI.
fn parse_absolute_uri(uri: &str) -> Option<(String, String)> {
    let rest = uri.strip_prefix("http://")?;
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    if host_port.is_empty() {
        return None;
    }
    Some((host_port.to_string(), path.to_string()))
}

/// Resolve a `host:port` string, applying `default_port` when the port is
/// missing.  Returns a string suitable for `TcpStream::connect`.
fn resolve_target(target: &str, default_port: u16) -> Option<String> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }

    // IPv6 bracket notation: [::1]:port
    if target.starts_with('[') {
        // Must have closing bracket
        let end = target.find(']')?;
        let host = &target[1..end];
        if host.is_empty() {
            return None;
        }
        let rest = &target[end + 1..];
        let port = if rest.is_empty() {
            default_port
        } else {
            rest.strip_prefix(':')?.parse::<u16>().ok()?
        };
        return Some(format!("[{}]:{}", host, port));
    }

    // If it already contains a colon and looks like host:port
    if let Some((host, port_str)) = target.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            if !host.is_empty() {
                return Some(format!("{}:{}", host, port));
            }
        }
    }

    // No port specified — use default
    Some(format!("{}:{}", target, default_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_header_end_finds_crlf_crlf() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\nbody";
        let end = find_header_end(data).unwrap();
        assert_eq!(&data[end..], b"body");
    }

    #[test]
    fn find_header_end_returns_none_when_incomplete() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n";
        assert!(find_header_end(data).is_none());
    }

    #[test]
    fn parse_absolute_uri_with_path() {
        let (host, path) = parse_absolute_uri("http://example.com/foo/bar").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(path, "/foo/bar");
    }

    #[test]
    fn parse_absolute_uri_with_port() {
        let (host, path) = parse_absolute_uri("http://example.com:8080/test").unwrap();
        assert_eq!(host, "example.com:8080");
        assert_eq!(path, "/test");
    }

    #[test]
    fn parse_absolute_uri_no_path() {
        let (host, path) = parse_absolute_uri("http://example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(path, "/");
    }

    #[test]
    fn parse_absolute_uri_rejects_non_http() {
        assert!(parse_absolute_uri("https://example.com").is_none());
        assert!(parse_absolute_uri("ftp://example.com").is_none());
    }

    #[test]
    fn parse_absolute_uri_rejects_empty_host() {
        assert!(parse_absolute_uri("http:///path").is_none());
    }

    #[test]
    fn resolve_target_host_port() {
        assert_eq!(
            resolve_target("example.com:3128", 80),
            Some("example.com:3128".to_string())
        );
    }

    #[test]
    fn resolve_target_host_only() {
        assert_eq!(
            resolve_target("example.com", 80),
            Some("example.com:80".to_string())
        );
    }

    #[test]
    fn resolve_target_ipv6() {
        assert_eq!(
            resolve_target("[::1]:8080", 80),
            Some("[::1]:8080".to_string())
        );
    }

    #[test]
    fn resolve_target_ipv6_default_port() {
        assert_eq!(resolve_target("[::1]", 443), Some("[::1]:443".to_string()));
    }

    #[test]
    fn resolve_target_empty() {
        assert!(resolve_target("", 80).is_none());
    }

    #[tokio::test]
    async fn start_builtin_proxy_binds_localhost() {
        // Bind to localhost to avoid permission issues in tests.
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let result = start_builtin_proxy(Some(addr)).await;
        assert!(result.is_ok());
        let bound = result.unwrap();
        assert_eq!(bound.ip(), std::net::Ipv4Addr::LOCALHOST);
        assert_ne!(bound.port(), 0);
    }
}
