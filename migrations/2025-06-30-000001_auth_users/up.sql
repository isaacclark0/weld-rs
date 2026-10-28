CREATE TABLE IF NOT EXISTS users (
    id BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY,
    -- Unique case-insensitively through the _ci collation
    email VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    password_hash VARCHAR(255),
    email_verified_at DATETIME(6),
    image TEXT,
    created_at DATETIME(6) NOT NULL,
    updated_at DATETIME(6) NOT NULL,
    UNIQUE KEY idx_users_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
