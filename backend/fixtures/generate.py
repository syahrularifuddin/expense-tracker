"""Membuat fixtures/emails.json: email notifikasi sintetis + hasil parse yang diharapkan.

Jalankan: python3 fixtures/generate.py   (dari direktori backend/)

Format body meniru email asli (setelah HTML -> teks, satu baris per blok), tetapi nama,
rekening, merchant, dan nominal rekaan. Nilai `expect` dihitung dari data sumber, bukan
dari parser, sehingga test benar-benar menguji regex di config/senders.toml.
"""

import datetime as dt
import json
import pathlib

WIB = dt.timezone(dt.timedelta(hours=7))
EN = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
ID_FULL = ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus",
           "September", "Oktober", "November", "Desember"]
NAME = "BUDI SANTOSO WIBOWO"


def en_amount(minor):  # 631,440.00
    whole, cents = divmod(minor, 100)
    return f"{whole:,}.{cents:02d}"


def id_amount(minor):  # 3.898.000 (Krom tanpa sen)
    return f"{minor // 100:,}".replace(",", ".")


DATE = {
    # nama -> (format, apakah detik tertulis)
    "jenius_card": (lambda t: f"{t.day:02d} {EN[t.month - 1].upper()} {t.year} {t:%H:%M:%S} WIB", True),
    "jenius_qris": (lambda t: t.strftime("%d/%m/%Y %H:%M:%S"), True),
    "krom_transfer": (lambda t: f"{t.day:02d} {ID_FULL[t.month - 1]} {t.year}, {t:%H.%M}", False),
    "krom_qris": (lambda t: t.strftime("%d-%m-%Y %H:%M (WIB)"), False),
    "bca": (lambda t: f"{t.day:02d} {EN[t.month - 1]} {t.year} {t:%H:%M:%S}", True),
}

SHOPS = ["KOPI KENANGAN SUDIRMAN", "SPBU 34-123.45 - JAKARTA SELATAN", "Gorengan Pak Kumis",
         "Tadaima Bakehouse, Kemang", "INDOMARET KEMANG RAYA", "Bakso Solo Samrat", "Superindo Cipete",
         "Es Teh Nusantara, Blok M", "Apotek K-24 Fatmawati", "Sate Khas Senayan", "Roti Bakar 88",
         "Alfamart Jl. Sudirman"]
ONLINE = ["NETFLIX.COM", "GOOGLE *YouTube", "TOKOPEDIA", "SPOTIFY P2A1B2C3", "OPENAI *CHATGPT"]
PEOPLE = ["SITI AMINAH", "AGUS PRASETYO", "DEWI LESTARI", "PT MAJU JAYA ABADI", "CV. SUMBER REJEKI",
          NAME]
AMOUNTS = [1_500_000, 63_144_000, 1_000_000, 7_225_000, 389_800_000, 4_500_000, 1_800_000,
           25_000_000, 12_550_050, 2_800_000, 999_990_000, 6_640_000]

TAIL_JENIUS = ("If you not do this transaction, please immediately call 1500 365.\n"
               "Beware of scams especially the one who impersonate Jenius team.")
TAIL_KROM = ("Segera hubungi Krom Support di nomor 021-5099-6920 atau di email support@krom.id "
             "jika tidak melakukan transaksi ini.\nTerima kasih,\nTim Krom")
TAIL_BCA = ("Please save this email as your transaction reference.\n"
            "If you do not recognize this transaction, immediately contact Halo BCA at 1500888.\n"
            "Best Regards,\nPT Bank Central Asia Tbk")


def kv(*pairs):
    """Gaya BCA: Label / ':' / Nilai, masing-masing satu baris."""
    return "\n".join(f"{k}\n:\n{v}" for k, v in pairs)


def jenius(i, t, amount, merchant):
    qris = i % 3 == 2
    fmt, sec = DATE["jenius_qris" if qris else "jenius_card"]
    currency, shown = ("SGD", f"{amount / 100:.2f}") if i == 4 else ("IDR", en_amount(amount))
    if i == 4:
        merchant = "ORACLE SINGAPORE SINGAPORE SG"
    body = (f"Hi {NAME.title()},\nThank you for using Jenius Credit Card. You've just made a transaction with\n"
            f"Card number: 437896******1234\nMerchant: {merchant}\nTransaction date & time: {fmt(t)}\n"
            f"Total: {currency} {shown}\n{TAIL_JENIUS}")
    subject = "Credit Card QRIS Transaction Notification" if qris else "Credit Card Transaction Notification"
    return "Jenius <jenius_noreply@smbci.com>", subject, body, sec, merchant, currency, None, None


def krom(i, t, amount, merchant):
    if i % 2 == 0:
        fmt, sec = DATE["krom_transfer"]
        to = PEOPLE[i % len(PEOPLE)]
        body = (f"Hai, {NAME},\nKamu telah mengirim dana dengan rincian sebagai berikut:\n"
                f"Tanggal & Waktu: {fmt(t)}\nDari: Tabungan Utama • 770011112222\n"
                f"Ke: {to} • 0480001234\nMetode transfer: BI-Fast\nJumlah: Rp{id_amount(amount)}\n{TAIL_KROM}")
        return "Krom <noreply@krom.id>", "Transfer Berhasil", body, sec, to, "IDR", "Transfer", None
    fmt, sec = DATE["krom_qris"]
    body = (f"Hai, {NAME},\nKamu telah melakukan pembayaran QRIS dengan rincian sebagai berikut:\n"
            f"Tanggal & Waktu: {fmt(t)}\nDari: Tabungan utama • 770011112222\n"
            f"Merchant: {merchant}\nJumlah: Rp{id_amount(amount)}\n{TAIL_KROM}")
    return "Krom <noreply@krom.id>", "Transaksi QRIS Berhasil", body, sec, merchant, "IDR", None, None


ID_ABBR = ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agt", "Sep", "Okt", "Nov", "Des"]


def bca(i, t, amount, merchant):
    """Tujuh jenis jurnal myBCA, bergantian bahasa Inggris dan Indonesia."""
    kind = ["qris_en", "transfer_en", "qris_id", "transfer_bank_id", "va_id", "ewallet_en",
            "qris_transfer_id", "flazz_en", "bill_id", "va_old_id", "transfer_bca_id", "qris_en"][i]
    en = kind.endswith("_en")
    date = f"{t.day:02d} {(EN if en else ID_ABBR)[t.month - 1]} {t.year} {t:%H:%M:%S}"
    head = (f"Hello {NAME},\nYou just made a transaction through myBCA.\nHere are the details of your transaction :"
            if en else f"Hai {NAME},\nAnda baru saja melakukan transaksi dengan menggunakan fasilitas myBCA.\n"
            "Berikut ini adalah detail transaksi Anda :")
    top = [("Status", "Successful"), ("Transaction Date", date)] if en else \
          [("Status", "Berhasil"), ("Tanggal Transaksi", date)]
    amt = f"IDR {en_amount(amount)}"
    fee = 250_000
    category, total = None, amount
    if kind == "qris_en":
        rows = [("Transaction Type", "QRIS Payment"), ("Payment to", merchant), ("Acquirer", "GOPAY"),
                ("Total Payment", amt), ("Reference No.", f"QRS{i}")]
    elif kind == "qris_id":
        rows = [("Jenis Transaksi", "Pembayaran QRIS"), ("Pembayaran Ke", merchant), ("Pengakuisisi", "BCA"),
                ("Total Bayar", amt), ("Nomor Referensi", f"QRS{i}")]
    elif kind == "transfer_en":
        merchant, category = PEOPLE[i % len(PEOPLE)], "Transfer"
        rows = [("Transfer Type", "Transfer to BCA Account"), ("Beneficiary Account", "5320001234"),
                ("Beneficiary Name", merchant), ("Transfer Amount", amt), ("Remarks", "-")]
    elif kind == "transfer_bca_id":
        merchant, category = PEOPLE[i % len(PEOPLE)], "Transfer"
        rows = [("Jenis Transfer", "Transfer ke rekening BCA"), ("Rekening Tujuan", "5320001234"),
                ("Nama Penerima", merchant), ("Nominal Tujuan", amt), ("Berita", "-")]
    elif kind == "transfer_bank_id":  # bank lain: nominal + biaya terpisah, tanpa total
        merchant, category = PEOPLE[i % len(PEOPLE)], "Transfer"
        rows = [("Jenis Transfer", "Transfer ke BRI"), ("Nama Penerima", merchant), ("Bank Tujuan", "BRI"),
                ("Nominal", amt), ("Biaya", f"IDR {en_amount(fee)}"), ("Layanan Transfer", "BI FAST")]
    elif kind == "va_id":
        merchant = "PT TOKOPEDIA / TOKOPEDIA"
        rows = [("Jenis Transaksi", "Transfer ke BCA Virtual Account"), ("Nama", "TTSbyTokopedia"),
                ("Nama Perusahaan/Produk", merchant), ("Total Tagihan", amt), ("Total Bayar", amt)]
    elif kind == "va_old_id":  # format lama tanpa "Total Bayar"
        merchant = "PT ESPAY DEBIT INDONESIA KOE / DANA"
        rows = [("Jenis Transaksi", "Transfer ke BCA Virtual Account"), ("Nama", "DANATOPUP BUDI"),
                ("Nama Perusahaan/Produk", merchant), ("Nominal Bayar", amt)]
    elif kind == "ewallet_en":
        merchant, category, total = "e-Wallet - GO-PAY TOPUP", "Top Up", amount + fee
        rows = [("Transaction Type", merchant), ("Amount", amt), ("Admin Fee", f"IDR {en_amount(fee)}"),
                ("Total Payment", f"IDR {en_amount(total)}")]
    elif kind == "qris_transfer_id":
        merchant, total = "budi.shop88", amount + fee
        rows = [("Jenis Transaksi", "Transfer QRIS"), ("Nama Penerima", merchant), ("Pengakuisisi", "SHOPEEPAY"),
                ("Nominal", amt), ("Biaya", f"IDR {en_amount(fee)}"), ("Total Transfer", f"IDR {en_amount(total)}")]
    elif kind == "flazz_en":
        merchant, category = "Flazz Top Up", "Top Up"
        rows = [("Transaction Type", merchant), ("Flazz Card Number", "0145000011112222"), ("Top Up Amount", amt)]
    else:  # bill_id
        merchant, category = "Kartu Kredit & Paylater - BCA", "Tagihan"
        rows = [("Jenis Transaksi", merchant), ("Jumlah Tagihan", f"IDR {en_amount(amount * 2)}"),
                ("Total Bayar", amt), ("Sisa Tagihan", "IDR 0.00")]
    body = f"{head}\n{kv(*top, *rows)}\n{TAIL_BCA}"
    return "BCA <bca@bca.co.id>", "Internet Transaction Journal", body, True, merchant, "IDR", category, total


PROVIDERS = {"jenius": jenius, "krom": krom, "bca": bca}

# Dilewati (pengirim dikenal, subjek bukan transaksi) atau gagal parse (subjek cocok,
# isi tidak). Keduanya harus menghasilkan parse == None.
NOT_PARSED = [
    ("BCA <bca@bca.co.id>", "Promo Spesial Akhir Tahun", "Nikmati cashback hingga Rp50.000 dengan BCA."),
    ("Jenius <jenius_noreply@smbci.com>", "Your Credit Card e-Statement", "Your e-Statement is ready."),
    ("Krom <noreply@krom.id>", "Kode OTP Krom", "Kode OTP kamu 123456. Jangan berikan kepada siapa pun."),
    ("BCA <bca@bca.co.id>", "Internet Transaction Journal",
     "Hello BUDI SANTOSO WIBOWO,\nYou just made a transaction through myBCA.\n"
     + kv(("Status", "Failed"), ("Transaction Date", "03 Nov 2026 10:00:00"), ("Transaction Type", "QRIS Payment"),
          ("Payment to", "TOKO GAGAL"), ("Total Payment", "IDR 50,000.00"))),
    ("BCA <bca@bca.co.id>", "Internet Transaction Journal",
     "Hai BUDI SANTOSO WIBOWO,\nAnda baru saja melakukan pendaftaran rekening tujuan transfer dengan "
     "menggunakan fasilitas myBCA.\n"
     + kv(("Tanggal Transaksi", "04 Nov 2026 09:00:00"), ("Jenis Pendaftaran", "DAFTAR REKENING TUJUAN BCA"),
          ("Rekening Tujuan", "1234xxxx56"), ("Nama Pemilik Rekening", "CONTOH PT"))),
]


def iso(t):
    return t.astimezone(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def main():
    out = []
    start = dt.datetime(2026, 10, 1, 7, 5, 9, tzinfo=WIB)
    step = dt.timedelta(days=4, hours=3, minutes=17, seconds=11)

    for p, (provider, make) in enumerate(PROVIDERS.items()):
        for i in range(12):
            t = start + step * i + dt.timedelta(hours=p)
            amount = AMOUNTS[(i + p) % len(AMOUNTS)]
            if provider == "krom":  # Krom menulis nominal tanpa sen
                amount -= amount % 100
            pool = ONLINE if provider == "jenius" and i % 2 else SHOPS
            sender, subject, body, has_seconds, merchant, currency, category, total = make(
                i, t, amount, pool[(i + p) % len(pool)])
            out.append({
                "id": f"{provider}-{i:02d}",
                "from": sender,
                "subject": subject,
                "body": body,
                "received_at": iso(t + dt.timedelta(minutes=1)),
                "expect": {
                    "provider": provider,
                    "amount_minor": total or amount,
                    "currency": currency,
                    "type": "debit",
                    "merchant": merchant,
                    "category": category,
                    "occurred_at": iso(t if has_seconds else t.replace(second=0)),
                },
            })

    # Refund: transaksi Jenius dibatalkan -> credit.
    for i in (1, 6):
        t = start + step * i + dt.timedelta(hours=5)
        # Email asli menulis "Merchant: Refund" dengan tanggal + nominal transaksi semula.
        merchant, amount = "Refund", AMOUNTS[i]
        out.append({
            "id": f"jenius-cancel-{i:02d}",
            "from": "Jenius <jenius_noreply@smbci.com>",
            "subject": "Your Jenius 437896******1234 transaction has been cancelled",
            "body": (f"Hi {NAME.title()},\nYour Jenius Credit Card transaction has been cancelled.\n"
                     f"Card number: 437896******1234\nMerchant: {merchant}\n"
                     f"Transaction date & time: {DATE['jenius_card'][0](t)}\nTotal: IDR {en_amount(amount)}\n"
                     f"{TAIL_JENIUS}"),
            "received_at": iso(t + dt.timedelta(minutes=1)),
            "expect": {"provider": "jenius", "amount_minor": amount, "currency": "IDR", "type": "credit",
                       "merchant": merchant, "category": None, "occurred_at": iso(t)},
        })

    for i, (sender, subject, body) in enumerate(NOT_PARSED):
        out.append({
            "id": f"not-parsed-{i:02d}", "from": sender, "subject": subject, "body": body,
            "received_at": iso(start + step * i + dt.timedelta(hours=18)), "expect": None,
        })

    path = pathlib.Path(__file__).with_name("emails.json")
    path.write_text(json.dumps(out, ensure_ascii=False, indent=1) + "\n")
    print(f"{len(out)} email -> {path}")


if __name__ == "__main__":
    main()
