//! Parser for the Vault game detail page (`/vault/{id}`).
//!
//! Extracts the embedded `media` JSON array, base64-decodes `GoodTitle`,
//! parses the version/disc/format selects, and reads the metadata table.

use std::collections::HashMap;

use base64::Engine;
use regex::Regex;
use serde::Deserialize;

use crate::error::VimmError;
use crate::model::{Format, GameDetail, Media, Ratings};

/// Parse a Vault game detail page into [`GameDetail`].
///
/// # Panics
///
/// Panics if any of the hardcoded CSS selectors (`h1`, `.vaultTable tr`,
/// `td`, `#dl_format`, `#dl_format option`) are invalid (they are
/// compile-time constants and always valid).
///
/// # Errors
///
/// - [`VimmError::Parse`] if the media JSON, metadata table, or expected
///   HTML structure cannot be found or parsed.
pub fn parse(html: &str) -> Result<GameDetail, VimmError> {
    let doc = scraper::Html::parse_document(html);

    // --- Title ---
    let title = doc
        .select(&scraper::Selector::parse("h1").unwrap())
        .next()
        .map(|h| h.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    // --- Media JSON ---
    let raw_entries = extract_media_json(html)?;
    let engine = base64::engine::general_purpose::STANDARD;

    let mut media_list: Vec<Media> = Vec::new();
    for entry in &raw_entries {
        let good_title_bytes = engine
            .decode(&entry.good_title)
            .map_err(|e| VimmError::Parse(format!("base64 decode error: {e}")))?;
        let good_title = String::from_utf8(good_title_bytes)
            .map_err(|_| VimmError::Parse("GoodTitle is not valid UTF-8".into()))?;

        let formats = build_formats(entry);

        media_list.push(Media {
            id: entry.id,
            version: entry.version.clone(),
            disc: entry.sort_order,
            good_title,
            serial: entry.serial.clone(),
            verified_date: entry.verified_date.clone(),
            formats,
        });
    }

    // --- Selects (version, disc, format hint) ---
    let _selected_version = get_selected_value(html, "dl_version");
    let _selected_disc = get_selected_value(html, "disc_number");
    let has_format_select = scraper::Html::parse_document(html)
        .select(&scraper::Selector::parse("#dl_format").unwrap())
        .next()
        .is_some();

    if has_format_select {
        // Read format options from the select to enrich labels+descriptions.
        let doc = scraper::Html::parse_document(html);
        let opt_sel = scraper::Selector::parse("#dl_format option").unwrap();
        for opt in doc.select(&opt_sel) {
            let value = opt.value().attr("value").unwrap_or("");
            for format in &mut media_list.iter_mut().flat_map(|m| &mut m.formats) {
                if format.key == value {
                    format.label = opt.text().collect::<String>().trim().to_string();
                    format.description = opt.value().attr("title").unwrap_or("").to_string();
                    format.alt = 0; // corrected from Mirror order below
                }
            }
        }
    } else if let Some(entry) = raw_entries.first() {
        // Single-format: no #dl_format, synthesise from the first Mirror entry.
        if let Some(first_mirror) = entry.mirror.first() {
            for format in &mut media_list.iter_mut().flat_map(|m| &mut m.formats) {
                if format.key == *first_mirror {
                    format.label = format!(".{first_mirror}");
                }
            }
        }
    }

    // --- Metadata table ---
    let meta = parse_metadata_table(html);

    let region = meta.get("Region").cloned().unwrap_or_default();
    let players = meta.get("Players").map_or(1, |s| parse_players(s));
    let year = meta
        .get("Year")
        .and_then(|s| s.trim().parse::<u16>().ok())
        .unwrap_or(0);
    let publisher = meta.get("Publisher").cloned().unwrap_or_default();
    let serial = meta.get("Serial").cloned().unwrap_or_default();
    // Simultaneous is not in the standard table; default to false.
    let simultaneous = false;

    let ratings = meta
        .get("Ratings")
        .map(|s| parse_ratings(s))
        .transpose()?
        .unwrap_or(Ratings {
            graphics: 0.0,
            sound: 0.0,
            gameplay: 0.0,
            overall: 0.0,
            votes: 0,
        });

    let verified_date = meta.get("Verified").cloned().unwrap_or_default();

    // Rebuild alt indexes from the Mirror order in the first entry.
    if let Some(first) = raw_entries.first() {
        for format in &mut media_list.iter_mut().flat_map(|m| &mut m.formats) {
            if let Some(pos) = first.mirror.iter().position(|k| k == &format.key) {
                format.alt = u8::try_from(pos).unwrap_or(0);
            }
        }
    }

    // Pick the id from the first media entry.
    let id = raw_entries.first().map_or(0, |e| e.id);

    Ok(GameDetail {
        id,
        system: String::new(), // system not available on detail page in v1
        title,
        region,
        players,
        simultaneous,
        year,
        publisher,
        serial,
        ratings,
        verified_date,
        media: media_list,
    })
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct RawMediaEntry {
    #[serde(rename = "ID")]
    id: u32,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "SortOrder")]
    sort_order: u32,
    #[serde(rename = "GoodTitle")]
    good_title: String,
    #[serde(rename = "Serial")]
    serial: String,
    #[serde(rename = "VerifiedDate")]
    verified_date: String,
    #[serde(rename = "Mirror")]
    mirror: Vec<String>,
    #[serde(rename = "Zipped")]
    zipped: Option<Vec<u64>>,
    #[serde(rename = "AltZipped")]
    alt_zipped: Option<Vec<u64>>,
    #[serde(rename = "AltZipped2")]
    alt_zipped2: Option<Vec<u64>>,
}

/// Extract the `let media=[…]` JSON array from the page HTML.
fn extract_media_json(html: &str) -> Result<Vec<RawMediaEntry>, VimmError> {
    let start = html
        .find("media=")
        .ok_or_else(|| VimmError::Parse("media JSON not found".into()))?;
    let after_start = &html[start..];
    let arr_start = after_start
        .find('[')
        .ok_or_else(|| VimmError::Parse("media array start not found".into()))?;
    let arr = &after_start[arr_start..];

    let mut depth = 0u32;
    let mut end = 0;
    for (i, ch) in arr.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(VimmError::Parse("unbalanced media JSON array".into()));
    }

    let json_str = &arr[..end];
    serde_json::from_str::<Vec<RawMediaEntry>>(json_str).map_err(VimmError::from)
}

/// Build `Format` entries from a raw media entry.
fn build_formats(entry: &RawMediaEntry) -> Vec<Format> {
    let zipped = entry.zipped.as_deref().unwrap_or(&[]);

    entry
        .mirror
        .iter()
        .enumerate()
        .map(|(i, key)| Format {
            key: key.clone(),
            label: format!(".{key}"),
            description: String::new(),
            alt: u8::try_from(i).unwrap_or(0),
            zipped_size_bytes: zipped.get(i).copied().unwrap_or(0),
        })
        .collect()
}

/// Get the `value` attribute of the first `<option selected>` in a `<select>`.
fn get_selected_value(html: &str, select_id: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(html);
    // Build a selector for e.g. #dl_version option[selected]
    let sel = scraper::Selector::parse(&format!("#{select_id} option[selected]")).ok()?;
    doc.select(&sel)
        .next()
        .and_then(|opt| opt.value().attr("value"))
        .map(ToString::to_string)
}

/// Parse the `.vaultTable` metadata into a key–value map.
fn parse_metadata_table(html: &str) -> HashMap<String, String> {
    let doc = scraper::Html::parse_document(html);
    let Ok(row_sel) = scraper::Selector::parse(".vaultTable tr") else {
        return HashMap::new();
    };
    let Ok(td_sel) = scraper::Selector::parse("td") else {
        return HashMap::new();
    };

    doc.select(&row_sel)
        .filter_map(|row| {
            let mut cells = row.select(&td_sel);
            let key = cells
                .next()?
                .text()
                .collect::<String>()
                .trim()
                .trim_end_matches(':')
                .to_string();
            let val = cells.next()?.text().collect::<String>().trim().to_string();
            Some((key, val))
        })
        .collect()
}

/// Parse ratings text like `"Graphics: 7.2, Sound: 8.3, Gameplay: 9.1, Overall: 8.8 (145 votes)"`.
fn parse_ratings(text: &str) -> Result<Ratings, VimmError> {
    let re =
        Regex::new(r"Graphics:\s*([\d.]+),\s*Sound:\s*([\d.]+),\s*Gameplay:\s*([\d.]+),\s*Overall:\s*([\d.]+)\s*\((\d+)").map_err(|e| VimmError::Parse(e.to_string()))?;
    let caps = re
        .captures(text)
        .ok_or_else(|| VimmError::Parse("unexpected ratings format".into()))?;
    Ok(Ratings {
        graphics: caps[1].parse().unwrap_or(0.0),
        sound: caps[2].parse().unwrap_or(0.0),
        gameplay: caps[3].parse().unwrap_or(0.0),
        overall: caps[4].parse().unwrap_or(0.0),
        votes: caps[5].parse().unwrap_or(0),
    })
}

/// Parse a players string (`"1-2"` → `2`, `"1"` → `1`, `"1-4"` → `4`).
fn parse_players(text: &str) -> u32 {
    text.split(&['-', '–', '—', '/', ','][..])
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .max()
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_format_detail() {
        let html = include_str!("../../../tests/fixtures/game_834.html");
        let detail = parse(html).expect("should parse");

        assert_eq!(detail.title, "Super Mario Bros.");
        assert_eq!(detail.region, "USA");
        assert_eq!(detail.players, 2);
        assert_eq!(detail.year, 1985);
        assert_eq!(detail.publisher, "Nintendo");
        assert_eq!(detail.serial, "NES-SM-USA");
        assert_eq!(detail.verified_date, "2024-01-01");
        assert!((detail.ratings.graphics - 7.2).abs() < 0.01);
        assert!((detail.ratings.sound - 8.3).abs() < 0.01);
        assert!((detail.ratings.gameplay - 9.1).abs() < 0.01);
        assert!((detail.ratings.overall - 8.8).abs() < 0.01);
        assert_eq!(detail.ratings.votes, 145);

        // Single media entry, single format.
        assert_eq!(detail.media.len(), 1);
        let m = &detail.media[0];
        assert_eq!(m.version, "1.0");
        assert_eq!(m.good_title, "Super Mario Bros. (USA)");
        assert_eq!(m.formats.len(), 1);
        assert_eq!(m.formats[0].key, "nes");
        assert_eq!(m.formats[0].alt, 0);
        assert_eq!(m.formats[0].zipped_size_bytes, 21_874);
    }

    #[test]
    fn parses_multi_format_detail() {
        let html = include_str!("../../../tests/fixtures/game_7818.html");
        let detail = parse(html).expect("should parse");

        assert_eq!(detail.title, "Armored Core");
        assert_eq!(detail.region, "USA");
        assert_eq!(detail.players, 1);
        assert_eq!(detail.year, 1997);
        assert_eq!(detail.publisher, "FromSoftware");
        assert_eq!(detail.serial, "SLUS-00001");
        assert!((detail.ratings.overall - 8.0).abs() < 0.01);
        assert_eq!(detail.ratings.votes, 230);

        // Two media entries (1.0, 1.1).
        assert_eq!(detail.media.len(), 2, "should have two media entries");

        // First media: version 1.0, three formats.
        let m0 = &detail.media[0];
        assert_eq!(m0.version, "1.0");
        assert_eq!(m0.good_title, "Armored Core (USA).iso");
        assert_eq!(m0.formats.len(), 3);
        assert_eq!(m0.formats[0].key, "ciso");
        assert_eq!(m0.formats[0].alt, 0);
        assert_eq!(m0.formats[0].zipped_size_bytes, 350_000);
        assert_eq!(m0.formats[1].key, "nkit.iso");
        assert_eq!(m0.formats[1].alt, 1);
        assert_eq!(m0.formats[1].zipped_size_bytes, 320_000);
        assert_eq!(m0.formats[2].key, "rvz");
        assert_eq!(m0.formats[2].alt, 2);
        assert_eq!(m0.formats[2].zipped_size_bytes, 310_000);

        // Second media: version 1.1, three formats.
        let m1 = &detail.media[1];
        assert_eq!(m1.version, "1.1");
        assert_eq!(m1.good_title, "Armored Core (USA) (v1.1).iso");
        assert_eq!(m1.formats.len(), 3);
    }

    #[test]
    fn missing_media_json_returns_error() {
        let html = "<html><body>no media</body></html>";
        let result = parse(html);
        assert!(result.is_err());
    }
}
