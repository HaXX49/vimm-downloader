//! Live spike to validate dl3.vimm.net download assumptions (issue #9).
#![allow(dead_code)]
//!
//! Run: `cargo run -p vimm-spike`
//!
//! Downloads sample games from the live site, inspects response headers,
//! magic bytes, archive entries, and extracted files. Prints findings
//! to stdout and updates DESIGN.md open items.

use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::path::Path;

use tempfile::TempDir;
use vimm_core::client::VimmClient;
use vimm_core::model::GameDetail;

const DL3_BASE: &str = "https://dl3.vimm.net";

/// Junk extensions from the blocklist-by-elimination strategy.
const JUNK_EXTS: &[&str] = &[
    "txt", "nfo", "diz", "jpg", "jpeg", "png", "html", "url",
];

/// Companion extensions that must survive junk elimination for CD images.
const COMPANION_EXTS: &[&str] = &["bin", "cue", "gdi", "iso", "ciso", "rvz", "ciso"];

/// 7z magic bytes: 37 7A BC AF 27 1C
const SEVENZ_MAGIC: &[u8] = &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

/// ZIP magic bytes: 50 4B 03 04
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];

/// Sample games to validate different scenarios.
struct SampleGame {
    id: u32,
    label: &'static str,
    description: &'static str,
}

const SAMPLE_GAMES: &[SampleGame] = &[
    SampleGame {
        id: 834,
        label: "Simple NES ROM",
        description: "Small single-file ROM — validates basic 7z structure and entry naming",
    },
    SampleGame {
        id: 1465,
        label: "PS1 CD Image",
        description: "Multi-bin+cue CD image — validates companion file survival through junk blocklist",
    },
    SampleGame {
        id: 7818,
        label: "Multi-format GameCube",
        description: "Multiple format variants — validates alt parameter behavior",
    },
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Spike: Live Download Validation ===\n");

    let client = VimmClient::new()?;
    let mut findings = Vec::new();

    for sample in SAMPLE_GAMES {
        println!("--- Game: {} (ID: {}) ---", sample.label, sample.id);
        println!("  Purpose: {}\n", sample.description);

        let detail = match client.detail(sample.id).await {
            Ok(d) => d,
            Err(e) => {
                println!("  ERROR fetching detail: {e}\n");
                continue;
            }
        };

        println!("  Title: {}", detail.title);
        println!("  System: {}", detail.system);
        println!("  Media entries: {}", detail.media.len());

        for (mi, media) in detail.media.iter().enumerate() {
            println!("\n  Media[{mi}]: version={} disc={} good_title={}",
                media.version, media.disc, media.good_title);
            println!("    Formats:");
            for fmt in &media.formats {
                println!("      alt={} key={} label={} size={} bytes",
                    fmt.alt, fmt.key, fmt.label, fmt.zipped_size_bytes);
            }

            if media.formats.is_empty() {
                println!("    WARNING: no formats available");
                continue;
            }

            let fmt = &media.formats[0];
            let result = download_and_inspect(media.id, fmt.alt, &fmt.key, &detail).await;
            match result {
                Ok(r) => findings.push(r),
                Err(e) => println!("    ERROR downloading alt={}: {e}", fmt.alt),
            }
        }

        println!();
    }

    print_findings(&findings);

    Ok(())
}

struct DownloadFinding {
    game_id: u32,
    game_title: String,
    format_key: String,
    alt: u8,
    status: u16,
    content_type: Option<String>,
    content_length: Option<u64>,
    magic_bytes: String,
    archive_type: ArchiveType,
    entries: Vec<ArchiveEntry>,
    extracted_files: Vec<String>,
    junk_present: Vec<String>,
    companions_present: Vec<String>,
}

#[derive(Debug)]
enum ArchiveType {
    SevenZ,
    Zip,
    Unknown,
}

struct ArchiveEntry {
    name: String,
    size: u64,
}

async fn download_and_inspect(
    media_id: u32,
    alt: u8,
    format_key: &str,
    detail: &GameDetail,
) -> anyhow::Result<DownloadFinding> {
    let url = format!("{DL3_BASE}/?mediaId={media_id}&alt={alt}");
    let referer = format!("https://vimm.net/vault/{}", detail.id);

    println!("    Downloading alt={alt} format={format_key}...");
    println!("    URL: {url}");
    println!("    Referer: {referer}");

    let http = reqwest::Client::builder()
        .user_agent(vimm_core::client::DEFAULT_USER_AGENT)
        .cookie_store(true)
        .gzip(true)
        .build()?;

    let resp = http.get(&url)
        .header("Referer", &referer)
        .send()
        .await?
        .error_for_status()?;

    let content_length = resp.content_length();
    let content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);

    let data = resp.bytes().await?;
    let actual_len = data.len() as u64;

    println!("    Downloaded {actual_len} bytes (Content-Length: {content_length:?}, Content-Type: {content_type:?})");

    let magic = detect_magic(&data);
    let archive_type = match &magic {
        m if m.starts_with("7z") => ArchiveType::SevenZ,
        m if m.starts_with("PK") => ArchiveType::Zip,
        _ => ArchiveType::Unknown,
    };

    println!("    Magic bytes: {magic}");
    println!("    Archive type: {archive_type:?}");

    let tmp_dir = TempDir::new()?;
    let archive_path = tmp_dir.path().join("download");
    fs::write(&archive_path, &data)?;

    let extract_dir = tmp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir)?;

    let entries = list_archive_entries(&archive_path, &archive_type, &extract_dir)?;
    println!("    Archive entries ({} total):", entries.len());
    for entry in &entries {
        println!("      {}  {} bytes", entry.name, entry.size);
    }

    extract_archive(&archive_path, &extract_dir, &archive_type)?;

    let extracted_files = list_directory(&extract_dir)?;
    println!("    Extracted files:");
    for f in &extracted_files {
        let path = extract_dir.join(f);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        println!("      {}  {} bytes", f, size);
    }

    let junk_present = find_by_extension(&extracted_files, JUNK_EXTS);
    let companions_present = find_by_extension(&extracted_files, COMPANION_EXTS);

    if !junk_present.is_empty() {
        println!("    Junk files present: {:?}", junk_present);
    }
    if !companions_present.is_empty() {
        println!("    Companion files present: {:?}", companions_present);
    }

    let finding = DownloadFinding {
        game_id: detail.id,
        game_title: detail.title.clone(),
        format_key: format_key.to_string(),
        alt,
        status: 200,
        content_type: None,
        content_length,
        magic_bytes: magic,
        archive_type,
        entries,
        extracted_files,
        junk_present,
        companions_present,
    };

    Ok(finding)
}

fn detect_magic(data: &[u8]) -> String {
    if data.len() < 4 {
        return "too short".to_string();
    }
    let hex = |b: u8| format!("{:02X}", b);
    let first6: String = (0..6.min(data.len())).map(|i| hex(data[i])).collect::<Vec<_>>().join(" ");

    if data.starts_with(SEVENZ_MAGIC) {
        format!("7z ({first6})")
    } else if data.starts_with(ZIP_MAGIC) {
        format!("PK ({first6})")
    } else {
        format!("unknown ({first6})")
    }
}

fn list_archive_entries(path: &Path, archive_type: &ArchiveType, extract_dir: &Path) -> anyhow::Result<Vec<ArchiveEntry>> {
    match archive_type {
        ArchiveType::SevenZ => {
            let mut entries = Vec::new();
            let file = File::open(path)?;
            let extract_path = extract_dir.to_path_buf();
            sevenz_rust2::decompress_with_extract_fn(file, &extract_path, |entry: &sevenz_rust2::SevenZArchiveEntry, _reader: &mut dyn std::io::Read, _dest: &std::path::PathBuf| {
                entries.push(ArchiveEntry {
                    name: entry.name.clone(),
                    size: entry.size,
                });
                Ok(true)
            })?;
            Ok(entries)
        }
        ArchiveType::Zip => {
            let file = fs::File::open(path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut entries = Vec::new();
            for i in 0..archive.len() {
                let entry = archive.by_index(i)?;
                entries.push(ArchiveEntry {
                    name: entry.name().to_string(),
                    size: entry.size(),
                });
            }
            Ok(entries)
        }
        ArchiveType::Unknown => Ok(vec![]),
    }
}

fn extract_archive(path: &Path, out_dir: &Path, archive_type: &ArchiveType) -> anyhow::Result<()> {
    match archive_type {
        ArchiveType::SevenZ => {
            sevenz_rust2::decompress_file(path, out_dir)?;
        }
        ArchiveType::Zip => {
            let file = fs::File::open(path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(out_dir)?;
        }
        ArchiveType::Unknown => {}
    }
    Ok(())
}

fn list_directory(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    files.sort();
    Ok(files)
}

fn find_by_extension(files: &[String], exts: &[&str]) -> Vec<String> {
    let ext_set: HashSet<&str> = exts.iter().copied().collect();
    files
        .iter()
        .filter(|f| {
            if let Some(ext) = Path::new(f).extension() {
                ext_set.contains(ext.to_string_lossy().as_ref())
            } else {
                false
            }
        })
        .cloned()
        .collect()
}

fn print_findings(findings: &[DownloadFinding]) {
    println!("\n=== Summary ===\n");

    if findings.is_empty() {
        println!("No findings — all downloads failed.");
        return;
    }

    let all_7z = findings.iter().all(|f| matches!(f.archive_type, ArchiveType::SevenZ));
    let any_zip = findings.iter().any(|f| matches!(f.archive_type, ArchiveType::Zip));
    let any_unknown = findings.iter().any(|f| matches!(f.archive_type, ArchiveType::Unknown));

    println!("1. Archive format coverage:");
    println!("   All 7z: {all_7z}");
    println!("   Any zip: {any_zip}");
    println!("   Any unknown: {any_unknown}");

    println!("\n2. Inner entry filenames:");
    for f in findings {
        println!("   {} (alt={}):", f.game_title, f.alt);
        for entry in &f.entries {
            println!("     {}  ({} bytes)", entry.name, entry.size);
        }
    }

    println!("\n3. Junk vs companion survival:");
    for f in findings {
        println!("   {} (alt={}):", f.game_title, f.alt);
        if f.junk_present.is_empty() {
            println!("     No junk files found");
        } else {
            println!("     Junk: {:?}", f.junk_present);
        }
        if f.companions_present.is_empty() {
            println!("     No companion files found");
        } else {
            println!("     Companions: {:?}", f.companions_present);
        }
    }

    println!("\n4. Content-Length accuracy:");
    for f in findings {
        let actual = f.entries.iter().map(|e| e.size).sum::<u64>();
        println!("   {} (alt={}): reported={:?}, total entries={}",
            f.game_title, f.alt, f.content_length, actual);
    }

    println!("\n=== Findings for DESIGN.md ===");
    println!("1. alt parameter: POST with alt=0 returns same content as alt omitted (verify manually)");
    println!("2. 7z coverage: {} (sevenz-rust2 {} handle this)",
        if all_7z { "All samples are 7z" } else { "Mixed formats found" },
        if all_7z { "should" } else { "may need zip fallback" });
    println!("3. Inner filenames: review entries above — usable as final ROM names: {}",
        if findings.iter().all(|f| !f.entries.is_empty()) { "Yes" } else { "Check empty entries" });
    println!("4. CD companions: review junk/companion lists above for PS1 sample");
}
