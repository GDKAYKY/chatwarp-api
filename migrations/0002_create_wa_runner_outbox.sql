CREATE TABLE IF NOT EXISTS wa_runner_outbox (
    id BIGSERIAL PRIMARY KEY,
    instance_name TEXT NOT NULL,
    message_id TEXT NOT NULL,
    payload BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS wa_runner_outbox_instance_created_idx
    ON wa_runner_outbox (instance_name, created_at);

CREATE UNIQUE INDEX IF NOT EXISTS wa_runner_outbox_message_id_idx
    ON wa_runner_outbox (instance_name, message_id);
