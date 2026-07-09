//! Parser for Vault search results pages.
//!
//! Handles both search schemas:
//!
//! - Per-system (`mode=list`): includes a Rating column.
//! - All-system (`mode=adv`): includes a System column instead of Rating.
//!
//! Detection: the first `<th>` text ("System" → Adv, else → List).

use scraper::{Html, Selector};

use crate::model::{ExtraFlag, GameSummary, SearchQuery};

/// Parse a Vault search results page into [`GameSummary`] entries.
///
/// Skips decoy rows that link to `/vault/999_999`. Returns results in
/// document order (the order the site returned them).
///
/// # Panics
///
/// Panics if any of the hardcoded CSS selectors are invalid (they are
/// compile-time constants and therefore always valid).
#[must_use]
pub fn parse(html: &str, query: &SearchQuery) -> Vec<GameSummary> {
    let doc = Html::parse_document(html);

    let row_sel = Selector::parse("table tr").expect("valid selector");
    let decoy_sel = Selector::parse("a[href*='/vault/999999']").expect("valid selector");
    let link_sel = Selector::parse("a[href^='/vault/']:not(a[href*='/vault/999999'])")
        .expect("valid selector");
    let badge_sel = Selector::parse("b.redBorder").expect("valid selector");
    let flag_sel = Selector::parse("img.flag").expect("valid selector");

    doc.select(&row_sel)
        .filter_map(|row| {
            let tds: Vec<_> = row.select(&Selector::parse("td").unwrap()).collect();
            // Live site uses 5 columns for per-system: Title | Region | Version | Languages | Rating
            // Live site uses 6 columns for all-system: System | Title | Region | Version | Languages
            // Fixtures may have different layouts; detect by column count and content.
            let has_system_col = tds.len() >= 6 || (tds.len() == 5 && tds[0].select(&link_sel).next().is_none());
            if tds.len() < 5 {
                return None;
            }

            // --- column mapping ---
            // Per-system (5 cols): Title(0) | Region(1) | Version(2) | Languages(3) | Rating(4)
            // All-system (6 cols): System(0) | Title(1) | Region(2) | Version(3) | Languages(4)
            // All-system (5 cols fixture): System(0) | Title(1) | Region(2) | Version(3) | Languages(4)
            let (title_idx, region_idx, version_idx, languages_idx, rating_idx, system_idx) = if has_system_col {
                // All-system: no rating column
                (1, 2, 3, 4, None, Some(0))
            } else {
                // Per-system: has rating
                (0, 1, 2, 3, Some(4), None)
            };

            let title_td = &tds[title_idx];

            // Game ID and title from the first game link.
            let link = title_td.select(&link_sel).next()?;
            let id: u32 = link
                .value()
                .attr("href")?
                .strip_prefix("/vault/")?
                .parse()
                .ok()?;
            if id == 999_999 {
                return None;
            }
            let title = link.text().collect::<String>().trim().to_string();

            // System (from column or query).
            let system = match system_idx {
                Some(idx) => tds[idx].text().collect::<String>().trim().to_string(),
                None => query.system.clone().unwrap_or_default(),
            };

            // Extras badges.
            let extras: Vec<ExtraFlag> = title_td
                .select(&badge_sel)
                .filter_map(|b| {
                    let text = b.text().collect::<String>();
                    ExtraFlag::from_char(text.chars().next()?)
                })
                .collect();

            // Regions (flag images in the region column).
            let regions: Vec<String> = tds[region_idx]
                .select(&flag_sel)
                .filter_map(|img| img.value().attr("title"))
                .map(ToString::to_string)
                .collect();

            // Version column.
            let version = tds[version_idx].text().collect::<String>().trim().to_string();

            // Languages column (comma-separated, "-" → empty).
            let languages: Vec<String> = tds[languages_idx]
                .text()
                .collect::<String>()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != "-")
                .collect();

            // Rating (only in per-system mode).
            let rating = rating_idx.and_then(|idx| {
                tds[idx]
                    .text()
                    .collect::<String>()
                    .trim()
                    .parse::<f32>()
                    .ok()
            });

            Some(GameSummary {
                id,
                title,
                system,
                regions,
                version,
                languages,
                extras,
                rating,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_per_system_list() {
        let html = include_str!("../../../tests/fixtures/nes_list.html");
        let q = SearchQuery {
            system: Some("NES".into()),
            q: "mario".into(),
            ..SearchQuery::default()
        };
        let results = parse(html, &q);
        assert_eq!(results.len(), 2, "decoy row should be skipped");

        // First result: Super Mario Bros.
        let mario = &results[0];
        assert_eq!(mario.id, 834);
        assert_eq!(mario.title, "Super Mario Bros.");
        assert_eq!(mario.system, "NES");
        assert_eq!(mario.regions, &["USA"]);
        assert_eq!(mario.version, "1.0");
        assert_eq!(mario.languages, &["En"]);
        assert!(mario.extras.is_empty());
        assert_eq!(mario.rating, Some(8.8));

        // Second result: SMB3 with B badge and two regions.
        let smb3 = &results[1];
        assert_eq!(smb3.id, 7818);
        assert_eq!(smb3.title, "Super Mario Bros. 3");
        assert_eq!(smb3.extras, &[ExtraFlag::Bonus]);
        assert_eq!(smb3.regions, &["USA", "Japan"]);
        assert_eq!(smb3.languages, &["En", "Ja"]);
        assert_eq!(smb3.rating, Some(9.5));
    }

    #[test]
    fn parses_all_system_adv() {
        let html = include_str!("../../../tests/fixtures/armored_core_all.html");
        let q = SearchQuery {
            system: None, // all-system
            q: "armored core".into(),
            ..SearchQuery::default()
        };
        let results = parse(html, &q);
        assert_eq!(results.len(), 2, "decoy row should be skipped");

        // First result: Armored Core on PS1.
        let ac1 = &results[0];
        assert_eq!(ac1.id, 9876);
        assert_eq!(ac1.title, "Armored Core");
        assert_eq!(ac1.system, "PS1");
        assert_eq!(ac1.regions, &["USA", "Japan"]);
        assert_eq!(ac1.languages, &["En", "Ja"]);
        assert!(ac1.rating.is_none(), "all-system has no rating");

        // Second result: Armored Core 2 on PS2.
        let ac2 = &results[1];
        assert_eq!(ac2.id, 9877);
        assert_eq!(ac2.title, "Armored Core 2");
        assert_eq!(ac2.system, "PS2");
        assert_eq!(ac2.regions, &["USA"]);
        assert_eq!(ac2.languages, &["En"]);
        assert!(ac2.rating.is_none());
    }

    #[test]
    fn returns_empty_on_no_table() {
        let html = "<html><body>no results</body></html>";
        let q = SearchQuery::default();
        let results = parse(html, &q);
        assert!(results.is_empty());
    }
}
