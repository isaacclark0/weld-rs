CREATE TABLE IF NOT EXISTS oauth_states (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    provider_id VARCHAR(64) NOT NULL,
    csrf_state VARCHAR(255) NOT NULL,
    pkce_verifier VARCHAR(255) NOT NULL,
    intent VARCHAR(16) NOT NULL DEFAULT 'login',
    link_user_id BIGINT,
    expires_at DATETIME(6) NOT NULL,
    created_at DATETIME(6) NOT NULL,
    UNIQUE KEY idx_oauth_states_csrf_state (csrf_state),
    KEY idx_oauth_states_expires_at (expires_at),
    KEY idx_oauth_states_link_user_id (link_user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
