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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn op_renders_site_param() {
        assert_eq!(Op::Ge.as_param(), ">=");
        assert_eq!(Op::Eq.as_param(), "=");
        assert_eq!(Op::Lt.as_param(), "<");
    }

    #[test]
    fn sort_renders_site_param() {
        assert_eq!(Sort::Title.as_param(), "Title");
        assert_eq!(Sort::Rating.as_param(), "Rating");
    }

    #[test]
    fn order_renders_site_param() {
        assert_eq!(Order::Asc.as_param(), "ASC");
        assert_eq!(Order::Desc.as_param(), "DESC");
    }
}
