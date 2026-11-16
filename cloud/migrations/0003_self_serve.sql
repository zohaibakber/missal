ALTER TABLE users DROP COLUMN must_change_password;
ALTER TABLE users ADD COLUMN email_verified_at INTEGER;
ALTER TABLE users ADD COLUMN trial_ends_at INTEGER;
ALTER TABLE users ADD COLUMN paid_until INTEGER;
UPDATE users SET email_verified_at = created_at, trial_ends_at = unixepoch() * 1000 + 604800000;

CREATE TABLE email_codes (
    email TEXT NOT NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('verify', 'reset')),
    code_hash TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    sent_at INTEGER NOT NULL,
    PRIMARY KEY (email, purpose)
);
