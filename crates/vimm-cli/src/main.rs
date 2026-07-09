//! vimm-cli: command-line frontend for the vimm-downloader.

use clap::{Parser, Subcommand};
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
            eprintln!(
                "not yet implemented — see issue #5 (query={:?}, system={:?})",
                args.query, args.system
            );
        }
        Command::Info { id } => {
            eprintln!("not yet implemented — see issue #6 (id={id})");
        }
        Command::Download(args) => {
            let out = std::path::Path::new(&args.out);
            std::fs::create_dir_all(out)?;

            let client = VimmClient::new()?;

            let detail = client.detail(args.id).await?;
            if detail.media.is_empty() {
                anyhow::bail!("No downloadable media found for game {}", args.id);
            }

            let media = &detail.media[0];
            if media.formats.is_empty() {
                anyhow::bail!("No formats available for media {}", media.id);
            }

            let fmt = &media.formats[0];
            eprintln!(
                "Downloading: {} ({} format: {})",
                media.good_title, media.version, fmt.key
            );

            let progress_bar = if cli.json {
                None
            } else {
                let bar = indicatif::ProgressBar::new(
                    fmt.zipped_size_bytes,
                );
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
                    }) as Box<dyn FnMut(u64, Option<u64>) + Send>)
                },
            )
            .await?;

            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "game_id": args.id,
                        "media_id": media.id,
                        "version": media.version,
                        "format": fmt.key,
                        "path": path,
                    }))?
                );
            } else {
                if let Some(bar) = progress_bar {
                    bar.finish_with_message(format!("Downloaded to {}", path.display()));
                } else {
                    eprintln!("Downloaded to {}", path.display());
                }
            }
        }
    }

    Ok(())
}
