-- Palet kategori bawaan disesuaikan dengan design system (oranye + netral lembut).
UPDATE categories SET color = v.color
FROM (VALUES
    ('Makanan & Minuman', '#E8833A'),
    ('Transportasi',      '#5F7A99'),
    ('Belanja',           '#C9A26B'),
    ('Tagihan',           '#4E9A9A'),
    ('Hiburan',           '#9A6B8F'),
    ('Kesehatan',         '#2F8F5B'),
    ('Transfer',          '#7C8597'),
    ('Top Up',            '#A0A4AD'),
    ('Pemasukan',         '#2F8F5B'),
    ('Lainnya',           '#A0A4AD')
) AS v(name, color)
WHERE categories.user_id IS NULL AND categories.name = v.name;
