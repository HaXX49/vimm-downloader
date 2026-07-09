//! Snapshot tests for parser outputs against saved HTML fixtures.
//!
//! Each test parses a fixture and compares the result against a saved JSON
//! snapshot. To refresh snapshots, run:
//!
//! ```bash
//! cargo test -p vimm-core snapshot_refresh -- --ignored
//! ```
//!
//! Then commit the updated `tests/fixtures/expected/*.json` files.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use vimm_core::model::SearchQuery;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn expected_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("expected")
            .join(name)
    }

    fn assert_snapshot(name: &str, actual: &str) {
        let path = expected_path(name);
        if !path.exists() || std::env::var("UPDATE_SNAPSHOTS").is_ok() {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, actual).unwrap();
            eprintln!("Updated snapshot: {}", path.display());
            return;
        }
        let expected = fs::read_to_string(&path).unwrap();
        assert_eq!(
            actual.trim(),
            expected.trim(),
            "Snapshot mismatch for {name}. Run with UPDATE_SNAPSHOTS=1 to regenerate."
        );
    }

    #[test]
    fn snapshot_systems() {
        let html = fs::read_to_string(fixture_path("vault_home.html")).unwrap();
        let systems = vimm_core::systems::parse(&html);
        assert_snapshot(
            "systems.json",
            &serde_json::to_string_pretty(&systems).unwrap(),
        );
    }

    #[test]
    fn snapshot_search_per_system() {
        let html = fs::read_to_string(fixture_path("nes_list.html")).unwrap();
        let query = SearchQuery {
            system: Some("NES".into()),
            q: "mario".into(),
            ..Default::default()
        };
        let results = vimm_core::search::parse(&html, &query);
        assert_snapshot(
            "search_per_system.json",
            &serde_json::to_string_pretty(&results).unwrap(),
        );
    }

    #[test]
    fn snapshot_search_all_system() {
        let html = fs::read_to_string(fixture_path("armored_core_all.html")).unwrap();
        let query = SearchQuery {
            q: "armored core".into(),
            ..Default::default()
        };
        let results = vimm_core::search::parse(&html, &query);
        assert_snapshot(
            "search_all_system.json",
            &serde_json::to_string_pretty(&results).unwrap(),
        );
    }

    #[test]
    fn snapshot_detail_single_format() {
        let html = fs::read_to_string(fixture_path("game_834.html")).unwrap();
        let detail = vimm_core::detail::parse(&html, 834).unwrap();
        assert_snapshot(
            "detail_single.json",
            &serde_json::to_string_pretty(&detail).unwrap(),
        );
    }

    #[test]
    fn snapshot_detail_multi_format() {
        let html = fs::read_to_string(fixture_path("game_7818.html")).unwrap();
        let detail = vimm_core::detail::parse(&html, 7818).unwrap();
        assert_snapshot(
            "detail_multi.json",
            &serde_json::to_string_pretty(&detail).unwrap(),
        );
    }

    /// Run with `UPDATE_SNAPSHOTS=1 cargo test snapshot_refresh -- --ignored`
    /// to regenerate all snapshots.
    #[test]
    #[ignore = "run with UPDATE_SNAPSHOTS=1 to regenerate"]
    fn snapshot_refresh() {
        snapshot_systems();
        snapshot_search_per_system();
        snapshot_search_all_system();
        snapshot_detail_single_format();
        snapshot_detail_multi_format();
    }
}
