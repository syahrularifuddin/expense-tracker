-- Metode pembayaran per transaksi: provider aturan parser (bca, jenius, krom, custom) atau 'manual'.
ALTER TABLE transactions ADD COLUMN provider TEXT NOT NULL DEFAULT 'custom';

-- Baris lama tidak menyimpan pengirim; provider diturunkan dari subjek email bawaan.
UPDATE transactions SET provider = CASE
    WHEN source = 'manual' THEN 'manual'
    WHEN raw_subject ILIKE 'Internet Transaction Journal' THEN 'bca'
    WHEN raw_subject ILIKE 'Credit Card%Transaction Notification' THEN 'jenius'
    WHEN raw_subject ILIKE 'Transfer Berhasil' OR raw_subject ILIKE 'Transaksi QRIS Berhasil' THEN 'krom'
    ELSE 'custom'
END;
