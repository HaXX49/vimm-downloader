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
    fs::create_dir_all(out_dir)
        .map_err(|error| io_with_path("create output directory", out_dir, &error))?;
    let staging = tempfile::Builder::new()
        .prefix(".vimm-extract-")
        .tempdir_in(out_dir)
        .map_err(|error| io_with_path("create extraction staging directory", out_dir, &error))?;

    match format {
        ArchiveFormat::SevenZ => extract_sevenz(archive_path, staging.path())?,
        ArchiveFormat::Zip => extract_zip(archive_path, staging.path())?,
    }

    if !opts.keep_extras {
        delete_junk(staging.path())?;
    }

    let staged_files = collect_extracted_files(staging.path())?;
    let published_files = publish_files(staging.path(), out_dir, &staged_files)?;

    if !opts.keep_archive {
        fs::remove_file(archive_path)
            .map_err(|error| io_with_path("remove downloaded archive", archive_path, &error))?;
    }

    Ok(published_files)
}

fn publish_files(
    staging_dir: &Path,
    out_dir: &Path,
    staged_files: &[PathBuf],
) -> Result<Vec<PathBuf>, VimmError> {
    let destinations = staged_files
        .iter()
        .map(|source| {
            let relative = source.strip_prefix(staging_dir).map_err(|_| {
                VimmError::Archive(format!(
                    "staged path escaped extraction directory: {}",
                    source.display()
                ))
            })?;
            let destination = out_dir.join(relative);
            ensure_destination_available(out_dir, &destination)?;
            Ok((source, destination))
        })
        .collect::<Result<Vec<_>, VimmError>>()?;

    let mut published = Vec::with_capacity(destinations.len());
    for (source, destination) in destinations {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| io_with_path("create destination directory", parent, &error))?;
        }
        fs::rename(source, &destination)
            .map_err(|error| io_with_path("publish extracted file", &destination, &error))?;
        published.push(destination);
    }
    Ok(published)
}

fn ensure_destination_available(out_dir: &Path, destination: &Path) -> Result<(), VimmError> {
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(VimmError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to overwrite existing destination '{}'",
                    destination.display()
                ),
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_with_path("inspect destination", destination, &error)),
    }

    let mut parent = destination.parent();
    while let Some(path) = parent.filter(|path| *path != out_dir) {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => {
                return Err(VimmError::Io(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "destination parent is not a directory: '{}'",
                        path.display()
                    ),
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_with_path("inspect destination parent", path, &error)),
        }
        parent = path.parent();
    }
    Ok(())
}

fn io_with_path(operation: &str, path: &Path, error: &std::io::Error) -> VimmError {
    VimmError::Io(std::io::Error::new(
        error.kind(),
        format!("{operation} '{}': {error}", path.display()),
    ))
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
    let mut file = fs::File::open(path)
        .map_err(|error| io_with_path("open downloaded archive", path, &error))?;
    let mut buf = [0u8; 6];
    let n = file
        .read(&mut buf)
        .map_err(|error| io_with_path("read downloaded archive", path, &error))?;
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
    sevenz_rust2::decompress_file(path, out_dir).map_err(|error| {
        VimmError::Archive(format!(
            "extract 7z archive '{}' into '{}': {error}",
            path.display(),
            out_dir.display()
        ))
    })
}

fn extract_zip(path: &Path, out_dir: &Path) -> Result<(), VimmError> {
    let file =
        fs::File::open(path).map_err(|error| io_with_path("open ZIP archive", path, &error))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| VimmError::Archive(format!("open ZIP '{}': {error}", path.display())))?;
    archive.extract(out_dir).map_err(|error| {
        VimmError::Archive(format!(
            "extract ZIP '{}' into '{}': {error}",
            path.display(),
            out_dir.display()
        ))
    })
}

fn collect_extracted_files(dir: &Path) -> Result<Vec<PathBuf>, VimmError> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), VimmError> {
    for entry in
        fs::read_dir(dir).map_err(|error| io_with_path("read extraction directory", dir, &error))?
    {
        let entry = entry.map_err(|error| io_with_path("read extraction entry", dir, &error))?;
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
    for entry in fs::read_dir(dir)
        .map_err(|error| io_with_path("read junk-cleanup directory", dir, &error))?
    {
        let entry = entry.map_err(|error| io_with_path("read junk-cleanup entry", dir, &error))?;
        let path = entry.path();
        if path.is_dir() {
            delete_junk_recursive(&path, junk_exts)?;
            // Remove empty directories after junk deletion.
            if path
                .read_dir()
                .map_err(|error| io_with_path("inspect cleaned directory", &path, &error))?
                .next()
                .is_none()
            {
                fs::remove_dir(&path).map_err(|error| {
                    io_with_path("remove empty cleaned directory", &path, &error)
                })?;
            }
        } else if let Some(ext) = path.extension() {
            if junk_exts.contains(ext.to_string_lossy().as_ref()) {
                fs::remove_file(&path)
                    .map_err(|error| io_with_path("remove archive junk file", &path, &error))?;
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

        archive.start_file_from_path("bonus.sav", options).unwrap();
        archive.write_all(b"SAVE_DATA").unwrap();

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

    fn assert_no_staging_directories(out_dir: &Path) {
        let staging_exists = fs::read_dir(out_dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".vimm-extract-")
        });
        assert!(!staging_exists, "temporary extraction directory remained");
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
        assert_no_staging_directories(&out_dir);
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
    fn extraction_never_deletes_unrelated_output_files() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        let nested = out_dir.join("personal");
        fs::create_dir_all(&nested).unwrap();
        fs::write(out_dir.join("Vimm's Lair.txt"), b"existing notes").unwrap();
        fs::write(out_dir.join("cover.jpg"), b"existing cover").unwrap();
        fs::write(nested.join("page.html"), b"existing page").unwrap();
        let archive = create_test_zip(tmp.path());

        extract(&archive, &out_dir, ExtractOptions::default()).unwrap();

        assert_eq!(
            fs::read(out_dir.join("Vimm's Lair.txt")).unwrap(),
            b"existing notes"
        );
        assert_eq!(
            fs::read(out_dir.join("cover.jpg")).unwrap(),
            b"existing cover"
        );
        assert_eq!(
            fs::read(nested.join("page.html")).unwrap(),
            b"existing page"
        );
        assert!(out_dir.join("game.nes").exists());
        assert_no_staging_directories(&out_dir);
    }

    #[test]
    fn destination_collision_is_non_destructive() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        fs::write(out_dir.join("game.nes"), b"MY_EXISTING_ROM").unwrap();
        let archive = create_test_zip(tmp.path());

        let error = extract(&archive, &out_dir, ExtractOptions::default()).unwrap_err();

        assert!(
            matches!(error, VimmError::Io(ref io) if io.kind() == std::io::ErrorKind::AlreadyExists)
        );
        assert_eq!(
            fs::read(out_dir.join("game.nes")).unwrap(),
            b"MY_EXISTING_ROM"
        );
        assert!(!out_dir.join("bonus.sav").exists());
        assert!(archive.exists(), "archive should remain recoverable");
        assert_no_staging_directories(&out_dir);
    }

    #[test]
    fn nested_archive_paths_are_preserved() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let archive_path = tmp.path().join("nested.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file_from_path("disc/game.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"DISC_DATA").unwrap();
        archive.finish().unwrap();

        let files = extract(&archive_path, &out_dir, ExtractOptions::default()).unwrap();

        assert_eq!(files, vec![out_dir.join("disc/game.bin")]);
        assert_eq!(fs::read(&files[0]).unwrap(), b"DISC_DATA");
        assert_no_staging_directories(&out_dir);
    }

    #[test]
    fn detect_format_sevenz() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.7z");
        fs::write(&path, SEVENZ_MAGIC).unwrap();
        assert!(matches!(
            detect_format(&path).unwrap(),
            ArchiveFormat::SevenZ
        ));
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

    fn create_test_sevenz(dir: &Path) -> PathBuf {
        // Create source files in a subdirectory.
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("game.nes"), b"ROM_DATA").unwrap();
        fs::write(src.join("readme.txt"), b"Read me!").unwrap();
        fs::write(src.join("cover.jpg"), b"JPEG_DATA").unwrap();

        let archive_path = dir.join("test.7z");
        sevenz_rust2::compress_to_path(&src, &archive_path).unwrap();
        archive_path
    }

    #[test]
    fn extract_sevenz_removes_junk() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let archive = create_test_sevenz(tmp.path());
        let files = extract(&archive, &out_dir, ExtractOptions::default()).unwrap();

        assert!(files.iter().any(|f| f.file_name().unwrap() == "game.nes"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == "readme.txt"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == "cover.jpg"));
        assert!(!archive.exists());
    }

    #[test]
    fn extract_sevenz_keep_extras() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let archive = create_test_sevenz(tmp.path());
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
    fn extract_sevenz_keep_archive() {
        let tmp = setup_test_dir();
        let out_dir = tmp.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();

        let archive = create_test_sevenz(tmp.path());
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
}
