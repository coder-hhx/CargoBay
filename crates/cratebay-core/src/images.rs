//! Linux OS image catalog, download, and management.
//!
//! Provides a built-in catalog of downloadable Linux distributions
//! (kernel + initrd + rootfs) for VM booting.
//!
//! Ported from master branch `images.rs` and adapted for the v2 error model
//! (`AppError` instead of `ImageError`).

use crate::storage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Status of an OS image on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStatus {
    /// Not yet downloaded.
    NotDownloaded,
    /// Currently downloading.
    Downloading,
    /// Downloaded and ready to use.
    Ready,
}

/// A single downloadable Linux OS image entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsImageEntry {
    /// Short identifier, e.g. "alpine-3.19".
    pub id: String,
    /// Human-readable name, e.g. "Alpine Linux 3.19".
    pub name: String,
    /// Distribution version string.
    pub version: String,
    /// CPU architecture (aarch64 / x86_64).
    pub arch: String,
    /// URL to download the kernel (vmlinuz).
    pub kernel_url: String,
    /// URL to download the initrd / initramfs.
    pub initrd_url: String,
    /// URL to download the root filesystem image (optional).
    pub rootfs_url: String,
    /// Approximate total download size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum of the kernel file (hex).
    pub kernel_sha256: String,
    /// SHA-256 checksum of the initrd file (hex).
    pub initrd_sha256: String,
    /// SHA-256 checksum of the rootfs file (hex).
    pub rootfs_sha256: String,
    /// Default kernel command line.
    pub default_cmdline: String,
    /// Current status on disk.
    #[serde(default = "default_status")]
    pub status: ImageStatus,
}

fn default_status() -> ImageStatus {
    ImageStatus::NotDownloaded
}

/// Progress information for an ongoing download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub image_id: String,
    /// Which file is being downloaded: "kernel", "initrd", or "rootfs".
    pub current_file: String,
    /// Bytes downloaded so far (across all files).
    pub bytes_downloaded: u64,
    /// Total bytes to download (across all files).
    pub bytes_total: u64,
    /// `true` when the download is complete.
    pub done: bool,
    /// Error message if something went wrong.
    pub error: Option<String>,
}

/// Paths to the downloaded image files on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePaths {
    pub kernel_path: PathBuf,
    pub initrd_path: PathBuf,
    pub rootfs_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Built-in catalog
// ---------------------------------------------------------------------------

/// Return the built-in catalog of available Linux images.
///
/// This is compiled into the binary.  The URLs below are placeholders that
/// will be replaced with real CDN / GitHub Release asset URLs once the
/// image build pipeline is ready.
pub fn builtin_catalog() -> Vec<OsImageEntry> {
    vec![
        OsImageEntry {
            id: "cratebay-runtime-aarch64".into(),
            name: "CrateBay Runtime (aarch64)".into(),
            version: "1.0.0".into(),
            arch: "aarch64".into(),
            kernel_url: "https://github.com/coder-hhx/CrateBay/releases/download/runtime-v1.0.0/vmlinuz-aarch64".into(),
            initrd_url: "https://github.com/coder-hhx/CrateBay/releases/download/runtime-v1.0.0/initramfs-aarch64".into(),
            rootfs_url: String::new(),
            size_bytes: 120_000_000,
            kernel_sha256: String::new(),
            initrd_sha256: String::new(),
            rootfs_sha256: String::new(),
            default_cmdline: "console=hvc0 panic=1".into(),
            status: ImageStatus::NotDownloaded,
        },
        OsImageEntry {
            id: "cratebay-runtime-x86_64".into(),
            name: "CrateBay Runtime (x86_64)".into(),
            version: "1.0.0".into(),
            arch: "x86_64".into(),
            kernel_url: "https://github.com/coder-hhx/CrateBay/releases/download/runtime-v1.0.0/vmlinuz-x86_64".into(),
            initrd_url: "https://github.com/coder-hhx/CrateBay/releases/download/runtime-v1.0.0/initramfs-x86_64".into(),
            rootfs_url: String::new(),
            size_bytes: 120_000_000,
            kernel_sha256: String::new(),
            initrd_sha256: String::new(),
            rootfs_sha256: String::new(),
            default_cmdline: "console=hvc0 panic=1".into(),
            status: ImageStatus::NotDownloaded,
        },
        OsImageEntry {
            id: "alpine-3.19".into(),
            name: "Alpine Linux 3.19".into(),
            version: "3.19".into(),
            arch: "aarch64".into(),
            kernel_url: "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/aarch64/netboot/vmlinuz-lts".into(),
            initrd_url: "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/aarch64/netboot/initramfs-lts".into(),
            rootfs_url: String::new(),
            size_bytes: 50_000_000,
            kernel_sha256: String::new(),
            initrd_sha256: String::new(),
            rootfs_sha256: String::new(),
            default_cmdline: "console=hvc0 panic=1".into(),
            status: ImageStatus::NotDownloaded,
        },
    ]
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Root directory for all downloaded images.
pub fn images_dir() -> PathBuf {
    storage::data_dir().join("images")
}

/// Directory for a specific image's files.
pub fn image_dir(image_id: &str) -> PathBuf {
    images_dir().join(image_id)
}

/// Canonical file paths for a given image.
pub fn image_paths(image_id: &str) -> ImagePaths {
    let dir = image_dir(image_id);
    ImagePaths {
        kernel_path: dir.join("vmlinuz"),
        initrd_path: dir.join("initramfs"),
        rootfs_path: dir.join("rootfs.img"),
    }
}

// ---------------------------------------------------------------------------
// Status persistence (JSON metadata file)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Meta {
    #[serde(default = "default_status")]
    status: ImageStatus,
}

fn status_file(image_id: &str) -> PathBuf {
    image_dir(image_id).join("metadata.json")
}

/// Persist the image status to a metadata file.
pub fn save_image_status(image_id: &str, status: &ImageStatus) -> Result<(), std::io::Error> {
    let meta = Meta {
        status: status.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    storage::write_atomic(&status_file(image_id), &bytes)
}

/// Load the image status from its metadata file.
pub fn load_image_status(image_id: &str) -> ImageStatus {
    let path = status_file(image_id);
    let Ok(bytes) = std::fs::read(&path) else {
        return ImageStatus::NotDownloaded;
    };
    let Ok(meta) = serde_json::from_slice::<Meta>(&bytes) else {
        return ImageStatus::NotDownloaded;
    };
    meta.status
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// List all available OS images, with their current download status.
pub fn list_available_images() -> Vec<OsImageEntry> {
    let mut catalog = builtin_catalog();
    for entry in &mut catalog {
        entry.status = load_image_status(&entry.id);
    }
    catalog
}

/// List only images that have been downloaded and are ready.
pub fn list_downloaded_images() -> Vec<OsImageEntry> {
    list_available_images()
        .into_iter()
        .filter(|e| e.status == ImageStatus::Ready)
        .collect()
}

/// Find a catalog entry by id.
pub fn find_image(id: &str) -> Option<OsImageEntry> {
    list_available_images().into_iter().find(|e| e.id == id)
}

/// Delete a downloaded image from disk.
pub fn delete_image(image_id: &str) -> Result<(), std::io::Error> {
    let dir = image_dir(image_id);
    if !dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("image not found: {}", image_id),
        ));
    }
    std::fs::remove_dir_all(&dir)
}

/// Check if an image is downloaded and ready.
pub fn is_image_ready(image_id: &str) -> bool {
    load_image_status(image_id) == ImageStatus::Ready
}

/// Create a VM disk image by copying the rootfs or creating a blank raw file.
///
/// If the rootfs file exists it is used as the base; otherwise a sparse
/// raw image of `size_bytes` is created.
pub fn create_disk_from_image(
    image_id: &str,
    dest: &Path,
    size_bytes: u64,
) -> Result<(), std::io::Error> {
    let paths = image_paths(image_id);

    if paths.rootfs_path.exists() {
        crate::fsutil::copy_file_fast(&paths.rootfs_path, dest)?;

        // Ensure the disk is at least `size_bytes` (sparse extend).
        let current = std::fs::metadata(dest)?.len();
        if current < size_bytes {
            let f = std::fs::OpenOptions::new().write(true).open(dest)?;
            f.set_len(size_bytes)?;
        }
    } else {
        // Create a sparse raw disk image.
        let f = std::fs::File::create(dest)?;
        f.set_len(size_bytes)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_not_empty() {
        let catalog = builtin_catalog();
        assert!(!catalog.is_empty());
    }

    #[test]
    fn builtin_catalog_has_unique_ids() {
        let catalog = builtin_catalog();
        let mut ids: Vec<&str> = catalog.iter().map(|e| e.id.as_str()).collect();
        let len_before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), len_before, "catalog image ids must be unique");
    }

    #[test]
    fn builtin_catalog_entries_have_required_fields() {
        for entry in builtin_catalog() {
            assert!(!entry.id.is_empty(), "id should not be empty");
            assert!(!entry.name.is_empty(), "name should not be empty");
            assert!(!entry.version.is_empty(), "version should not be empty");
            assert!(!entry.arch.is_empty(), "arch should not be empty");
            assert!(!entry.kernel_url.is_empty(), "kernel_url should not be empty");
            assert!(!entry.initrd_url.is_empty(), "initrd_url should not be empty");
            assert!(entry.size_bytes > 0, "size_bytes should be > 0");
            assert!(!entry.default_cmdline.is_empty(), "default_cmdline should not be empty");
            assert_eq!(entry.status, ImageStatus::NotDownloaded);
        }
    }

    #[test]
    fn find_image_returns_some_for_known_id() {
        let entry = find_image("alpine-3.19");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().id, "alpine-3.19");
    }

    #[test]
    fn find_image_returns_none_for_unknown_id() {
        assert!(find_image("nonexistent-distro-99").is_none());
    }

    #[test]
    fn image_paths_contains_expected_filenames() {
        let paths = image_paths("alpine-3.19");
        assert!(paths.kernel_path.ends_with("vmlinuz"));
        assert!(paths.initrd_path.ends_with("initramfs"));
        assert!(paths.rootfs_path.ends_with("rootfs.img"));
    }

    #[test]
    fn image_paths_contain_image_id_in_path() {
        let paths = image_paths("ubuntu-24.04");
        let kernel_str = paths.kernel_path.to_string_lossy();
        assert!(kernel_str.contains("ubuntu-24.04"));
    }

    #[test]
    fn images_dir_is_under_data_dir() {
        let img = images_dir();
        let data = storage::data_dir();
        assert!(img.starts_with(&data));
    }

    #[test]
    fn image_dir_appends_image_id() {
        let dir = image_dir("debian-12");
        assert!(dir.ends_with("debian-12"));
    }

    #[test]
    fn image_status_serde_round_trip() {
        for status in [
            ImageStatus::NotDownloaded,
            ImageStatus::Downloading,
            ImageStatus::Ready,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: ImageStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, status);
        }
    }

    #[test]
    fn is_image_ready_returns_false_for_non_downloaded() {
        assert!(!is_image_ready("nonexistent-image-xyz"));
    }

    #[test]
    fn default_status_is_not_downloaded() {
        assert_eq!(default_status(), ImageStatus::NotDownloaded);
    }

    #[test]
    fn builtin_catalog_contains_runtime_images() {
        let catalog = builtin_catalog();
        assert!(catalog.iter().any(|e| e.id == "cratebay-runtime-aarch64"));
        assert!(catalog.iter().any(|e| e.id == "cratebay-runtime-x86_64"));
    }
}
