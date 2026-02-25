CREATE TABLE IF NOT EXISTS auth_states (
  instance_name TEXT PRIMARY KEY,
  state_json TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
