//! Live integration tests against vimm.net.
//!
//! Run with: `cargo test --features live`
//! These tests require network access and are NOT run in CI.

#[cfg(feature = "live")]
mod live {
    use vimm_core::client::{ClientConfig, VimmClient};
    use vimm_core::model::SearchQuery;

    fn test_client() -> VimmClient {
        let cfg = ClientConfig {
            min_request_interval: std::time::Duration::from_millis(100),
            ..ClientConfig::default()
        };
        VimmClient::with_config(cfg).expect("client builds")
    }

    #[tokio::test]
    async fn live_list_systems() {
        let client = test_client();
        let systems = client.list_systems().await.expect("list_systems");
        assert!(!systems.is_empty(), "should find at least one system");
        assert!(systems.len() >= 30, "should find ~33 systems, got {}", systems.len());
    }

    #[tokio::test]
    async fn live_search_per_system() {
        let client = test_client();
        let query = SearchQuery {
            system: Some("NES".into()),
            q: "mario".into(),
            ..Default::default()
        };
        let results = client.search(&query).await.expect("search");
        assert!(!results.is_empty(), "should find at least one Mario game");
    }

    #[tokio::test]
    async fn live_search_all_system() {
        let client = test_client();
        let query = SearchQuery {
            q: "final fantasy".into(),
            ..Default::default()
        };
        let results = client.search(&query).await.expect("search");
        assert!(!results.is_empty(), "should find at least one Final Fantasy game");
    }

    #[tokio::test]
    async fn live_detail() {
        let client = test_client();
        let detail = client.detail(834).await.expect("detail");
        assert_eq!(detail.id, 834);
        assert!(!detail.title.is_empty());
        assert!(!detail.media.is_empty());
    }
}
