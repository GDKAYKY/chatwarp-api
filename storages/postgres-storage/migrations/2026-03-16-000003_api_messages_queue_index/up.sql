CREATE INDEX IF NOT EXISTS idx_api_messages_queued
    ON api_messages (session, created_at)
    WHERE status = 'queued';
