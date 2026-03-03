// @generated automatically by Diesel CLI.

diesel::table! {
    app_state_keys (key_id, device_id) {
        key_id -> Binary,
        key_data -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    app_state_mutation_macs (name, index_mac, device_id) {
        name -> Text,
        version -> BigInt,
        index_mac -> Binary,
        value_mac -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    app_state_versions (name, device_id) {
        name -> Text,
        state_data -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    base_keys (address, message_id, device_id) {
        address -> Text,
        message_id -> Text,
        base_key -> Binary,
        device_id -> Integer,
        created_at -> Integer,
    }
}

diesel::table! {
    device_registry (user_id, device_id) {
        user_id -> Text,
        devices_json -> Text,
        timestamp -> Integer,
        phash -> Nullable<Text>,
        device_id -> Integer,
        updated_at -> Integer,
    }
}

diesel::table! {
    device (id) {
        id -> Integer,
        lid -> Text,
        pn -> Text,
        registration_id -> Integer,
        noise_key -> Binary,
        identity_key -> Binary,
        signed_pre_key -> Binary,
        signed_pre_key_id -> Integer,
        signed_pre_key_signature -> Binary,
        adv_secret_key -> Binary,
        account -> Nullable<Binary>,
        push_name -> Text,
        app_version_primary -> Integer,
        app_version_secondary -> Integer,
        app_version_tertiary -> BigInt,
        app_version_last_fetched_ms -> BigInt,
        edge_routing_info -> Nullable<Binary>,
    }
}

diesel::table! {
    identities (address, device_id) {
        address -> Text,
        key -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    lid_pn_mapping (lid, device_id) {
        lid -> Text,
        phone_number -> Text,
        created_at -> BigInt,
        learning_source -> Text,
        updated_at -> BigInt,
        device_id -> Integer,
    }
}

diesel::table! {
    prekeys (id, device_id) {
        id -> Integer,
        key -> Binary,
        uploaded -> Bool,
        device_id -> Integer,
    }
}

diesel::table! {
    sender_key_status (group_jid, participant, device_id) {
        group_jid -> Text,
        participant -> Text,
        device_id -> Integer,
        marked_at -> Integer,
    }
}

diesel::table! {
    sender_keys (address, device_id) {
        address -> Text,
        record -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    sessions (address, device_id) {
        address -> Text,
        record -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    signed_prekeys (id, device_id) {
        id -> Integer,
        record -> Binary,
        device_id -> Integer,
    }
}

diesel::table! {
    skdm_recipients (group_jid, device_jid, device_id) {
        group_jid -> Text,
        device_jid -> Text,
        device_id -> Integer,
        created_at -> Integer,
    }
}

diesel::table! {
    api_sessions (session) {
        session -> Text,
        status -> Nullable<Text>,
        webhook_url -> Nullable<Text>,
        webhook_events -> Nullable<Jsonb>,
        webhook_by_events -> Bool,
        webhook_base64 -> Bool,
        webhook_headers -> Jsonb,
        webhook_enabled -> Bool,
        pair_code -> Nullable<Text>,
        qr_code -> Nullable<Text>,
        phone_number -> Nullable<Text>,
        last_error -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    webhook_outbox (id) {
        id -> Uuid,
        session -> Nullable<Text>,
        event -> Nullable<Text>,
        payload -> Nullable<Jsonb>,
        status -> Text,
        attempts -> Int4,
        next_attempt_at -> Timestamptz,
        last_error -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    api_chats (session, id) {
        session -> Text,
        id -> Text,
        title -> Nullable<Text>,
        last_message_at -> Nullable<Timestamptz>,
        unread_count -> Int4,
    }
}

diesel::table! {
    api_messages (id) {
        id -> Uuid,
        session -> Nullable<Text>,
        chat_id -> Nullable<Text>,
        from_me -> Bool,
        message_type -> Nullable<Text>,
        payload -> Nullable<Jsonb>,
        status -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    api_contacts (session, id) {
        session -> Text,
        id -> Text,
        name -> Nullable<Text>,
        exists -> Bool,
        profile_picture_url -> Nullable<Text>,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    api_groups (session, id) {
        session -> Text,
        id -> Text,
        subject -> Nullable<Text>,
        participants -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    api_profiles (session) {
        session -> Text,
        name -> Nullable<Text>,
        status -> Nullable<Text>,
        picture_url -> Nullable<Text>,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    api_presence (session, chat_id) {
        session -> Text,
        chat_id -> Text,
        presence -> Nullable<Text>,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    api_labels (session, id) {
        session -> Text,
        id -> Text,
        name -> Nullable<Text>,
        color -> Nullable<Text>,
    }
}

diesel::table! {
    api_label_chats (session, label_id, chat_id) {
        session -> Text,
        label_id -> Text,
        chat_id -> Text,
    }
}

diesel::table! {
    api_status_updates (id) {
        id -> Uuid,
        session -> Nullable<Text>,
        status_type -> Nullable<Text>,
        payload -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    api_channels (session, id) {
        session -> Text,
        id -> Text,
        title -> Nullable<Text>,
        followed -> Bool,
        metadata -> Nullable<Jsonb>,
    }
}

diesel::table! {
    api_apps (id) {
        id -> Uuid,
        name -> Nullable<Text>,
        config -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    api_keys (id) {
        id -> Uuid,
        label -> Nullable<Text>,
        key_hash -> Nullable<Text>,
        created_at -> Timestamptz,
        revoked_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    api_events (id) {
        id -> Uuid,
        session -> Nullable<Text>,
        event -> Nullable<Text>,
        payload -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    app_state_keys,
    app_state_mutation_macs,
    app_state_versions,
    api_apps,
    api_chats,
    api_channels,
    api_contacts,
    api_events,
    api_groups,
    api_keys,
    api_label_chats,
    api_labels,
    api_messages,
    api_presence,
    api_profiles,
    api_sessions,
    api_status_updates,
    base_keys,
    device,
    device_registry,
    identities,
    lid_pn_mapping,
    prekeys,
    sender_key_status,
    sender_keys,
    sessions,
    signed_prekeys,
    skdm_recipients,
    webhook_outbox,
);
