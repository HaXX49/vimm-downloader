//! vimm-cli: command-line frontend for the vimm-downloader.

use clap::{Parser, Subcommand};
use vimm_core::model::{Order, Ratings, SearchQuery, Sort};
use vimm_core::VimmClient;

/// Download ROMs from the Vimm's Lair Vault.
#[derive(Parser, Debug)]
#[command(
    name = "vimm-downloader",
    version,
    about = "Download ROMs from the Vimm's Lair Vault",
    long_about = "A portable downloader for https://vimm.net/vault. \
                  See DESIGN.md for architecture and scope."
)]
struct Cli {
    /// Enable verbose logging.
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output as JSON (machine-readable).
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List the 33 supported consoles and their slugs.
    Systems,
    /// Search the Vault (omit --system to search across all systems).
    Search(SearchArgs),
    /// Show full detail for a single game.
    Info { id: u32 },
    /// Download a game's ROM.
    Download(DownloadArgs),
}

#[derive(clap::Args, Debug)]
struct SearchArgs {
    /// System slug (e.g. NES, X360-D). Omit to search across all systems.
    #[arg(long)]
    system: Option<String>,
    /// Title substring (minimum 3 characters).
    #[arg(long)]
    query: String,
    /// Comma-separated region filter.
    #[arg(long)]
    region: Option<String>,
    /// Sort field (Title|Players|Year|Rating).
    #[arg(long, default_value = "Title")]
    sort: String,
    /// Sort direction (ASC|DESC).
    #[arg(long, default_value = "ASC")]
    order: String,
    /// Maximum number of results to display.
    #[arg(long, default_value_t = 50)]
    limit: u32,
}

#[derive(clap::Args, Debug)]
struct DownloadArgs {
    /// Game ID (from `search` or `info`).
    id: u32,
    /// Specific ROM version (default: site's preselected newest).
    #[arg(long)]
    version: Option<String>,
    /// Disc number (default: 1).
    #[arg(long, default_value_t = 1)]
    disc: u32,
    /// Format key (e.g. ciso, nkit.iso, rvz). Default: site's first option.
    #[arg(long)]
    format: Option<String>,
    /// Output directory (default: current directory).
    #[arg(long, default_value_t = String::from("."))]
    out: String,
    /// Keep the raw 7z archive instead of extracting.
    #[arg(long)]
    archive: bool,
    /// Keep extras (.nfo, .txt, cover images) during extraction.
    #[arg(long)]
    keep_extras: bool,
    /// Path to a config file (default: ~/.config/vimm-downloader/config.toml).
    #[arg(long)]
    config: Option<String>,
}

fn display_or_na(value: &str) -> &str {
    if value.is_empty() {
        "N/A"
    } else {
        value
    }
}

fn display_year(year: u16) -> String {
    if year == 0 {
        "N/A".to_string()
    } else {
        year.to_string()
    }
}

fn display_ratings(ratings: Ratings) -> String {
    if ratings.graphics == 0.0
        && ratings.sound == 0.0
        && ratings.gameplay == 0.0
        && ratings.overall == 0.0
        && ratings.votes == 0
    {
        return "N/A".to_string();
    }
    format!(
        "G={:.1} S={:.1} GP={:.1} O={:.1} ({} votes)",
        ratings.graphics, ratings.sound, ratings.gameplay, ratings.overall, ratings.votes
    )
}

fn display_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes == 0 {
        "N/A".to_string()
    } else if bytes >= GIB {
        display_decimal_unit(bytes, GIB, "GB")
    } else if bytes >= MIB {
        display_decimal_unit(bytes, MIB, "MB")
    } else if bytes >= KIB {
        format!("{} KB", bytes / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn display_decimal_unit(value: u64, divisor: u64, unit: &str) -> String {
    let mut whole = value / divisor;
    let mut hundredths = ((value % divisor) * 100 + divisor / 2) / divisor;
    if hundredths == 100 {
        whole += 1;
        hundredths = 0;
    }
    format!("{whole}.{hundredths:02} {unit}")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Systems => {
            let client = VimmClient::new()?;
            let systems = client.list_systems().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&systems)?);
            } else {
                println!("{:<8}  {:<35}  YEAR", "SLUG", "NAME");
                for s in &systems {
                    println!("{:<8}  {:<35}  {}", s.slug, s.name, s.launch_year);
                }
            }
        }
        Command::Search(args) => {
            let client = VimmClient::new()?;

            let sort = match args.sort.to_lowercase().as_str() {
                "title" => Sort::Title,
                "players" => Sort::Players,
                "year" => Sort::Year,
                "rating" => Sort::Rating,
                _ => Sort::Title,
            };
            let order = match args.order.to_uppercase().as_str() {
                "ASC" => Order::Asc,
                "DESC" => Order::Desc,
                _ => Order::Asc,
            };

            let query = SearchQuery {
                system: args.system,
                q: args.query,
                sort,
                order,
                ..Default::default()
            };

            let mut results = client.search(&query).await?;
            let total = results.len();
            results.truncate(args.limit as usize);

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                println!(
                    "{:<6}  {:<40}  {:<10}  {:<8}  RATING",
                    "ID", "TITLE", "SYSTEM", "VERSION"
                );
                for g in &results {
                    let rating = g.rating.map_or("-".to_string(), |r| format!("{:.1}", r));
                    println!(
                        "{:<6}  {:<40}  {:<10}  {:<8}  {}",
                        g.id, g.title, g.system, g.version, rating
                    );
                }
                println!(
                    "\n{} result(s) (showing {} of {total})",
                    results.len(),
                    results.len()
                );
            }
        }
        Command::Info { id } => {
            let client = VimmClient::new()?;
            let detail = client.detail(id).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&detail)?);
            } else {
                println!("Title: {}", display_or_na(&detail.title));
                println!("System: {}", display_or_na(&detail.system));
                println!("Region: {}", display_or_na(&detail.region));
                println!("Players: {}", detail.players);
                println!("Year: {}", display_year(detail.year));
                println!("Publisher: {}", display_or_na(&detail.publisher));
                println!("Serial: {}", display_or_na(&detail.serial));
                println!("Ratings: {}", display_ratings(detail.ratings));
                println!("Verified: {}", display_or_na(&detail.verified_date));
                println!("\nMedia ({} version(s)):", detail.media.len());
                for (i, media) in detail.media.iter().enumerate() {
                    println!("\n  [{}] {} (disc {})", i + 1, media.version, media.disc);
                    println!("    Title: {}", display_or_na(&media.good_title));
                    println!("    Serial: {}", display_or_na(&media.serial));
                    println!("    Verified: {}", display_or_na(&media.verified_date));
                    println!("    Formats:");
                    for fmt in &media.formats {
                        println!(
                            "      alt={} key={} label={} size={}",
                            fmt.alt,
                            fmt.key,
                            fmt.label,
                            display_size(fmt.zipped_size_bytes)
                        );
                    }
                }
            }
        }
        Command::Download(args) => {
            let out = std::path::Path::new(&args.out);
            std::fs::create_dir_all(out)?;

            let config_path = args.config.as_ref().map(std::path::Path::new);
            let config = match config_path {
                Some(p) => vimm_core::Config::load_from_path(p),
                None => vimm_core::Config::load(),
            };

            let client = VimmClient::new()?;

            let detail = client.detail(args.id).await?;
            if detail.media.is_empty() {
                anyhow::bail!("No downloadable media found for game {}", args.id);
            }

            let media = &detail.media[0];
            if media.formats.is_empty() {
                anyhow::bail!("No formats available for media {}", media.id);
            }

            let resolved_format = config.resolve_format(&detail.system, args.format.as_deref());
            let fmt = if resolved_format.is_empty() {
                &media.formats[0]
            } else {
                media
                    .formats
                    .iter()
                    .find(|f| f.key == resolved_format)
                    .unwrap_or(&media.formats[0])
            };
            eprintln!(
                "Downloading: {} ({} format: {})",
                media.good_title, media.version, fmt.key
            );

            let progress_bar = if cli.json {
                None
            } else {
                let bar = indicatif::ProgressBar::new(fmt.zipped_size_bytes);
                bar.set_style(
                    indicatif::ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                        .progress_chars("#>-"),
                );
                Some(bar)
            };

            let path = vimm_core::download_rom(
                &client,
                media.id,
                fmt.alt,
                args.id,
                out,
                if cli.json {
                    None
                } else {
                    let bar = progress_bar.as_ref().unwrap().clone();
                    Some(Box::new(move |downloaded, total| {
                        bar.set_position(downloaded);
                        if let Some(t) = total {
                            bar.set_length(t);
                        }
                    })
                        as Box<dyn FnMut(u64, Option<u64>) + Send>)
                },
            )
            .await?;

            let _extracted = if args.archive {
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "game_id": args.id,
                            "media_id": media.id,
                            "version": media.version,
                            "format": fmt.key,
                            "archive": path,
                        }))?
                    );
                } else {
                    if let Some(bar) = progress_bar {
                        bar.finish_with_message(format!("Archive saved: {}", path.display()));
                    } else {
                        eprintln!("Archive saved: {}", path.display());
                    }
                }
                Vec::new()
            } else {
                let opts = vimm_core::ExtractOptions {
                    keep_archive: false,
                    keep_extras: args.keep_extras,
                };
                let files = vimm_core::extract(&path, out, opts)?;

                if cli.json {
                    let file_paths: Vec<_> = files
                        .iter()
                        .map(|f| f.to_string_lossy().to_string())
                        .collect();
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "game_id": args.id,
                            "media_id": media.id,
                            "version": media.version,
                            "format": fmt.key,
                            "files": file_paths,
                        }))?
                    );
                } else {
                    if let Some(bar) = progress_bar {
                        bar.finish_with_message(format!("Extracted {} files", files.len()));
                    } else {
                        eprintln!("Extracted {} files to {}", files.len(), out.display());
                    }
                }
                files
            };
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_unavailable_values() {
        assert_eq!(display_or_na(""), "N/A");
        assert_eq!(display_or_na("Nintendo"), "Nintendo");
        assert_eq!(display_year(0), "N/A");
        assert_eq!(display_year(2005), "2005");
        assert_eq!(
            display_ratings(Ratings {
                graphics: 0.0,
                sound: 0.0,
                gameplay: 0.0,
                overall: 0.0,
                votes: 0,
            }),
            "N/A"
        );
    }

    #[test]
    fn displays_binary_sizes_with_readable_units() {
        assert_eq!(display_size(0), "N/A");
        assert_eq!(display_size(512), "512 B");
        assert_eq!(display_size(512 * 1024), "512 KB");
        assert_eq!(display_size(6632 * 1024), "6.48 MB");
        assert_eq!(display_size(1536 * 1024 * 1024), "1.50 GB");
    }
}
