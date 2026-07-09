//! Archive extraction with junk-by-elimination logic.
//!
//! Detects archive format by magic bytes (7z primary, zip fallback),
//! extracts entries to the output directory, and removes junk files
//! per the blocklist.

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::VimmError;

/// File extensions considered junk and removed during extraction.
const JUNK_EXTS: &[&str] = &["txt", "nfo", "diz", "jpg", "jpeg", "png", "html", "url"];

/// Options controlling extraction behavior.
#[derive(Debug, Clone, Default, Copy)]
pub struct ExtractOptions {
    /// Keep the raw archive instead of deleting it after extraction.
    pub keep_archive: bool,
    /// Keep junk files (`.txt`, `.nfo`, etc.) instead of deleting them.
    pub keep_extras: bool,
}

/// Extract an archive to the output directory.
///
/// Detects format by magic bytes, extracts all entries, then removes
/// junk files unless [`ExtractOptions::keep_extras`] is set.
///
/// # Errors
///
/// - [`VimmError::Archive`] if the archive cannot be opened or extracted.
/// - [`VimmError::Io`] if file operations fail.
pub fn extract(
    archive_path: &Path,
    out_dir: &Path,
    opts: ExtractOptions,
) -> Result<Vec<PathBuf>, VimmError> {
    let format = detect_format(archive_path)?;

    match format {
        ArchiveFormat::SevenZ => extract_sevenz(archive_path, out_dir)?,
        ArchiveFormat::Zip => extract_zip(archive_path, out_dir)?,
    }

    if !opts.keep_extras {
        delete_junk(out_dir)?;
    }

    if !opts.keep_archive {
        fs::remove_file(archive_path).map_err(VimmError::from)?;
    }

    // Re-collect after junk deletion.
    collect_extracted_files(out_dir)
}

enum ArchiveFormat {
    SevenZ,
    Zip,
}

/// 7z magic bytes: `37 7A BC AF 27 1C`.
const SEVENZ_MAGIC: &[u8] = &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

/// ZIP magic bytes: `50 4B 03 04`.
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];

fn detect_format(path: &Path) -> Result<ArchiveFormat, VimmError> {
    let mut file = fs::File::open(path).map_err(VimmError::from)?;
    let mut buf = [0u8; 6];
    let n = file.read(&mut buf).map_err(VimmError::from)?;
    if n < 4 {
        return Err(VimmError::Archive("file too short to detect format".into()));
    }
    if buf.starts_with(SEVENZ_MAGIC) {
        Ok(ArchiveFormat::SevenZ)
    } else if buf.starts_with(ZIP_MAGIC) {
        Ok(ArchiveFormat::Zip)
    } else {
        Err(VimmError::Archive(format!(
            "unrecognized archive magic bytes: {:02x?}",
            &buf[..n]
        )))
    }
}

fn extract_sevenz(path: &Path, out_dir: &Path) -> Result<(), VimmError> {
    let file = fs::File::open(path).map_err(VimmError::from)?;
    sevenz_rust2::decompress_file(path, out_dir).map_err(|e| VimmError::Archive(e.to_string()))?;
    // Suppress unused warning for `file` — decompress_file opens its own handle.
    drop(file);
    Ok(())
}

fn extract_zip(path: &Path, out_dir: &Path) -> Result<(), VimmError> {
    let file = fs::File::open(path).map_err(VimmError::from)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| VimmError::Archive(e.to_string()))?;
    archive.extract(out_dir).map_err(|e| VimmError::Archive(e.to_string()))?;
    Ok(())
}

fn collect_extracted_files(dir: &Path) -> Result<Vec<PathBuf>, VimmError> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), VimmError> {
    for entry in fs::read_dir(dir).map_err(VimmError::from)? {
        let entry = entry.map_err(VimmError::from)?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn delete_junk(dir: &Path) -> Result<(), VimmError> {
    let junk_exts: HashSet<&str> = JUNK_EXTS.iter().copied().collect();
    delete_junk_recursive(dir, &junk_exts)
}

fn delete_junk_recursive(dir: &Path, junk_exts: &HashSet<&str>) -> Result<(), VimmError> {
    for entry in fs::read_dir(dir).map_err(VimmError::from)? {
        let entry = entry.map_err(VimmError::from)?;
        let path = entry.path();
        if path.is_dir() {
            delete_junk_recursive(&path, junk_exts)?;
            // Remove empty directories after junk deletion.
            if path.read_dir().map_err(VimmError::from)?.next().is_none() {
                fs::remove_dir(&path).map_err(VimmError::from)?;
            }
        } else if let Some(ext) = path.extension() {
            if junk_exts.contains(ext.to_string_lossy().as_ref()) {
                fs::remove_file(&path).map_err(VimmError::from)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_zip(dir: &Path) -> PathBuf {
        let archive_path = dir.join("test.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        archive.start_file_from_path("game.nes", options).unwrap();
        archive.write_all(b"ROM_DATA").unwrap();

        archive.start_file_from_path("readme.txt", options).unwrap();
        archive.write_all(b"Read me!").unwrap();

        archive.start_file_from_path("cover.jpg", options).unwrap();
        archive.write_all(b"JPEG_DATA").unwrap();

        archive.finish().unwrap();
        archive_path
    }

    fn setup_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn extract_zip_removes_junk() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let archive = create_test_zip(tmp.path());
        let files = extract(&archive, &out_dir, ExtractOptions::default()).unwrap();

        assert!(files.iter().any(|f| f.file_name().unwrap() == "game.nes"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == "readme.txt"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == "cover.jpg"));
        assert!(!archive.exists());
    }

    #[test]
    fn extract_zip_keep_extras() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let archive = create_test_zip(tmp.path());
        let files = extract(
            &archive,
            &out_dir,
            ExtractOptions {
                keep_extras: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(files.iter().any(|f| f.file_name().unwrap() == "game.nes"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "readme.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "cover.jpg"));
    }

    #[test]
    fn extract_zip_keep_archive() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let archive = create_test_zip(tmp.path());
        extract(
            &archive,
            &out_dir,
            ExtractOptions {
                keep_archive: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(archive.exists());
    }

    #[test]
    fn detect_format_sevenz() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.7z");
        fs::write(&path, SEVENZ_MAGIC).unwrap();
        assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::SevenZ));
    }

    #[test]
    fn detect_format_zip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.zip");
        fs::write(&path, ZIP_MAGIC).unwrap();
        assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::Zip));
    }

    #[test]
    fn detect_format_unknown() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.bin");
        fs::write(&path, b"NOT_AN_ARCHIVE").unwrap();
        assert!(detect_format(&path).is_err());
    }
}
