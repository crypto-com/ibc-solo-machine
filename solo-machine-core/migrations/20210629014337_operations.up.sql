CREATE TABLE IF NOT EXISTS operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT,
    address TEXT NOT NULL,
    denom TEXT NOT NULL,
    amount INTEGER NOT NULL,
    operation_type TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
