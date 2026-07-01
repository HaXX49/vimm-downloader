//! Core data model for the Vimm's Lair Vault.
//!
//! See `DESIGN.md` for the full schema rationale.

use serde::{Deserialize, Serialize};

/// A supported console (e.g. NES, GameCube, PS1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct System {
    /// URL slug used by vimm.net (e.g. `NES`, `X360-D`).
    pub slug: String,
    /// Display name (e.g. `Nintendo`, `Xbox 360 (Digital)`).
    pub name: String,
    /// Launch year parsed from the `title="Launched …"` attribute.
    pub launch_year: u16,
}

/// Comparison operator for numeric search filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `=`
    Eq,
    /// `<`
    Lt,
    /// `<=`
    Le,
}

impl Op {
    /// Render as the literal string vimm.net expects in form params.
    #[must_use]
    pub fn as_param(self) -> &'static str {
        match self {
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Eq => "=",
            Op::Lt => "<",
            Op::Le => "<=",
        }
    }
}

/// Sort field for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum Sort {
    /// Sort by title (site default).
    #[default]
    Title,
    /// Sort by player count.
    Players,
    /// Sort by release year.
    Year,
    /// Sort by community rating.
    Rating,
}

impl Sort {
    /// Render as the literal string vimm.net expects.
    #[must_use]
    pub fn as_param(self) -> &'static str {
        match self {
            Sort::Title => "Title",
            Sort::Players => "Players",
            Sort::Year => "Year",
            Sort::Rating => "Rating",
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum Order {
    /// Ascending (site default).
    #[default]
    Asc,
    /// Descending.
    Desc,
}

impl Order {
    /// Render as the literal string vimm.net expects.
    #[must_use]
    pub fn as_param(self) -> &'static str {
        match self {
            Order::Asc => "ASC",
            Order::Desc => "DESC",
        }
    }
}

/// Search mode derived from [`SearchQuery::system`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchMode {
    /// Per-system: `mode=list`, `system={slug}`. Result table includes a Rating column.
    List,
    /// All-system: `mode=adv`, `system=` empty. Result table includes a System column, no Rating.
    Adv,
}

/// A search query against the Vault. Maps 1:1 onto vimm.net's advanced-search
/// form parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchQuery {
    /// System slug, or `None`/`Some("all")` to search across all 33 systems.
    pub system: Option<String>,
    /// Title substring (site requires `minlength=3`).
    pub q: String,
    /// Player-count filter.
    pub players: Option<(Op, u8)>,
    /// Require simultaneous multiplayer?
    pub simultaneous: Option<bool>,
    /// Publisher substring.
    pub publisher: Option<String>,
    /// Release-year filter.
    pub year: Option<(Op, u16)>,
    /// Rating filter (only meaningful in per-system mode; the all-system
    /// results table has no Rating column).
    pub rating: Option<(Op, f32)>,
    /// Region ID (numeric, from the Vault's region `<select>`).
    pub region: Option<String>,
    /// Sort field.
    pub sort: Sort,
    /// Sort direction.
    pub order: Order,
    /// Letter section (`A`..`Z` or `number`); per-system only.
    pub section: Option<String>,
}

impl SearchQuery {
    /// Derive the search mode from the `system` field.
    ///
    /// `None` or `Some("all")` → [`SearchMode::Adv`] (all-system).
    /// `Some(slug)` → [`SearchMode::List`] (per-system).
    #[must_use]
    pub fn mode(&self) -> SearchMode {
        match &self.system {
            None => SearchMode::Adv,
            Some(s) if s.eq_ignore_ascii_case("all") => SearchMode::Adv,
            Some(_) => SearchMode::List,
        }
    }
}

/// Badge flags surfaced inline in Vault result rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExtraFlag {
    /// `T` — Translated.
    Translated,
    /// `D` — Demo.
    Demo,
    /// `P` — Prototype.
    Prototype,
    /// `U` — Unlicensed.
    Unlicensed,
    /// `B` — Bonus disc.
    Bonus,
}

/// One row in a Vault search results page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSummary {
    /// Numeric game ID from `/vault/{id}`.
    pub id: u32,
    /// Game title.
    pub title: String,
    /// System slug (always populated; in per-system mode it equals the query).
    pub system: String,
    /// Region names parsed from flag `<img title="…">`.
    pub regions: Vec<String>,
    /// ROM version string (e.g. `1.0`, `2.00`).
    pub version: String,
    /// Language codes (empty when the site shows `-`).
    pub languages: Vec<String>,
    /// Inline extras badges (`T/D/P/U/B`).
    pub extras: Vec<ExtraFlag>,
    /// Community rating. `None` in all-system mode (column is absent).
    pub rating: Option<f32>,
}

/// A downloadable format variant for a [`Media`] entry.
///
/// GameCube/Wii/PS2/etc. discs ship in multiple compressed formats
/// (`.ciso`/`.nkit.iso`/`.rvz`); the `alt` field selects which one the
/// `POST dl3.vimm.net` endpoint returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Format {
    /// Format key (suffix of the site's `Mirror[]` entry, e.g. `ciso`).
    pub key: String,
    /// Label shown in the `#dl_format` `<select>` (e.g. `.ciso`).
    pub label: String,
    /// One-line description from the `<option title="…">` / format dialog.
    pub description: String,
    /// The `alt` POST field value (0/1/2) that selects this format.
    pub alt: u8,
    /// Zipped download size in bytes (`Zipped`/`AltZipped`/`AltZipped2` by index).
    pub zipped_size_bytes: u64,
}

/// One version/disc of a game, with its available formats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Media {
    /// `mediaId` for the `POST dl3.vimm.net` request.
    pub id: u32,
    /// Version string (e.g. `1.0`, `1.2`).
    pub version: String,
    /// Disc number (1-based; `SortOrder` in the site's JSON).
    pub disc: u32,
    /// Decoded canonical `GoodTitle` (e.g. `Super Smash Bros. Melee (USA) (En,Ja).iso`).
    pub good_title: String,
    /// Serial number (e.g. `DOL-GALE-USA`).
    pub serial: String,
    /// No-Intro verification date (`YYYY-MM-DD`).
    pub verified_date: String,
    /// Available format variants (always ≥1).
    pub formats: Vec<Format>,
}

/// Full detail page for a single game (`GET /vault/{id}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameDetail {
    /// Numeric game ID.
    pub id: u32,
    /// System slug.
    pub system: String,
    /// Display title (plain HTML, not base64).
    pub title: String,
    /// Primary region (from metadata table).
    pub region: String,
    /// Player count.
    pub players: u32,
    /// Whether multiplayer is simultaneous.
    pub simultaneous: bool,
    /// Release year.
    pub year: u16,
    /// Publisher.
    pub publisher: String,
    /// Serial number.
    pub serial: String,
    /// Community ratings: `(graphics, sound, gameplay, overall)`.
    pub ratings: Ratings,
    /// No-Intro verification date.
    pub verified_date: String,
    /// All version/disc/format combinations.
    pub media: Vec<Media>,
    /// The site's pre-selected version (from `#dl_version option[selected]`).
    #[serde(default)]
    pub selected_version: Option<String>,
    /// The site's pre-selected disc number (from `#disc_number option[selected]`).
    #[serde(default)]
    pub selected_disc: Option<String>,
}

/// Community ratings breakdown.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ratings {
    /// Graphics score (0-10).
    pub graphics: f32,
    /// Sound score (0-10).
    pub sound: f32,
    /// Gameplay score (0-10).
    pub gameplay: f32,
    /// Overall score (0-10).
    pub overall: f32,
    /// Number of votes contributing to `overall`.
    pub votes: u32,
}

// ---------------------------------------------------------------------------
// Convenience methods needed by downstream issues (#3 HTTP, #5 search parser)
// ---------------------------------------------------------------------------

impl SearchQuery {
    /// Serialize the query into the `(key, value)` form params vimm.net expects.
    ///
    /// Always includes `p=list`. Adds `mode=adv` + empty `system` for
    /// all-system searches, or `mode=list` + `system={slug}` for per-system.
    #[must_use]
    pub fn to_params(&self) -> Vec<(String, String)> {
        let mut params = vec![
            ("p".to_string(), "list".to_string()),
            ("mode".to_string(), self.mode().as_param().to_string()),
        ];

        match &self.system {
            None => params.push(("system".to_string(), String::new())),
            Some(s) if s.eq_ignore_ascii_case("all") => {
                params.push(("system".to_string(), String::new()));
            }
            Some(slug) => params.push(("system".to_string(), slug.clone())),
        }

        params.push(("q".to_string(), self.q.clone()));

        if let Some((op, val)) = self.players {
            params.push(("players".to_string(), op.as_param().to_string()));
            params.push(("playersValue".to_string(), val.to_string()));
        }
        if let Some(sim) = self.simultaneous {
            params.push(("simultaneous".to_string(), sim.to_string()));
        }
        if let Some(pub_) = &self.publisher {
            params.push(("publisher".to_string(), pub_.clone()));
        }
        if let Some((op, val)) = self.year {
            params.push(("year".to_string(), op.as_param().to_string()));
            params.push(("yearValue".to_string(), val.to_string()));
        }
        if let Some((op, val)) = self.rating {
            params.push(("rating".to_string(), op.as_param().to_string()));
            params.push(("ratingValue".to_string(), val.to_string()));
        }
        if let Some(region) = &self.region {
            params.push(("region".to_string(), region.clone()));
        }

        params.push(("sort".to_string(), self.sort.as_param().to_string()));
        params.push(("sortOrder".to_string(), self.order.as_param().to_string()));

        if let Some(section) = &self.section {
            params.push(("section".to_string(), section.clone()));
        }

        params
    }
}

impl SearchMode {
    /// Render as the literal `mode` form param value.
    #[must_use]
    pub fn as_param(self) -> &'static str {
        match self {
            SearchMode::List => "list",
            SearchMode::Adv => "adv",
        }
    }
}

impl ExtraFlag {
    /// Parse a single badge letter (`T/D/P/U/B`) into an [`ExtraFlag`].
    ///
    /// Returns `None` for unrecognized letters — the caller decides whether
    /// to ignore or error.
    #[must_use]
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_uppercase() {
            'T' => Some(Self::Translated),
            'D' => Some(Self::Demo),
            'P' => Some(Self::Prototype),
            'U' => Some(Self::Unlicensed),
            'B' => Some(Self::Bonus),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- mode() ---------------------------------------------------------------

    #[test]
    fn mode_is_adv_when_system_none() {
        let q = SearchQuery::default();
        assert_eq!(q.mode(), SearchMode::Adv);
    }

    #[test]
    fn mode_is_adv_when_system_all_case_insensitive() {
        let q = SearchQuery {
            system: Some("ALL".into()),
            ..Default::default()
        };
        assert_eq!(q.mode(), SearchMode::Adv);
    }

    #[test]
    fn mode_is_list_when_system_slug() {
        let q = SearchQuery {
            system: Some("NES".into()),
            ..Default::default()
        };
        assert_eq!(q.mode(), SearchMode::List);
    }

    // -- as_param() -----------------------------------------------------------

    #[test]
    fn op_renders_site_param() {
        assert_eq!(Op::Gt.as_param(), ">");
        assert_eq!(Op::Ge.as_param(), ">=");
        assert_eq!(Op::Eq.as_param(), "=");
        assert_eq!(Op::Lt.as_param(), "<");
        assert_eq!(Op::Le.as_param(), "<=");
    }

    #[test]
    fn sort_renders_site_param() {
        assert_eq!(Sort::Title.as_param(), "Title");
        assert_eq!(Sort::Players.as_param(), "Players");
        assert_eq!(Sort::Year.as_param(), "Year");
        assert_eq!(Sort::Rating.as_param(), "Rating");
    }

    #[test]
    fn order_renders_site_param() {
        assert_eq!(Order::Asc.as_param(), "ASC");
        assert_eq!(Order::Desc.as_param(), "DESC");
    }

    #[test]
    fn search_mode_renders_param() {
        assert_eq!(SearchMode::List.as_param(), "list");
        assert_eq!(SearchMode::Adv.as_param(), "adv");
    }

    // -- to_params() ----------------------------------------------------------

    #[test]
    fn to_params_all_system_includes_mode_adv_and_empty_system() {
        let q = SearchQuery {
            q: "armored core".into(),
            ..Default::default()
        };
        let params = q.to_params();
        assert!(params.contains(&("p".into(), "list".into())));
        assert!(params.contains(&("mode".into(), "adv".into())));
        assert!(params.contains(&("system".into(), String::new())));
        assert!(params.contains(&("q".into(), "armored core".into())));
    }

    #[test]
    fn to_params_per_system_includes_mode_list_and_slug() {
        let q = SearchQuery {
            system: Some("NES".into()),
            q: "mario".into(),
            ..Default::default()
        };
        let params = q.to_params();
        assert!(params.contains(&("mode".into(), "list".into())));
        assert!(params.contains(&("system".into(), "NES".into())));
    }

    #[test]
    fn to_params_includes_optional_filters_when_set() {
        let q = SearchQuery {
            system: Some("PS1".into()),
            q: "final fantasy".into(),
            players: Some((Op::Ge, 2)),
            simultaneous: Some(true),
            publisher: Some("Square".into()),
            year: Some((Op::Ge, 1997)),
            rating: Some((Op::Ge, 8.0)),
            region: Some("8".into()),
            sort: Sort::Rating,
            order: Order::Desc,
            section: Some("number".into()),
        };
        let params = q.to_params();
        assert!(params.contains(&("players".into(), ">=".into())));
        assert!(params.contains(&("playersValue".into(), "2".into())));
        assert!(params.contains(&("simultaneous".into(), "true".into())));
        assert!(params.contains(&("publisher".into(), "Square".into())));
        assert!(params.contains(&("year".into(), ">=".into())));
        assert!(params.contains(&("yearValue".into(), "1997".into())));
        assert!(params.contains(&("rating".into(), ">=".into())));
        assert!(params.contains(&("ratingValue".into(), "8".into())));
        assert!(params.contains(&("region".into(), "8".into())));
        assert!(params.contains(&("sort".into(), "Rating".into())));
        assert!(params.contains(&("sortOrder".into(), "DESC".into())));
        assert!(params.contains(&("section".into(), "number".into())));
    }

    #[test]
    fn to_params_omits_optional_filters_when_none() {
        let q = SearchQuery {
            system: Some("NES".into()),
            q: "zelda".into(),
            ..Default::default()
        };
        let params = q.to_params();
        assert!(!params.iter().any(|(k, _)| k == "players"));
        assert!(!params.iter().any(|(k, _)| k == "publisher"));
        assert!(!params.iter().any(|(k, _)| k == "year"));
        assert!(!params.iter().any(|(k, _)| k == "rating"));
        assert!(!params.iter().any(|(k, _)| k == "region"));
        assert!(!params.iter().any(|(k, _)| k == "section"));
    }

    // -- ExtraFlag::from_char -------------------------------------------------

    #[test]
    fn extra_flag_from_char_parses_all_valid_letters() {
        assert_eq!(ExtraFlag::from_char('T'), Some(ExtraFlag::Translated));
        assert_eq!(ExtraFlag::from_char('D'), Some(ExtraFlag::Demo));
        assert_eq!(ExtraFlag::from_char('P'), Some(ExtraFlag::Prototype));
        assert_eq!(ExtraFlag::from_char('U'), Some(ExtraFlag::Unlicensed));
        assert_eq!(ExtraFlag::from_char('B'), Some(ExtraFlag::Bonus));
    }

    #[test]
    fn extra_flag_from_char_is_case_insensitive() {
        assert_eq!(ExtraFlag::from_char('t'), Some(ExtraFlag::Translated));
        assert_eq!(ExtraFlag::from_char('d'), Some(ExtraFlag::Demo));
    }

    #[test]
    fn extra_flag_from_char_returns_none_for_unknown() {
        assert_eq!(ExtraFlag::from_char('X'), None);
        assert_eq!(ExtraFlag::from_char('1'), None);
        assert_eq!(ExtraFlag::from_char(' '), None);
    }

    // -- serde round-trip -----------------------------------------------------

    #[test]
    fn system_serializes_and_deserializes() {
        let s = System {
            slug: "X360-D".into(),
            name: "Xbox 360 (Digital)".into(),
            launch_year: 2005,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: System = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn game_summary_with_optional_rating_serializes() {
        let g = GameSummary {
            id: 834,
            title: "Super Mario Bros.".into(),
            system: "NES".into(),
            regions: vec!["World".into()],
            version: "1.0".into(),
            languages: vec![],
            extras: vec![ExtraFlag::Translated],
            rating: Some(8.85),
        };
        let json = serde_json::to_string(&g).unwrap();
        let back: GameSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(g, back);
    }

    #[test]
    fn game_summary_with_null_rating_serializes() {
        let g = GameSummary {
            id: 51455,
            title: "Armored Core".into(),
            system: "PS1".into(),
            regions: vec!["Japan".into()],
            version: "1.1".into(),
            languages: vec![],
            extras: vec![],
            rating: None,
        };
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("\"rating\":null"));
        let back: GameSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(g, back);
    }

    #[test]
    fn format_serializes_with_all_fields() {
        let f = Format {
            key: "ciso".into(),
            label: ".ciso".into(),
            description: "Works on hardware and emulators (Dolphin)".into(),
            alt: 0,
            zipped_size_bytes: 961_898,
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: Format = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }
}
