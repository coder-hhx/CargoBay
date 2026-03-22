//! Filesystem utility functions.
//!
//! Provides optimised file operations such as copy-on-write cloning
//! on macOS (APFS) to speed up installation of large VM disk images.

use std::path::Path;

/// Copy a file, using copy-on-write cloning when available.
///
/// On macOS with APFS, this calls `clonefile(2)` first which is nearly
/// instantaneous for large files.  Falls back to a regular byte-copy on
/// other platforms or when the clone syscall is unavailable.
pub fn copy_file_fast(src: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(target_os = "macos")]
    {
        // Only attempt clonefile when the destination does not already exist
        // (clonefile fails with EEXIST).
        if !dest.exists() && try_clonefile(src, dest).is_ok() {
            return Ok(());
        }
    }

    std::fs::copy(src, dest)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn try_clonefile(src: &Path, dest: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let src = CString::new(src.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "src contains NUL"))?;
    let dest = CString::new(dest.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "dest contains NUL"))?;

    // SAFETY: clonefile is a well-known macOS syscall that takes two C string
    // paths and a flags argument. Both CString pointers are valid for the
    // duration of the call.
    let rc = unsafe { libc::clonefile(src.as_ptr(), dest.as_ptr(), 0) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_file_fast_copies_content() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("source.txt");
        let dest = tmp.path().join("sub").join("dest.txt");

        std::fs::write(&src, b"hello world").unwrap();
        copy_file_fast(&src, &dest).unwrap();

        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn copy_file_fast_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("source.bin");
        let dest = tmp.path().join("a").join("b").join("c").join("dest.bin");

        std::fs::write(&src, b"data").unwrap();
        copy_file_fast(&src, &dest).unwrap();

        assert!(dest.exists());
    }

    #[test]
    fn copy_file_fast_overwrites_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dest = tmp.path().join("dst");

        std::fs::write(&src, b"new").unwrap();
        std::fs::write(&dest, b"old").unwrap();

        copy_file_fast(&src, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new");
    }
}
