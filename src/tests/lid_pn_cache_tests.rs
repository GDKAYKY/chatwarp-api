    use super::*;

    #[tokio::test]
    async fn test_basic_operations() {
        let cache = LidPnCache::new();

        // Initially empty
        assert!(cache.get_current_lid("559980000001").await.is_none());
        assert!(cache.get_phone_number("100000012345678").await.is_none());

        // Add a mapping
        let entry = LidPnEntry::new(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            LearningSource::Usync,
        );
        cache.add(entry).await;

        // Should be retrievable both ways
        assert_eq!(
            cache.get_current_lid("559980000001").await,
            Some("100000012345678".to_string())
        );
        assert_eq!(
            cache.get_phone_number("100000012345678").await,
            Some("559980000001".to_string())
        );
    }

    #[tokio::test]
    async fn test_timestamp_conflict_resolution() {
        let cache = LidPnCache::new();

        // Add old mapping
        let old_entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Other,
        );
        cache.add(old_entry).await;

        assert_eq!(
            cache.get_current_lid("559980000001").await,
            Some("100000012345678".to_string())
        );

        // Add newer mapping for same phone (different LID)
        let new_entry = LidPnEntry::with_timestamp(
            "100000087654321".to_string(),
            "559980000001".to_string(),
            2000,
            LearningSource::Usync,
        );
        cache.add(new_entry).await;

        // Should return the newer LID for PN lookup
        assert_eq!(
            cache.get_current_lid("559980000001").await,
            Some("100000087654321".to_string())
        );

        // Both LIDs should still be in the LID -> Entry map
        assert_eq!(
            cache.get_phone_number("100000012345678").await,
            Some("559980000001".to_string())
        );
        assert_eq!(
            cache.get_phone_number("100000087654321").await,
            Some("559980000001".to_string())
        );
    }

    #[tokio::test]
    async fn test_older_entry_does_not_override() {
        let cache = LidPnCache::new();

        // Add new mapping first
        let new_entry = LidPnEntry::with_timestamp(
            "100000087654321".to_string(),
            "559980000001".to_string(),
            2000,
            LearningSource::Usync,
        );
        cache.add(new_entry).await;

        // Try to add older mapping
        let old_entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Other,
        );
        cache.add(old_entry).await;

        // PN -> LID should still return the newer one
        assert_eq!(
            cache.get_current_lid("559980000001").await,
            Some("100000087654321".to_string())
        );
    }

    #[tokio::test]
    async fn test_warm_up() {
        let cache = LidPnCache::new();

        let entries = vec![
            LidPnEntry::with_timestamp(
                "lid1".to_string(),
                "pn1".to_string(),
                1,
                LearningSource::Other,
            ),
            LidPnEntry::with_timestamp(
                "lid2".to_string(),
                "pn2".to_string(),
                2,
                LearningSource::Usync,
            ),
            LidPnEntry::with_timestamp(
                "lid3".to_string(),
                "pn3".to_string(),
                3,
                LearningSource::PeerPnMessage,
            ),
        ];

        cache.warm_up(entries).await;

        assert_eq!(cache.lid_count().await, 3);
        assert_eq!(cache.pn_count().await, 3);

        assert_eq!(cache.get_current_lid("pn1").await, Some("lid1".to_string()));
        assert_eq!(cache.get_current_lid("pn2").await, Some("lid2".to_string()));
        assert_eq!(cache.get_current_lid("pn3").await, Some("lid3".to_string()));
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = LidPnCache::new();

        let entry = LidPnEntry::new(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            LearningSource::Usync,
        );
        cache.add(entry).await;

        assert_eq!(cache.lid_count().await, 1);
        assert_eq!(cache.pn_count().await, 1);

        cache.clear().await;

        assert_eq!(cache.lid_count().await, 0);
        assert_eq!(cache.pn_count().await, 0);
        assert!(cache.get_current_lid("559980000001").await.is_none());
    }

    #[test]
    fn test_learning_source_serialization() {
        let sources = [
            (LearningSource::Usync, "usync"),
            (LearningSource::PeerPnMessage, "peer_pn_message"),
            (LearningSource::PeerLidMessage, "peer_lid_message"),
            (LearningSource::RecipientLatestLid, "recipient_latest_lid"),
            (LearningSource::MigrationSyncLatest, "migration_sync_latest"),
            (LearningSource::MigrationSyncOld, "migration_sync_old"),
            (LearningSource::BlocklistActive, "blocklist_active"),
            (LearningSource::BlocklistInactive, "blocklist_inactive"),
            (LearningSource::Pairing, "pairing"),
            (LearningSource::Other, "other"),
        ];

        for (source, expected_str) in sources {
            assert_eq!(source.as_str(), expected_str);
            assert_eq!(LearningSource::parse(expected_str), source);
        }

        // Unknown string should map to Other
        assert_eq!(LearningSource::parse("unknown"), LearningSource::Other);
    }
