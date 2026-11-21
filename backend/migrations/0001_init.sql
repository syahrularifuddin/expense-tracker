CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    google_sub  TEXT NOT NULL UNIQUE,
    email       TEXT NOT NULL,
    name        TEXT,
    avatar_url  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE google_tokens (
    user_id                  UUID PRIMARY KEY REFERENCES users ON DELETE CASCADE,
    refresh_token_ciphertext BYTEA NOT NULL,
    nonce                    BYTEA NOT NULL,
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id            UUID NOT NULL REFERENCES users ON DELETE CASCADE,
    refresh_token_hash BYTEA NOT NULL UNIQUE,
    expires_at         TIMESTAMPTZ NOT NULL,
    revoked_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- user_id NULL = kategori default (global)
CREATE TABLE categories (
    id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users ON DELETE CASCADE,
    name    TEXT NOT NULL,
    color   TEXT
);
CREATE UNIQUE INDEX categories_owner_name
    ON categories (COALESCE(user_id, '00000000-0000-0000-0000-000000000000'), lower(name));

CREATE TABLE transactions (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID NOT NULL REFERENCES users ON DELETE CASCADE,
    occurred_at      TIMESTAMPTZ NOT NULL,
    amount_minor     BIGINT NOT NULL CHECK (amount_minor > 0), -- sen
    currency         TEXT NOT NULL DEFAULT 'IDR',
    type             TEXT NOT NULL CHECK (type IN ('debit', 'credit')),
    merchant         TEXT,
    description      TEXT,
    category_id      UUID REFERENCES categories ON DELETE SET NULL,
    category_locked  BOOLEAN NOT NULL DEFAULT false,
    source           TEXT NOT NULL CHECK (source IN ('gmail', 'manual')),
    gmail_message_id TEXT,
    raw_subject      TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, gmail_message_id)
);
CREATE INDEX transactions_user_occurred ON transactions (user_id, occurred_at DESC);

-- user_id NULL = aturan global, di-seed dari config/senders.toml
CREATE TABLE parser_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID REFERENCES users ON DELETE CASCADE,
    provider        TEXT NOT NULL,
    sender          TEXT NOT NULL DEFAULT '',
    subject_pattern TEXT,
    body_regex      TEXT NOT NULL,
    default_type    TEXT CHECK (default_type IN ('debit', 'credit')),
    enabled         BOOLEAN NOT NULL DEFAULT true,
    priority        INT NOT NULL DEFAULT 0
);

CREATE TABLE category_rules (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users ON DELETE CASCADE,
    keyword     TEXT NOT NULL,
    category_id UUID NOT NULL REFERENCES categories ON DELETE CASCADE
);

CREATE TABLE unparsed_emails (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          UUID NOT NULL REFERENCES users ON DELETE CASCADE,
    gmail_message_id TEXT NOT NULL,
    sender           TEXT NOT NULL,
    subject          TEXT NOT NULL,
    snippet          TEXT NOT NULL,
    received_at      TIMESTAMPTZ NOT NULL,
    ignored          BOOLEAN NOT NULL DEFAULT false,
    UNIQUE (user_id, gmail_message_id)
);

CREATE TABLE sync_state (
    user_id        UUID PRIMARY KEY REFERENCES users ON DELETE CASCADE,
    history_id     TEXT,
    last_synced_at TIMESTAMPTZ,
    status         TEXT NOT NULL DEFAULT 'idle',
    last_error     TEXT
);

INSERT INTO categories (name, color) VALUES
    ('Makanan & Minuman', '#E56910'),
    ('Transportasi',      '#0C66E4'),
    ('Belanja',           '#6E5DC6'),
    ('Tagihan',           '#1D7F8C'),
    ('Hiburan',           '#AE4787'),
    ('Kesehatan',         '#5B7F24'),
    ('Transfer',          '#626F86'),
    ('Top Up',            '#626F86'),
    ('Pemasukan',         '#22A06B'),
    ('Lainnya',           '#626F86');

INSERT INTO category_rules (keyword, category_id)
SELECT k.keyword, c.id
FROM (VALUES
    ('kopi', 'Makanan & Minuman'), ('warung', 'Makanan & Minuman'), ('resto', 'Makanan & Minuman'),
    ('mcd', 'Makanan & Minuman'), ('kfc', 'Makanan & Minuman'), ('gofood', 'Makanan & Minuman'),
    ('grab', 'Transportasi'), ('gojek', 'Transportasi'), ('pertamina', 'Transportasi'),
    ('mrt', 'Transportasi'), ('transjakarta', 'Transportasi'),
    ('tokopedia', 'Belanja'), ('shopee', 'Belanja'), ('indomaret', 'Belanja'),
    ('alfamart', 'Belanja'), ('lazada', 'Belanja'),
    ('pln', 'Tagihan'), ('telkom', 'Tagihan'), ('pdam', 'Tagihan'), ('bpjs', 'Tagihan'),
    ('netflix', 'Hiburan'), ('spotify', 'Hiburan'), ('cgv', 'Hiburan'), ('xxi', 'Hiburan'),
    ('apotek', 'Kesehatan'), ('klinik', 'Kesehatan'), ('halodoc', 'Kesehatan')
) AS k(keyword, category)
JOIN categories c ON c.name = k.category AND c.user_id IS NULL;
