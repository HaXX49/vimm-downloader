//! Download streaming for dl3.vimm.net.
//!
//! Streams ROM downloads to disk with progress reporting. Uses GET requests
//! with Referer header (per spike #9 findings).

use std::path::{Path, PathBuf};

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::client::VimmClient;
use crate::error::VimmError;

/// Callback signature for download progress updates.
/// Receives `(bytes_downloaded, total_bytes)` where `total_bytes` is `None`
/// if the server did not send `Content-Length`.
pub type ProgressCallback = Box<dyn FnMut(u64, Option<u64>) + Send>;

/// Progress reporting interval in bytes (1 MB).
const REPORT_INTERVAL: u64 = 1024 * 1024;

/// Download a ROM from dl3.vimm.net to the given destination.
///
/// # Arguments
///
/// * `client` — [`VimmClient`] with cookies already populated from visiting
///   vimm.net (the download requires the same cookie session).
/// * `media_id` — The media ID from [`GameDetail::media`], not the game ID.
/// * `alt` — Format variant index (0 = primary, 1/2 = alternates).
/// * `game_id` — The game ID for the Referer header.
/// * `dest` — Destination directory for the downloaded file.
/// * `progress` — Optional callback for progress updates.
///
/// # Returns
///
/// The path to the downloaded file on success.
///
/// # Errors
///
/// - [`VimmError::Http`] if the download request fails.
/// - [`VimmError::Io`] if writing to disk fails.
pub async fn download_rom(
    client: &VimmClient,
    media_id: u32,
    alt: u8,
    game_id: u32,
    dest: &Path,
    mut progress: Option<ProgressCallback>,
) -> Result<PathBuf, VimmError> {
    let url = format!(
        "https://dl3.vimm.net/?mediaId={media_id}&alt={alt}"
    );
    let referer = format!("https://vimm.net/vault/{game_id}");

    let resp = client.get_stream(&url, &referer).await?;
    let content_length = resp.content_length;
    let stem = format!(".download_{media_id}_{alt}");

    let tmp_path = dest.join(format!("{stem}.tmp"));
    let final_path = dest.join(format!("{stem}.archive"));

    let mut file = File::create(&tmp_path)
        .await
        .map_err(VimmError::from)?;

    let mut downloaded = 0u64;
    let mut next_report = 0u64;

    let mut resp = resp;
    while let Some(chunk) = resp.next_chunk().await? {
        file.write_all(&chunk).await.map_err(VimmError::from)?;
        downloaded += chunk.len() as u64;

        if let Some(ref mut cb) = progress {
            if downloaded >= next_report {
                cb(downloaded, content_length);
                next_report = downloaded + REPORT_INTERVAL;
            }
        }
    }

    file.flush().await.map_err(VimmError::from)?;
    drop(file);

    if let Some(ref mut cb) = progress {
        cb(downloaded, content_length);
    }

    tokio::fs::rename(&tmp_path, &final_path)
        .await
        .map_err(VimmError::from)?;

    Ok(final_path)
}
