//! Extracts the embedded PDFium library into a private directory for dynamic loading.

use std::io;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use {
    once_cell::sync::OnceCell,
    sha2::{Digest, Sha256},
    std::fs,
    std::io::Write,
    tempfile::TempDir,
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[cfg(not(target_arch = "wasm32"))]
static PDFIUM_DIR: OnceCell<TempDir> = OnceCell::new();

#[cfg(not(target_arch = "wasm32"))]
fn bundled_library_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    }
}

/// Extract and verify the bundled PDFium library once per process.
///
/// Keeps the directory alive for the lifetime of the process so the library
/// can be loaded by path after extraction.
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_bundled_pdfium() -> io::Result<PathBuf> {
    let dir = PDFIUM_DIR.get_or_try_init(|| {
        let mut builder = tempfile::Builder::new();
        builder.prefix("kreuzberg-pdfium-");
        #[cfg(unix)]
        builder.permissions(fs::Permissions::from_mode(0o700));
        let dir = builder.tempdir()?;

        #[cfg(unix)]
        let owner = {
            let owner = rustix::process::geteuid().as_raw();
            let metadata = fs::symlink_metadata(dir.path())?;
            if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Insecure PDFium directory",
                ));
            }
            owner
        };

        let path = dir.path().join(bundled_library_name());
        let embedded = include_bytes!(env!("KREUZBERG_PDFIUM_BUNDLED_PATH"));
        let mut file = fs::File::create_new(&path)?;
        file.write_all(embedded)?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o500))?;
        drop(file);

        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PDFium path is not a regular file",
            ));
        }
        #[cfg(unix)]
        if metadata.uid() != owner {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unexpected PDFium file owner",
            ));
        }
        if Sha256::digest(fs::read(&path)?) != Sha256::digest(embedded) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "PDFium SHA-256 mismatch"));
        }

        Ok::<_, io::Error>(dir)
    })?;
    Ok(dir.path().join(bundled_library_name()))
}

#[cfg(target_arch = "wasm32")]
pub fn extract_bundled_pdfium() -> io::Result<PathBuf> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Bundled PDFium file extraction is unavailable on WASM",
    ))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "bundled-pdfium")]
    fn test_extract_bundled_pdfium() {
        let result = extract_bundled_pdfium();
        assert!(result.is_ok());

        let lib_path = result.unwrap();
        assert!(
            lib_path.exists(),
            "Extracted library should exist at: {}",
            lib_path.display()
        );
        assert!(lib_path.file_name().is_some(), "Library path should have filename");

        let expected_name = bundled_library_name();
        assert_eq!(lib_path.file_name().unwrap(), expected_name);
    }

    #[test]
    #[cfg(feature = "bundled-pdfium")]
    fn test_extract_bundled_pdfium_reuses_existing() {
        let result1 = extract_bundled_pdfium();
        assert!(result1.is_ok());
        let path1 = result1.unwrap();

        let metadata1 = fs::metadata(&path1).expect("Should be able to read metadata");
        let size1 = metadata1.len();

        let result2 = extract_bundled_pdfium();
        assert!(result2.is_ok());
        let path2 = result2.unwrap();

        assert_eq!(path1, path2, "Extraction should return same path on second call");

        let metadata2 = fs::metadata(&path2).expect("Should be able to read metadata");
        let size2 = metadata2.len();
        assert_eq!(size1, size2, "Reused library should have same file size");
    }

    #[test]
    #[cfg(feature = "bundled-pdfium")]
    fn test_extract_bundled_pdfium_concurrent_access() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    let result = extract_bundled_pdfium();
                    assert!(result.is_ok(), "Concurrent extraction should succeed");
                    result.unwrap()
                })
            })
            .collect();

        let paths: Vec<PathBuf> = handles
            .into_iter()
            .map(|h| h.join().expect("Thread should complete"))
            .collect();

        let first_path = &paths[0];
        assert!(
            paths.iter().all(|p| p == first_path),
            "All concurrent extractions should return the same path"
        );

        assert!(
            first_path.exists(),
            "Extracted library should exist at: {}",
            first_path.display()
        );

        let metadata = fs::metadata(first_path).expect("Should be able to read metadata");
        let file_size = metadata.len();
        assert!(
            file_size > 1_000_000,
            "PDFium library should be at least 1MB, got {} bytes",
            file_size
        );
    }

    #[test]
    #[cfg(unix)]
    #[cfg(feature = "bundled-pdfium")]
    fn test_extract_bundled_pdfium_permissions() {
        let result = extract_bundled_pdfium();
        assert!(result.is_ok());

        let lib_path = result.unwrap();
        let metadata = fs::metadata(&lib_path).expect("Should be able to read metadata");
        let dir_metadata = fs::symlink_metadata(lib_path.parent().unwrap()).unwrap();
        assert_eq!(dir_metadata.permissions().mode() & 0o777, 0o700);
        assert_eq!(dir_metadata.uid(), rustix::process::geteuid().as_raw());
        assert_eq!(metadata.uid(), dir_metadata.uid());
        let perms = metadata.permissions();
        let mode = perms.mode();

        assert!(
            mode & 0o111 != 0,
            "Library should have executable bit set, got mode: {:#o}",
            mode
        );
    }
}
