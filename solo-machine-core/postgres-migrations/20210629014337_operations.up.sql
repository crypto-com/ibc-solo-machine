CREATE TABLE IF NOT EXISTS operations (
    id BIGSERIAL PRIMARY KEY,
    request_id TEXT,
    address TEXT NOT NULL,
    denom TEXT NOT NULL,
    amount BIGINT NOT NULL,
    operation_type JSONB NOT NULL,
    transaction_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
