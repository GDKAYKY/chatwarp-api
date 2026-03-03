CREATE TABLE device (
    id SERIAL PRIMARY KEY,
    lid TEXT NOT NULL DEFAULT '',
    pn TEXT NOT NULL DEFAULT '',
    registration_id INTEGER NOT NULL,
    noise_key BYTEA NOT NULL,
    identity_key BYTEA NOT NULL,
    signed_pre_key BYTEA NOT NULL,
    signed_pre_key_id INTEGER NOT NULL,
    signed_pre_key_signature BYTEA NOT NULL,
    adv_secret_key BYTEA NOT NULL,
    account BYTEA,
    push_name TEXT NOT NULL DEFAULT '',
    app_version_primary INTEGER NOT NULL DEFAULT 0,
    app_version_secondary INTEGER NOT NULL DEFAULT 0,
    app_version_tertiary BIGINT NOT NULL DEFAULT 0,
    app_version_last_fetched_ms BIGINT NOT NULL DEFAULT 0,
    edge_routing_info BYTEA
);

CREATE TABLE identities (
    address TEXT NOT NULL,
    key BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (address, device_id)
);

CREATE TABLE sessions (
    address TEXT NOT NULL,
    record BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (address, device_id)
);

CREATE TABLE prekeys (
    id INTEGER NOT NULL,
    key BYTEA NOT NULL,
    uploaded BOOLEAN NOT NULL DEFAULT FALSE,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (id, device_id)
);

CREATE TABLE sender_keys (
    address TEXT NOT NULL,
    record BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (address, device_id)
);

CREATE TABLE app_state_keys (
    key_id BYTEA NOT NULL,
    key_data BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (key_id, device_id)
);

CREATE TABLE app_state_versions (
    name TEXT NOT NULL,
    state_data BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (name, device_id)
);

CREATE TABLE app_state_mutation_macs (
    name TEXT NOT NULL,
    version BIGINT NOT NULL,
    index_mac BYTEA NOT NULL,
    value_mac BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (name, index_mac, device_id)
);

CREATE TABLE signed_prekeys (
    id INTEGER NOT NULL,
    record BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (id, device_id)
);

CREATE TABLE base_keys (
    address TEXT NOT NULL,
    message_id TEXT NOT NULL,
    base_key BYTEA NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (address, message_id, device_id)
);

CREATE TABLE lid_pn_mapping (
    lid TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    learning_source TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    PRIMARY KEY (lid, device_id)
);

CREATE TABLE skdm_recipients (
    group_jid TEXT NOT NULL,
    device_jid TEXT NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (group_jid, device_jid, device_id)
);

CREATE TABLE device_registry (
    user_id TEXT NOT NULL,
    devices_json TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    phash TEXT,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, device_id)
);

CREATE TABLE sender_key_status (
    group_jid TEXT NOT NULL,
    participant TEXT NOT NULL,
    device_id INTEGER NOT NULL REFERENCES device(id) ON DELETE CASCADE,
    marked_at INTEGER NOT NULL,
    PRIMARY KEY (group_jid, participant, device_id)
);

CREATE INDEX idx_identities_device_id ON identities (device_id);
CREATE INDEX idx_sessions_device_id ON sessions (device_id);
CREATE INDEX idx_prekeys_device_id ON prekeys (device_id);
CREATE INDEX idx_sender_keys_device_id ON sender_keys (device_id);
CREATE INDEX idx_signed_prekeys_device_id ON signed_prekeys (device_id);
CREATE INDEX idx_app_state_keys_device_id ON app_state_keys (device_id);
CREATE INDEX idx_app_state_versions_device_id ON app_state_versions (device_id);
CREATE INDEX idx_base_keys_device_id ON base_keys (device_id);
CREATE INDEX idx_lid_pn_mapping_device_id ON lid_pn_mapping (device_id);
CREATE INDEX idx_skdm_recipients_device_id ON skdm_recipients (device_id);
CREATE INDEX idx_device_registry_device_id ON device_registry (device_id);
CREATE INDEX idx_sender_key_status_device_id ON sender_key_status (device_id);
