CREATE TABLE posts
(
    id INTEGER AUTO_INCREMENT PRIMARY KEY,
    author_id BIGINT,
    created_at DATETIME NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    published BOOLEAN NOT NULL,
    FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE SET NULL
);
