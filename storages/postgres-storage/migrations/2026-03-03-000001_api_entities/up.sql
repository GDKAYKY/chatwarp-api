CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE api_sessions (
    session TEXT PRIMARY KEY,
    status TEXT,
    webhook_url TEXT,
    webhook_events JSONB,
    webhook_by_events BOOLEAN DEFAULT false,
    webhook_base64 BOOLEAN DEFAULT false,
    webhook_headers JSONB DEFAULT '{}'::jsonb,
    webhook_enabled BOOLEAN DEFAULT true,
    pair_code TEXT,
    qr_code TEXT,
    phone_number TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE webhook_outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session TEXT REFERENCES api_sessions(session),
    event TEXT,
    payload JSONB,
    status TEXT DEFAULT 'pending',
    attempts INT DEFAULT 0,
    next_attempt_at TIMESTAMPTZ DEFAULT now(),
    last_error TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE api_chats (
    session TEXT REFERENCES api_sessions(session),
    id TEXT,
    title TEXT,
    last_message_at TIMESTAMPTZ,
    unread_count INT DEFAULT 0,
    PRIMARY KEY (session, id)
);

CREATE TABLE api_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session TEXT REFERENCES api_sessions(session),
    chat_id TEXT,
    from_me BOOLEAN DEFAULT false,
    message_type TEXT,
    payload JSONB,
    status TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE api_contacts (
    session TEXT REFERENCES api_sessions(session),
    id TEXT,
    name TEXT,
    exists BOOLEAN DEFAULT false,
    profile_picture_url TEXT,
    updated_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (session, id)
);

CREATE TABLE api_groups (
    session TEXT REFERENCES api_sessions(session),
    id TEXT,
    subject TEXT,
    participants JSONB,
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (session, id)
);

CREATE TABLE api_profiles (
    session TEXT PRIMARY KEY REFERENCES api_sessions(session),
    name TEXT,
    status TEXT,
    picture_url TEXT,
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE api_presence (
    session TEXT REFERENCES api_sessions(session),
    chat_id TEXT,
    presence TEXT,
    updated_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (session, chat_id)
);

CREATE TABLE api_labels (
    session TEXT REFERENCES api_sessions(session),
    id TEXT,
    name TEXT,
    color TEXT,
    PRIMARY KEY (session, id)
);

CREATE TABLE api_label_chats (
    session TEXT REFERENCES api_sessions(session),
    label_id TEXT,
    chat_id TEXT,
    PRIMARY KEY (session, label_id, chat_id)
);

CREATE TABLE api_status_updates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session TEXT REFERENCES api_sessions(session),
    status_type TEXT,
    payload JSONB,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE api_channels (
    session TEXT REFERENCES api_sessions(session),
    id TEXT,
    title TEXT,
    followed BOOLEAN DEFAULT false,
    metadata JSONB,
    PRIMARY KEY (session, id)
);

CREATE TABLE api_apps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT,
    config JSONB,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    label TEXT,
    key_hash TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

CREATE TABLE api_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session TEXT REFERENCES api_sessions(session),
    event TEXT,
    payload JSONB,
    created_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_api_messages_session_chat_id ON api_messages (session, chat_id);
CREATE INDEX idx_webhook_outbox_status ON webhook_outbox (status, next_attempt_at);
