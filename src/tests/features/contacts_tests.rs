    use super::*;

    #[test]
    fn test_contact_info_struct() {
        let jid: Jid = "1234567890@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let lid: Jid = "12345678@lid".parse().expect("test JID should be valid");

        let info = ContactInfo {
            jid: jid.clone(),
            lid: Some(lid.clone()),
            is_registered: true,
            is_business: false,
            status: Some("Hey there!".to_string()),
            picture_id: Some(123456789),
        };

        assert!(info.is_registered);
        assert!(!info.is_business);
        assert_eq!(info.status, Some("Hey there!".to_string()));
        assert_eq!(info.picture_id, Some(123456789));
        assert!(info.lid.is_some());
    }

    #[test]
    fn test_profile_picture_struct() {
        let pic = ProfilePicture {
            id: "123456789".to_string(),
            url: "https://example.com/pic.jpg".to_string(),
            direct_path: Some("/v/pic.jpg".to_string()),
        };

        assert_eq!(pic.id, "123456789");
        assert_eq!(pic.url, "https://example.com/pic.jpg");
        assert!(pic.direct_path.is_some());
    }

    #[test]
    fn test_is_on_whatsapp_result_struct() {
        let jid: Jid = "1234567890@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let result = IsOnWhatsAppResult {
            jid,
            is_registered: true,
        };

        assert!(result.is_registered);
    }
