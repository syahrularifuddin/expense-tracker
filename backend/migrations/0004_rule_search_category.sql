-- Frasa subjek untuk query Gmail dan kategori default per aturan parser.
ALTER TABLE parser_rules
    ADD COLUMN search_subject TEXT,
    ADD COLUMN category TEXT;

-- Kata kunci merchant tambahan dari pola transaksi nyata.
INSERT INTO category_rules (keyword, category_id)
SELECT k.keyword, c.id
FROM (VALUES
    ('spbu', 'Transportasi'), ('shell', 'Transportasi'), ('parkir', 'Transportasi'),
    ('tol ', 'Transportasi'), ('kereta', 'Transportasi'), ('krl', 'Transportasi'),
    ('gorengan', 'Makanan & Minuman'), ('cilok', 'Makanan & Minuman'), ('bakso', 'Makanan & Minuman'),
    ('bakery', 'Makanan & Minuman'), ('bakehouse', 'Makanan & Minuman'), ('es te', 'Makanan & Minuman'),
    ('coffee', 'Makanan & Minuman'), ('cafe', 'Makanan & Minuman'), ('mie ', 'Makanan & Minuman'),
    ('nasi', 'Makanan & Minuman'), ('ayam', 'Makanan & Minuman'), ('sate', 'Makanan & Minuman'),
    ('martabak', 'Makanan & Minuman'), ('roti', 'Makanan & Minuman'), ('starbucks', 'Makanan & Minuman'),
    ('superindo', 'Belanja'), ('hypermart', 'Belanja'), ('lotte', 'Belanja'), ('blibli', 'Belanja'),
    ('oracle', 'Tagihan'), ('google', 'Tagihan'), ('apple.com', 'Tagihan'), ('openai', 'Tagihan'),
    ('anthropic', 'Tagihan'), ('github', 'Tagihan'), ('aws', 'Tagihan'), ('biznet', 'Tagihan'),
    ('indihome', 'Tagihan'), ('telkomsel', 'Tagihan'), ('xl axiata', 'Tagihan'),
    ('youtube', 'Hiburan'), ('disney', 'Hiburan'), ('steam', 'Hiburan'),
    ('guardian', 'Kesehatan'), ('watsons', 'Kesehatan'), ('kimia farma', 'Kesehatan')
) AS k(keyword, category)
JOIN categories c ON c.name = k.category AND c.user_id IS NULL;
