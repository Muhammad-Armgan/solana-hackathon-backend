-- 002_indexes.sql
-- Query-performance indexes for histories, lookups, and transfer tracking.

CREATE INDEX IF NOT EXISTS idx_users_wallet_address ON users(wallet_address);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

CREATE INDEX IF NOT EXISTS idx_quizzes_user_created ON quizzes(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_quizzes_subject ON quizzes(subject);

CREATE INDEX IF NOT EXISTS idx_papers_user_created ON papers(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_papers_subject ON papers(subject);

CREATE INDEX IF NOT EXISTS idx_uploads_user_created ON uploads(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_uploads_status ON uploads(status);

CREATE INDEX IF NOT EXISTS idx_transactions_from_user_created ON transactions(from_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_to_user_created ON transactions(to_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_type_created ON transactions(tx_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_reference_id ON transactions(reference_id);

CREATE INDEX IF NOT EXISTS idx_token_transfers_sender_created ON token_transfers(sender_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_token_transfers_recipient_created ON token_transfers(recipient_user_id, created_at DESC);
