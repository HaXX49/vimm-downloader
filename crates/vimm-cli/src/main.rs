//! vimm-cli: command-line frontend for the vimm-downloader.

use clap::{Parser, Subcommand};
use vimm_core::model::{Order, SearchQuery, Sort};
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
                println!("Title: {}", detail.title);
                println!("System: {}", detail.system);
                println!("Region: {}", detail.region);
                println!("Players: {}", detail.players);
                println!("Year: {}", detail.year);
                println!("Publisher: {}", detail.publisher);
                println!("Serial: {}", detail.serial);
                println!(
                    "Ratings: G={:.1} S={:.1} GP={:.1} O={:.1} ({} votes)",
                    detail.ratings.graphics,
                    detail.ratings.sound,
                    detail.ratings.gameplay,
                    detail.ratings.overall,
                    detail.ratings.votes
                );
                if !detail.verified_date.is_empty() {
                    println!("Verified: {}", detail.verified_date);
                }
                println!("\nMedia ({} version(s)):", detail.media.len());
                for (i, media) in detail.media.iter().enumerate() {
                    println!("\n  [{}] {} (disc {})", i + 1, media.version, media.disc);
                    println!("    Title: {}", media.good_title);
                    println!("    Serial: {}", media.serial);
                    if !media.verified_date.is_empty() {
                        println!("    Verified: {}", media.verified_date);
                    }
                    println!("    Formats:");
                    for fmt in &media.formats {
                        println!(
                            "      alt={} key={} label={} size={}",
                            fmt.alt,
                            fmt.key,
                            fmt.label,
                            if fmt.zipped_size_bytes > 0 {
                                format!("{} KB", fmt.zipped_size_bytes / 1024)
                            } else {
                                "N/A".to_string()
                            }
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
