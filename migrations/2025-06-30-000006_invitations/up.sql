CREATE TABLE IF NOT EXISTS invitations (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    email VARCHAR(255) NOT NULL,
    token_hash CHAR(64) NOT NULL,
    invited_by BIGINT NOT NULL,
    expires_at DATETIME(6) NOT NULL,
    accepted_at DATETIME(6),
    created_at DATETIME(6) NOT NULL,
    UNIQUE KEY idx_invitations_token_hash (token_hash),
    KEY idx_invitations_email (email),
    KEY idx_invitations_expires_at (expires_at),
    CONSTRAINT fk_invitations_invited_by FOREIGN KEY (invited_by) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
