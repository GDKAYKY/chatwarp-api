DROP INDEX IF EXISTS idx_webhook_outbox_status;
DROP INDEX IF EXISTS idx_api_messages_session_chat_id;

DROP TABLE IF EXISTS api_events;
DROP TABLE IF EXISTS api_keys;
DROP TABLE IF EXISTS api_apps;
DROP TABLE IF EXISTS api_channels;
DROP TABLE IF EXISTS api_status_updates;
DROP TABLE IF EXISTS api_label_chats;
DROP TABLE IF EXISTS api_labels;
DROP TABLE IF EXISTS api_presence;
DROP TABLE IF EXISTS api_profiles;
DROP TABLE IF EXISTS api_groups;
DROP TABLE IF EXISTS api_contacts;
DROP TABLE IF EXISTS api_messages;
DROP TABLE IF EXISTS api_chats;
DROP TABLE IF EXISTS webhook_outbox;
DROP TABLE IF EXISTS api_sessions;
