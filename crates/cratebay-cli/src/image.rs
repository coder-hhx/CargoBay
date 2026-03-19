async fn handle_image(cmd: ImageCommands) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "CrateBay/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/coder-hhx/CrateBay)"
        ))
        .build()
        .map_err(|e| e.to_string())?;

    match cmd {
        ImageCommands::Search {
            query,
            source,
            limit,
        } => {
            if let Some((registry, repo)) = parse_registry_reference(&query) {
                let tags = list_registry_tags(&client, &registry, &repo, limit).await?;
                if tags.is_empty() {
                    println!("No tags found for {}/{}.", registry, repo);
                    return Ok(());
                }
                println!("Tags for {}/{}:", registry, repo);
                for tag in tags {
                    println!("{}/{}:{}", registry, repo, tag);
                }
                return Ok(());
            }

            let src = source.to_ascii_lowercase();
            let mut items: Vec<ImageSearchItem> = Vec::new();
            let mut did_any = false;

            if matches!(src.as_str(), "all" | "dockerhub" | "hub" | "docker") {
                did_any = true;
                items.extend(search_dockerhub(&client, &query, limit).await?);
            }
            if matches!(src.as_str(), "all" | "quay") {
                did_any = true;
                items.extend(search_quay(&client, &query, limit).await?);
            }

            if !did_any {
                return Err(format!("Unknown source: {}", source));
            }

            if items.is_empty() {
                println!("No results.");
                return Ok(());
            }

            print_image_search_results(&items);
            Ok(())
        }
        ImageCommands::Tags { reference, limit } => {
            let Some((registry, repo)) = parse_registry_reference(&reference) else {
                return Err("Invalid reference. Expected e.g. ghcr.io/org/image".into());
            };

            let tags = list_registry_tags(&client, &registry, &repo, limit).await?;
            if tags.is_empty() {
                println!("No tags found for {}/{}.", registry, repo);
                return Ok(());
            }
            println!("Tags for {}/{}:", registry, repo);
            for tag in tags {
                println!("{}/{}:{}", registry, repo, tag);
            }
            Ok(())
        }
        ImageCommands::List => {
            let docker = connect_docker()?;
            let opts = ListImagesOptions::<String> {
                all: false,
                ..Default::default()
            };
            let images = docker
                .list_images(Some(opts))
                .await
                .map_err(|e| e.to_string())?;

            if images.is_empty() {
                println!("No local images found.");
                return Ok(());
            }

            println!(
                "{:<40} {:<14} {:<12} CREATED",
                "REPOSITORY:TAG", "IMAGE ID", "SIZE"
            );
            for img in images {
                let full_id = img.id.clone();
                let short_id = if let Some(stripped) = full_id.strip_prefix("sha256:") {
                    stripped.chars().take(12).collect::<String>()
                } else {
                    full_id.chars().take(12).collect::<String>()
                };
                let size = img.size.max(0) as u64;
                let size_str = format_bytes(size);
                let created = {
                    let ts = img.created;
                    if ts > 0 {
                        // Simple UTC timestamp formatting without chrono
                        let secs_per_min = 60i64;
                        let secs_per_hour = 3600i64;
                        let secs_per_day = 86400i64;
                        let days_since_epoch = ts / secs_per_day;
                        let time_of_day = ts % secs_per_day;
                        let hours = time_of_day / secs_per_hour;
                        let minutes = (time_of_day % secs_per_hour) / secs_per_min;

                        // Simple days-since-epoch to Y-M-D (good enough for display)
                        let mut y = 1970i64;
                        let mut remaining = days_since_epoch;
                        loop {
                            let days_in_year = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                                366
                            } else {
                                365
                            };
                            if remaining < days_in_year {
                                break;
                            }
                            remaining -= days_in_year;
                            y += 1;
                        }
                        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
                        let month_days = [
                            31,
                            if leap { 29 } else { 28 },
                            31,
                            30,
                            31,
                            30,
                            31,
                            31,
                            30,
                            31,
                            30,
                            31,
                        ];
                        let mut m = 0usize;
                        for &md in &month_days {
                            if remaining < md {
                                break;
                            }
                            remaining -= md;
                            m += 1;
                        }
                        format!(
                            "{:04}-{:02}-{:02} {:02}:{:02}",
                            y,
                            m + 1,
                            remaining + 1,
                            hours,
                            minutes
                        )
                    } else {
                        "-".to_string()
                    }
                };

                if img.repo_tags.is_empty() {
                    println!(
                        "{:<40} {:<14} {:<12} {}",
                        "<none>:<none>", short_id, size_str, created
                    );
                } else {
                    for tag in &img.repo_tags {
                        println!("{:<40} {:<14} {:<12} {}", tag, short_id, size_str, created);
                    }
                }
            }
            Ok(())
        }
        ImageCommands::Remove { reference } => {
            let docker = connect_docker()?;
            let opts = RemoveImageOptions {
                force: false,
                noprune: false,
            };
            let results = docker
                .remove_image(&reference, Some(opts), None)
                .await
                .map_err(|e| e.to_string())?;
            for info in results {
                if let Some(deleted) = info.deleted {
                    println!("Deleted: {}", deleted);
                }
                if let Some(untagged) = info.untagged {
                    println!("Untagged: {}", untagged);
                }
            }
            Ok(())
        }
        ImageCommands::Tag { source, target } => {
            let docker = connect_docker()?;
            let (repo, tag) = if let Some(pos) = target.rfind(':') {
                (&target[..pos], &target[pos + 1..])
            } else {
                (target.as_str(), "latest")
            };
            let opts = TagImageOptions { repo, tag };
            docker
                .tag_image(&source, Some(opts))
                .await
                .map_err(|e| e.to_string())?;
            println!("Tagged {} as {}", source, target);
            Ok(())
        }
        ImageCommands::Inspect { reference } => {
            let docker = connect_docker()?;
            let detail = docker
                .inspect_image(&reference)
                .await
                .map_err(|e| e.to_string())?;
            let json = serde_json::to_string_pretty(&detail).map_err(|e| e.to_string())?;
            println!("{}", json);
            Ok(())
        }
        ImageCommands::Load { path } => {
            let docker = connect_docker()?;
            let archive = PathBuf::from(&path);
            let file = tokio::fs::File::open(&archive)
                .await
                .map_err(|e| format!("Failed to open {}: {}", archive.display(), e))?;

            let read_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            let read_error_capture = read_error.clone();
            let archive_display = archive.display().to_string();

            let byte_stream = FramedRead::new(file, BytesCodec::new()).filter_map(move |result| {
                let read_error_capture = read_error_capture.clone();
                let archive_display = archive_display.clone();
                async move {
                    match result {
                        Ok(buf) => Some(buf.freeze()),
                        Err(e) => {
                            let mut guard =
                                read_error_capture.lock().unwrap_or_else(|e| e.into_inner());
                            *guard = Some(format!(
                                "Failed to read image archive {}: {}",
                                archive_display, e
                            ));
                            None
                        }
                    }
                }
            });

            let mut stream =
                docker.import_image_stream(ImportImageOptions::default(), byte_stream, None);
            let mut out = String::new();
            while let Some(progress) = stream.try_next().await.map_err(format_bollard_error)? {
                if let Some(error) = progress.error {
                    return Err(error);
                }
                if let Some(line) = progress.stream {
                    out.push_str(&line);
                    continue;
                }
                if let Some(status) = progress.status {
                    if let Some(p) = progress.progress {
                        out.push_str(&format!("{} {}\n", status, p));
                    } else {
                        out.push_str(&format!("{}\n", status));
                    }
                }
            }

            if let Some(e) = read_error.lock().unwrap_or_else(|e| e.into_inner()).take() {
                return Err(e);
            }

            let trimmed = out.trim();
            if trimmed.is_empty() {
                println!("Done.");
            } else {
                println!("{}", trimmed);
            }
            Ok(())
        }
        ImageCommands::Push { reference } => {
            validation::validate_image_reference(&reference)?;
            let docker = connect_docker()?;

            let (repo, tag) = split_image_reference(&reference);
            let auth = cratebay_core::docker_auth::resolve_registry_auth_for_image(&reference)?;
            let creds = auth.map(|a| DockerCredentials {
                username: a.username,
                password: a.password,
                serveraddress: Some(a.server_address),
                identitytoken: a.identity_token,
                ..Default::default()
            });

            let mut stream = docker.push_image(&repo, Some(PushImageOptions { tag }), creds);
            let mut out = String::new();
            while let Some(progress) = stream.try_next().await.map_err(format_bollard_error)? {
                if let Some(status) = progress.status {
                    if let Some(p) = progress.progress {
                        out.push_str(&format!("{} {}\n", status, p));
                    } else {
                        out.push_str(&format!("{}\n", status));
                    }
                }
            }

            let trimmed = out.trim();
            if trimmed.is_empty() {
                println!("Done.");
            } else {
                println!("{}", trimmed);
            }
            Ok(())
        }
        ImageCommands::PackContainer { container, tag } => {
            let docker = connect_docker()?;
            let (repo, image_tag) = split_image_reference(&tag);
            let opts = CommitContainerOptions {
                container: container.as_str(),
                repo: repo.as_str(),
                tag: image_tag.as_str(),
                pause: true,
                ..Default::default()
            };
            let result = docker
                .commit_container(opts, Config::<String>::default())
                .await
                .map_err(|e| e.to_string())?;

            let id = result
                .id
                .clone()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| result.expected.clone().filter(|v| !v.trim().is_empty()));
            if let Some(id) = id {
                println!("{}", id);
            } else {
                let json = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
                println!("{}", json);
            }
            Ok(())
        }
        ImageCommands::ListOs => {
            let images = cratebay_core::images::list_available_images();
            if images.is_empty() {
                println!("No OS images in catalog.");
                return Ok(());
            }
            println!(
                "{:<16} {:<28} {:<10} {:<10} STATUS",
                "ID", "NAME", "VERSION", "SIZE"
            );
            for img in images {
                let size_str = format_bytes(img.size_bytes);
                let status = match img.status {
                    cratebay_core::images::ImageStatus::NotDownloaded => "not downloaded",
                    cratebay_core::images::ImageStatus::Downloading => "downloading...",
                    cratebay_core::images::ImageStatus::Ready => "ready",
                };
                println!(
                    "{:<16} {:<28} {:<10} {:<10} {}",
                    img.id, img.name, img.version, size_str, status
                );
            }
            Ok(())
        }
        ImageCommands::DownloadOs { name } => {
            let entry = cratebay_core::images::find_image(&name);
            if entry.is_none() {
                return Err(format!(
                    "Unknown OS image: '{}'. Run 'cratebay image list-os' to see available images.",
                    name
                ));
            }

            println!("Downloading OS image '{}'...", name);
            cratebay_core::images::download_image(&name, move |file, downloaded, total| {
                if total > 0 {
                    let pct = (downloaded as f64 / total as f64 * 100.0).min(100.0);
                    eprint!(
                        "\r  [{}] {}/{} ({:.1}%)    ",
                        file,
                        format_bytes(downloaded),
                        format_bytes(total),
                        pct
                    );
                }
            })
            .await
            .map_err(|e| e.to_string())?;

            eprintln!();
            println!("OS image '{}' downloaded successfully.", name);
            let paths = cratebay_core::images::image_paths(&name);
            println!("  Kernel:  {}", paths.kernel_path.display());
            println!("  Initrd:  {}", paths.initrd_path.display());
            println!("  Rootfs:  {}", paths.rootfs_path.display());
            Ok(())
        }
        ImageCommands::DeleteOs { name } => {
            cratebay_core::images::delete_image(&name).map_err(|e| e.to_string())?;
            println!("Deleted OS image '{}'.", name);
            Ok(())
        }
    }
}

async fn search_dockerhub(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<ImageSearchItem>, String> {
    // Docker Hub search endpoints can be flaky / rate-limited / protected.
    // Try the modern Hub API first, but fall back to the legacy endpoint used
    // by `docker search` if we can't decode the response.
    match search_dockerhub_v2(client, query, limit).await {
        Ok(items) => Ok(items),
        Err(v2_err) => match search_dockerhub_v1(client, query, limit).await {
            Ok(items) => Ok(items),
            Err(v1_err) => Err(format!(
                "Docker Hub search failed.\n- v2: {}\n- v1: {}",
                v2_err, v1_err
            )),
        },
    }
}

async fn search_dockerhub_v2(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<ImageSearchItem>, String> {
    let mut url = reqwest::Url::parse("https://hub.docker.com/v2/search/repositories/")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("page_size", &limit.to_string());

    let resp = client
        .get(url)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    let body_text = String::from_utf8_lossy(&body);
    if !status.is_success() {
        return Err(format!(
            "v2 HTTP {} (body: {})",
            status,
            truncate_str(body_text.trim(), 200)
        ));
    }

    let data: DockerHubSearchResponse = serde_json::from_slice(&body).map_err(|e| {
        let snippet = body_text.trim().chars().take(400).collect::<String>();
        format!(
            "Docker Hub search returned unexpected JSON (HTTP {}): {}. Body: {}",
            status, e, snippet
        )
    })?;

    let mut out = Vec::new();
    for r in data.results.into_iter().take(limit) {
        let repo_name = r.name.trim().to_string();
        let name = if repo_name.contains('/') {
            repo_name
        } else {
            let ns = r.namespace.unwrap_or_default();
            let ns = ns.trim();
            if ns.is_empty() || ns == "library" {
                repo_name
            } else {
                format!("{}/{}", ns, repo_name)
            }
        };

        out.push(ImageSearchItem {
            source: "dockerhub",
            reference: name,
            description: r.description.unwrap_or_default(),
            stars: r.star_count,
            pulls: r.pull_count,
            official: r.is_official.unwrap_or(false),
        });
    }
    Ok(out)
}

async fn search_dockerhub_v1(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<ImageSearchItem>, String> {
    let mut url =
        reqwest::Url::parse("https://index.docker.io/v1/search").map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("n", &limit.to_string());

    let resp = client
        .get(url)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "v1 HTTP {} (body: {})",
            status,
            truncate_str(body.trim(), 200)
        ));
    }

    let data: DockerHubV1SearchResponse = serde_json::from_str(&body).map_err(|e| {
        format!(
            "v1 invalid JSON: {} (body: {})",
            e,
            truncate_str(body.trim(), 200)
        )
    })?;

    let mut out = Vec::new();
    for r in data.results.into_iter().take(limit) {
        out.push(ImageSearchItem {
            source: "dockerhub",
            reference: r.name,
            description: r.description.unwrap_or_default(),
            stars: r.star_count,
            pulls: None,
            official: r.is_official.unwrap_or(false),
        });
    }
    Ok(out)
}

async fn search_quay(
    client: &reqwest::Client,
    query: &str,
    limit: usize,
) -> Result<Vec<ImageSearchItem>, String> {
    let mut url = reqwest::Url::parse("https://quay.io/api/v1/find/repositories")
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("query", query);

    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Quay search failed: HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let results = json
        .get("results")
        .and_then(|v| v.as_array())
        .or_else(|| json.get("repositories").and_then(|v| v.as_array()))
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for item in results.into_iter().take(limit) {
        let full_name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                let ns = item
                    .get("namespace")
                    .or_else(|| item.get("namespace_name"))
                    .and_then(|v| v.as_str())?;
                let name = item
                    .get("repo_name")
                    .or_else(|| item.get("name"))
                    .and_then(|v| v.as_str())?;
                Some(format!("{}/{}", ns, name))
            })
            .unwrap_or_else(|| "<unknown>".to_string());

        let desc = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let stars = item
            .get("stars")
            .or_else(|| item.get("star_count"))
            .and_then(|v| v.as_u64());

        out.push(ImageSearchItem {
            source: "quay",
            reference: format!("quay.io/{}", full_name),
            description: desc,
            stars,
            pulls: None,
            official: false,
        });
    }

    Ok(out)
}

fn print_image_search_results(items: &[ImageSearchItem]) {
    println!(
        "{:<10} {:<48} {:>7} {:>12}  DESCRIPTION",
        "SOURCE", "IMAGE", "STARS", "PULLS"
    );
    for i in items {
        let stars = i.stars.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let pulls = i.pulls.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        let mut image = i.reference.clone();
        if i.official {
            image = format!("{} (official)", image);
        }
        println!(
            "{:<10} {:<48} {:>7} {:>12}  {}",
            i.source,
            truncate_str(&image, 48),
            stars,
            pulls,
            truncate_str(&i.description, 80)
        );
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for (idx, ch) in s.chars().enumerate() {
        if idx + 1 >= max {
            break;
        }
        out.push(ch);
    }
    out.push('\u{2026}');
    out
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn parse_registry_reference(reference: &str) -> Option<(String, String)> {
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let no_tag = {
        let last_slash = no_digest.rfind('/').unwrap_or(0);
        if let Some(colon_idx) = no_digest.rfind(':') {
            if colon_idx > last_slash {
                &no_digest[..colon_idx]
            } else {
                no_digest
            }
        } else {
            no_digest
        }
    };

    let (first, rest) = no_tag.split_once('/')?;
    if !(first.contains('.') || first.contains(':') || first == "localhost") {
        return None;
    }
    if rest.is_empty() {
        return None;
    }
    Some((first.to_string(), rest.to_string()))
}

async fn list_registry_tags(
    client: &reqwest::Client,
    registry: &str,
    repository: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let url = format!("https://{}/v2/{}/tags/list", registry, repository);
    let mut resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        let auth = resp
            .headers()
            .get(WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "Registry requires auth (missing WWW-Authenticate)".to_string())?;

        let fallback_scope = format!("repository:{}:pull", repository);
        let token = fetch_bearer_token(client, auth, Some(&fallback_scope)).await?;

        resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
    }

    if !resp.status().is_success() {
        return Err(format!(
            "Failed to list tags for {}/{}: HTTP {}",
            registry,
            repository,
            resp.status()
        ));
    }

    let data: RegistryTagsResponse = resp.json().await.map_err(|e| e.to_string())?;
    let mut tags = data.tags.unwrap_or_default();
    tags.sort();
    tags.truncate(limit);
    Ok(tags)
}

async fn fetch_bearer_token(
    client: &reqwest::Client,
    auth_header: &str,
    fallback_scope: Option<&str>,
) -> Result<String, String> {
    let params = parse_bearer_auth_params(auth_header)
        .ok_or_else(|| format!("Unsupported WWW-Authenticate header: {}", auth_header))?;

    let realm = params
        .get("realm")
        .ok_or_else(|| "WWW-Authenticate missing realm".to_string())?;

    let service = params.get("service").map(String::as_str);
    let scope = params.get("scope").map(String::as_str).or(fallback_scope);

    let mut url = reqwest::Url::parse(realm).map_err(|e| e.to_string())?;
    {
        let mut qp = url.query_pairs_mut();
        if let Some(service) = service {
            qp.append_pair("service", service);
        }
        if let Some(scope) = scope {
            qp.append_pair("scope", scope);
        }
        qp.append_pair("client_id", "cratebay");
    }

    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Token request failed: HTTP {}", resp.status()));
    }

    let token: RegistryTokenResponse = resp.json().await.map_err(|e| e.to_string())?;
    token
        .token
        .or(token.access_token)
        .ok_or_else(|| "Token response missing token".to_string())
}

fn parse_bearer_auth_params(header_value: &str) -> Option<HashMap<String, String>> {
    let header_value = header_value.trim();
    let mut parts = header_value.splitn(2, ' ');
    let scheme = parts.next()?.trim();
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let rest = parts.next()?.trim();

    let mut out = HashMap::new();
    for part in rest.split(',') {
        let part = part.trim();
        let mut kv = part.splitn(2, '=');
        let key = kv.next()?.trim();
        let val = kv.next()?.trim().trim_matches('"');
        out.insert(key.to_string(), val.to_string());
    }
    Some(out)
}
