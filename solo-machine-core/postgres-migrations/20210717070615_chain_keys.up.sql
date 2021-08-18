CREATE TABLE IF NOT EXISTS chain_keys (
    id BIGSERIAL PRIMARY KEY,
    chain_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(chain_id, public_key)
);

