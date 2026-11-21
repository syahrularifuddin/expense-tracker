-- Sender global yang dimatikan oleh user tertentu.
CREATE TABLE disabled_senders (
    user_id UUID NOT NULL REFERENCES users ON DELETE CASCADE,
    sender  TEXT NOT NULL,
    PRIMARY KEY (user_id, sender)
);
