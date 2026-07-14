//! Parser for the Vault game detail page (`/vault/{id}`).
//!
//! Extracts the embedded `media` JSON array, base64-decodes `GoodTitle`,
//! parses the version/disc/format selects, and reads the metadata table.

use std::collections::HashMap;

use base64::Engine;
use regex::Regex;
use scraper::{ElementRef, Selector};
use serde::Deserialize;

use crate::error::VimmError;
use crate::model::{Format, GameDetail, Media, Ratings};

/// Parse a Vault game detail page into [`GameDetail`].
///
/// `id` is the game ID from the URL path (`/vault/{id}`).
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
pub fn parse(html: &str, id: u32) -> Result<GameDetail, VimmError> {
    let doc = scraper::Html::parse_document(html);

    // --- Title ---
    let title = parse_title(&doc)?;
    let system = parse_system(&doc);

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
            serial: entry.serial.clone().unwrap_or_default(),
            verified_date: entry
                .good_date
                .as_ref()
                .map(|date| normalize_date(&date.date))
                .filter(|date| !date.is_empty())
                .unwrap_or_else(|| normalize_date(&entry.verified_date)),
            formats,
        });
    }

    // --- Selects (version, disc, format hint) ---
    let selected_version = get_selected_value(html, "dl_version");
    let selected_disc = get_selected_value(html, "disc_number");
    enrich_formats(html, &raw_entries, &mut media_list);

    // --- Metadata table ---
    let meta = parse_metadata_table(&doc);

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

    let ratings = parse_metadata_ratings(&meta)?;

    let serial = if serial.is_empty() {
        media_list
            .iter()
            .find_map(|media| (!media.serial.is_empty()).then(|| media.serial.clone()))
            .unwrap_or_default()
    } else {
        serial
    };
    let verified_date = meta
        .get("Verified")
        .cloned()
        .filter(|date| !date.is_empty())
        .or_else(|| {
            media_list.iter().find_map(|media| {
                (!media.verified_date.is_empty()).then(|| media.verified_date.clone())
            })
        })
        .unwrap_or_default();

    Ok(GameDetail {
        id,
        system,
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
        selected_version,
        selected_disc,
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
    #[serde(default)]
    serial: Option<String>,
    #[serde(rename = "VerifiedDate")]
    #[serde(default)]
    verified_date: String,
    #[serde(rename = "GoodDate")]
    #[serde(default)]
    good_date: Option<RawGoodDate>,
    #[serde(rename = "Mirror")]
    mirror: Vec<String>,
    #[serde(rename = "Zipped")]
    #[serde(deserialize_with = "deserialize_zipped")]
    zipped: Option<Vec<u64>>,
    #[serde(rename = "AltZipped")]
    #[allow(dead_code)]
    /// Available zipped sizes for alternative mirrors (v2 fallback).
    #[serde(deserialize_with = "deserialize_zipped")]
    alt_zipped: Option<Vec<u64>>,
    #[serde(rename = "AltZipped2")]
    #[allow(dead_code)]
    #[serde(deserialize_with = "deserialize_zipped")]
    alt_zipped2: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawGoodDate {
    date: String,
}

fn enrich_formats(html: &str, raw_entries: &[RawMediaEntry], media_list: &mut [Media]) {
    let doc = scraper::Html::parse_document(html);
    let opt_sel = scraper::Selector::parse("#dl_format option").unwrap();
    let options: Vec<_> = doc.select(&opt_sel).collect();

    if options.is_empty() {
        if let Some(first_mirror) = raw_entries.first().and_then(|entry| entry.mirror.first()) {
            for format in media_list.iter_mut().flat_map(|media| &mut media.formats) {
                if format.key == *first_mirror {
                    format.label = format!(".{first_mirror}");
                }
            }
        }
        return;
    }

    // Current pages use numeric option values (the alt index), while older
    // pages used the format key. Position is stable in both schemas.
    for (index, option) in options.into_iter().enumerate() {
        let label = option.text().collect::<String>().trim().to_string();
        let description = option.value().attr("title").unwrap_or("").to_string();
        let alt = option
            .value()
            .attr("value")
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or_else(|| u8::try_from(index).unwrap_or(0));
        let key = label.trim_start_matches('.').to_string();

        for media in &mut *media_list {
            if let Some(format) = media.formats.get_mut(index) {
                format.key.clone_from(&key);
                format.label.clone_from(&label);
                format.description.clone_from(&description);
                format.alt = alt;
            }
        }
    }
}

/// Deserialize `Zipped`/`AltZipped`/`AltZipped2` fields that may be either
/// a JSON array of numbers `[12345]`, a single number, a string, or null.
fn deserialize_zipped<'de, D>(deserializer: D) -> Result<Option<Vec<u64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct ZippedVisitor;

    impl<'de> Visitor<'de> for ZippedVisitor {
        type Value = Option<Vec<u64>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("null, array of numbers, or a single number/string")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_any(ZippedVisitor)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(val) = seq.next_element::<u64>()? {
                vec.push(val);
            }
            Ok(Some(vec))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(vec![v]))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.parse::<u64>()
                .map(|n| Some(vec![n]))
                .map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_option(ZippedVisitor)
}

/// Extract the `let media=[…]` JSON array from the page HTML.
///
/// Uses bracket-matching to find the balanced JSON array. This is not
/// string-literal-aware (a `]` inside a string value would mis-nest),
/// but in practice none of the free-text fields (`GoodTitle` is base64,
/// others are short strings) contain brackets. Verified against real
/// vimm.net pages in the #9 spike.
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
    entry
        .mirror
        .iter()
        .enumerate()
        .map(|(i, key)| Format {
            key: key.clone(),
            label: format!(".{key}"),
            description: String::new(),
            alt: u8::try_from(i).unwrap_or(0),
            zipped_size_bytes: size_kib_for_alt(entry, i).saturating_mul(1024),
        })
        .collect()
}

fn size_kib_for_alt(entry: &RawMediaEntry, alt: usize) -> u64 {
    let primary = entry.zipped.as_deref().unwrap_or(&[]);
    if primary.len() > 1 {
        return primary.get(alt).copied().unwrap_or(0);
    }

    match alt {
        0 => primary.first().copied().unwrap_or(0),
        1 => entry
            .alt_zipped
            .as_deref()
            .and_then(|sizes| sizes.first())
            .copied()
            .unwrap_or(0),
        2 => entry
            .alt_zipped2
            .as_deref()
            .and_then(|sizes| sizes.first())
            .copied()
            .unwrap_or(0),
        _ => 0,
    }
}

fn normalize_date(date: &str) -> String {
    date.split_whitespace().next().unwrap_or("").to_string()
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

fn parse_title(doc: &scraper::Html) -> Result<String, VimmError> {
    let meta_sel = Selector::parse("meta[property='og:title']").unwrap();
    if let Some(title) = doc
        .select(&meta_sel)
        .next()
        .and_then(|meta| meta.value().attr("content"))
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        return Ok(title.to_string());
    }

    let heading_sel = Selector::parse("h1").unwrap();
    if let Some(title) = doc
        .select(&heading_sel)
        .next()
        .map(|heading| heading.text().collect::<String>().trim().to_string())
        .filter(|title| !title.is_empty())
    {
        return Ok(title);
    }

    let canvas_sel = Selector::parse("canvas#canvas[data-v]").unwrap();
    let Some(encoded) = doc
        .select(&canvas_sel)
        .next()
        .and_then(|canvas| canvas.value().attr("data-v"))
    else {
        return Ok(String::new());
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| VimmError::Parse(format!("title base64 decode error: {error}")))?;
    String::from_utf8(decoded)
        .map_err(|_| VimmError::Parse("title canvas is not valid UTF-8".into()))
}

fn parse_system(doc: &scraper::Html) -> String {
    let selector = Selector::parse("#subMenu a.active[href^='/vault/']").unwrap();
    doc.select(&selector)
        .next()
        .and_then(|link| link.value().attr("href"))
        .and_then(|href| href.strip_prefix("/vault/"))
        .unwrap_or("")
        .to_string()
}

/// Parse metadata rows from both legacy and current detail tables.
fn parse_metadata_table(doc: &scraper::Html) -> HashMap<String, String> {
    let row_sel = Selector::parse("tr").unwrap();
    let flag_sel = Selector::parse("img.flag[title]").unwrap();
    let date_sel = Selector::parse("#data-date").unwrap();

    doc.select(&row_sel)
        .filter_map(|row| {
            let cells: Vec<ElementRef<'_>> = row
                .children()
                .filter_map(ElementRef::wrap)
                .filter(|child| child.value().name() == "td")
                .collect();
            let first = cells.first()?;
            let key = first
                .text()
                .collect::<String>()
                .trim()
                .trim_end_matches(':')
                .to_string();
            if key.is_empty() {
                return None;
            }

            let value = if key == "Verified" {
                row.select(&date_sel).next().map_or_else(
                    || {
                        cells.last().map_or_else(String::new, |cell| {
                            cell.text().collect::<String>().trim().to_string()
                        })
                    },
                    |date| date.text().collect::<String>().trim().to_string(),
                )
            } else if key == "Region" {
                let flags = row
                    .select(&flag_sel)
                    .filter_map(|flag| flag.value().attr("title"))
                    .collect::<Vec<_>>()
                    .join(", ");
                if flags.is_empty() {
                    cells.last()?.text().collect::<String>().trim().to_string()
                } else {
                    flags
                }
            } else {
                cells
                    .last()?
                    .text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            Some((key, value))
        })
        .collect()
}

fn parse_metadata_ratings(meta: &HashMap<String, String>) -> Result<Ratings, VimmError> {
    if let Some(combined) = meta.get("Ratings") {
        return parse_ratings(combined);
    }

    let parse_score = |key: &str| {
        meta.get(key)
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let overall_text = meta.get("Overall").map_or("", String::as_str);
    let votes_re =
        Regex::new(r"\((\d+)\s+votes?\)").map_err(|error| VimmError::Parse(error.to_string()))?;
    let votes = votes_re
        .captures(overall_text)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<u32>().ok())
        .unwrap_or(0);

    Ok(Ratings {
        graphics: parse_score("Graphics"),
        sound: parse_score("Sound"),
        gameplay: parse_score("Gameplay"),
        overall: parse_score("Overall"),
        votes,
    })
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
        let detail = parse(html, 834).expect("should parse");

        assert_eq!(detail.id, 834);
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
        assert_eq!(m.formats[0].label, ".nes");
        assert_eq!(m.formats[0].zipped_size_bytes, 21_874 * 1024);
        assert_eq!(detail.selected_version, Some("1.0".into()));
        assert_eq!(detail.selected_disc, Some("1".into()));
    }

    #[test]
    fn parses_multi_format_detail() {
        let html = include_str!("../../../tests/fixtures/game_7818.html");
        let detail = parse(html, 7818).expect("should parse");

        assert_eq!(detail.id, 7818);
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
        assert_eq!(m0.formats[0].zipped_size_bytes, 350_000 * 1024);
        assert_eq!(m0.formats[0].label, ".ciso");
        assert_eq!(m0.formats[0].description, "Compressed ISO");
        assert_eq!(m0.formats[1].key, "nkit.iso");
        assert_eq!(m0.formats[1].alt, 1);
        assert_eq!(m0.formats[1].zipped_size_bytes, 320_000 * 1024);
        assert_eq!(m0.formats[1].label, ".nkit.iso");
        assert_eq!(m0.formats[1].description, "NKit compressed ISO");
        assert_eq!(m0.formats[2].key, "rvz");
        assert_eq!(m0.formats[2].alt, 2);
        assert_eq!(m0.formats[2].zipped_size_bytes, 310_000 * 1024);
        assert_eq!(m0.formats[2].label, ".rvz");
        assert_eq!(m0.formats[2].description, "Dolphin compressed RVZ");

        // Second media: version 1.1, three formats.
        let m1 = &detail.media[1];
        assert_eq!(m1.version, "1.1");
        assert_eq!(m1.good_title, "Armored Core (USA) (v1.1).iso");
        assert_eq!(m1.formats.len(), 3);
        assert_eq!(detail.selected_version, Some("1.0".into()));
        assert_eq!(detail.selected_disc, Some("1".into()));
    }

    #[test]
    fn parses_current_single_format_detail() {
        let html = include_str!("../../../tests/fixtures/game_5625_current.html");
        let detail = parse(html, 5625).expect("should parse current detail page");

        assert_eq!(detail.title, "Pokemon: Emerald Version");
        assert_eq!(detail.system, "GBA");
        assert_eq!(detail.region, "USA, Europe");
        assert_eq!(detail.players, 1);
        assert_eq!(detail.year, 2005);
        assert!(detail.publisher.is_empty());
        assert!(detail.serial.is_empty());
        assert!((detail.ratings.graphics - 8.62).abs() < 0.01);
        assert!((detail.ratings.sound - 8.74).abs() < 0.01);
        assert!((detail.ratings.gameplay - 8.93).abs() < 0.01);
        assert!((detail.ratings.overall - 8.81).abs() < 0.01);
        assert_eq!(detail.ratings.votes, 69);
        assert_eq!(detail.verified_date, "2026-06-30");
        assert_eq!(detail.media[0].verified_date, "2026-06-30");
        assert_eq!(detail.media[0].formats[0].key, "GBA");
        assert_eq!(detail.media[0].formats[0].zipped_size_bytes, 6632 * 1024);
    }

    #[test]
    fn parses_current_multi_format_detail() {
        let html = include_str!("../../../tests/fixtures/game_7478_current.html");
        let detail = parse(html, 7478).expect("should parse current detail page");

        assert_eq!(detail.system, "GameCube");
        assert_eq!(detail.serial, "DOL-GZLE-USA");
        let formats = &detail.media[0].formats;
        assert_eq!(formats.len(), 3);
        assert_eq!(formats[0].key, "ciso");
        assert_eq!(formats[0].label, ".ciso");
        assert_eq!(formats[0].alt, 0);
        assert_eq!(formats[0].zipped_size_bytes, 730_817 * 1024);
        assert_eq!(formats[1].key, "nkit.iso");
        assert_eq!(formats[1].alt, 1);
        assert_eq!(formats[1].zipped_size_bytes, 730_267 * 1024);
        assert_eq!(formats[2].key, "rvz");
        assert_eq!(formats[2].alt, 2);
        assert_eq!(formats[2].zipped_size_bytes, 731_107 * 1024);
        assert_eq!(formats[2].description, ".rvz files only work in Dolphin");
    }

    #[test]
    fn missing_media_json_returns_error() {
        let html = "<html><body>no media</body></html>";
        let result = parse(html, 0);
        assert!(result.is_err());
    }
}
