    use super::*;
    use warp_core_binary::jid::Jid;

    #[test]
    fn test_spam_flow_as_str() {
        assert_eq!(SpamFlow::MessageMenu.as_str(), "MessageMenu");
        assert_eq!(
            SpamFlow::GroupSpamBannerReport.as_str(),
            "GroupSpamBannerReport"
        );
        assert_eq!(SpamFlow::ContactInfo.as_str(), "ContactInfo");
    }

    #[test]
    fn test_build_spam_list_node_basic() {
        let request = SpamReportRequest {
            message_id: "TEST123".to_string(),
            message_timestamp: 1234567890,
            spam_flow: SpamFlow::MessageMenu,
            ..Default::default()
        };

        let node = build_spam_list_node(&request);

        assert_eq!(node.tag, "spam_list");
        assert_eq!(node.attrs().string("spam_flow"), "MessageMenu");

        let message = node
            .get_optional_child_by_tag(&["message"])
            .expect("spam_list node should have message child");
        assert_eq!(message.attrs().string("id"), "TEST123");
        assert_eq!(message.attrs().string("t"), "1234567890");
    }

    #[test]
    fn test_build_spam_list_node_with_raw_message() {
        let request = SpamReportRequest {
            message_id: "TEST456".to_string(),
            message_timestamp: 1234567890,
            from_jid: Some(Jid::pn("5511999887766")),
            spam_flow: SpamFlow::MessageMenu,
            raw_message: Some(vec![0x01, 0x02, 0x03]),
            media_type: Some("image".to_string()),
            ..Default::default()
        };

        let node = build_spam_list_node(&request);
        let message = node
            .get_optional_child_by_tag(&["message"])
            .expect("spam_list node should have message child");
        let raw = message
            .get_optional_child_by_tag(&["raw"])
            .expect("message node should have raw child");

        assert_eq!(raw.attrs().string("v"), "3");
        assert_eq!(raw.attrs().string("mediatype"), "image");
    }

    #[test]
    fn test_build_spam_list_node_group() {
        let request = SpamReportRequest {
            message_id: "TEST789".to_string(),
            message_timestamp: 1234567890,
            group_jid: Some(Jid::group("120363025918861132")),
            group_subject: Some("Test Group".to_string()),
            participant_jid: Some(Jid::pn("5511999887766")),
            spam_flow: SpamFlow::GroupInfoReport,
            ..Default::default()
        };

        let node = build_spam_list_node(&request);

        assert_eq!(node.attrs().string("spam_flow"), "GroupInfoReport");
        assert_eq!(node.attrs().string("jid"), "120363025918861132@g.us");
        assert_eq!(node.attrs().string("subject"), "Test Group");
    }
