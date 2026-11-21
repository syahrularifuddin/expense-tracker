"""Membuat fixtures/emails.json: email notifikasi sintetis + hasil parse yang diharapkan.

Jalankan: python3 fixtures/generate.py   (dari direktori backend/)

Nilai `expect` dihitung dari data sumber, bukan dari parser, sehingga test
benar-benar menguji regex di config/senders.toml.
"""

import datetime as dt
import json
import pathlib

WIB = dt.timezone(dt.timedelta(hours=7))
ABBR = ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"]
FULL = ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus",
        "September", "Oktober", "November", "Desember"]


def rp(minor, space=True, dec=False):
    whole, cents = divmod(minor, 100)
    s = f"{whole:,}".replace(",", ".")
    if dec or cents:
        s += f",{cents:02d}"
    return ("Rp " if space else "Rp") + s


# nama format -> (fungsi format, apakah detik ikut tertulis)
DATE = {
    "slash_s": (lambda t: t.strftime("%d/%m/%Y %H:%M:%S"), True),
    "abbr_s": (lambda t: f"{t.day} {ABBR[t.month - 1]} {t.year} {t:%H:%M:%S}", True),
    "mandiri": (lambda t: f"{t.day} {ABBR[t.month - 1]} {t.year}\nJam\n{t:%H:%M:%S} WIB", True),
    "full_s": (lambda t: f"{t.day} {FULL[t.month - 1]} {t.year}, {t:%H:%M:%S} WIB", True),
    "pukul": (lambda t: f"{t.day} {ABBR[t.month - 1]} {t.year} pukul {t:%H:%M} WIB", False),
    "abbr_m": (lambda t: f"{t.day} {ABBR[t.month - 1]} {t.year}, {t:%H:%M}", False),
    "slash_m": (lambda t: t.strftime("%d/%m/%Y %H:%M"), False),
    "dash_m": (lambda t: t.strftime("%d-%m-%Y %H:%M"), False),
}

SHOPS = ["KOPI KENANGAN SUDIRMAN", "Tokopedia", "Grab", "Indomaret Kemang", "McDonald's Sarinah",
         "PT. PLN (Persero)", "Alfamart Jl. Sudirman", "Netflix", "Apotek K-24", "Pertamina 34.123.05",
         "Warung Padang Sederhana", "CGV Grand Indonesia"]
PEOPLE = ["BUDI SANTOSO", "SITI AMINAH", "PT MAJU JAYA ABADI", "DEWI LESTARI", "AGUS PRASETYO",
          "CV. SUMBER REJEKI"]
BANKS = ["BCA", "Bank Mandiri", "BRI", "BCA OneKlik", "Kartu Debit BNI", "Transfer Bank Permata"]
AMOUNTS = [2_800_000, 150_000_000, 1_250_050, 4_550_000, 25_000_000, 1_500_000, 750_000_000,
           3_200_000, 9_999_900, 12_500_000, 500_000, 67_850_075]

# provider -> (From, [(subject, body, type, format tanggal, pool merchant, opsi)])
# Placeholder body: {m} merchant, {a} nominal, {d} tanggal, {fee}, {total}.
T = {
    "bca": ("BCA <bca@bca.co.id>", [
        ("Notifikasi Transaksi Debit BCA",
         "Yth. Bapak/Ibu,\n\nBerikut informasi transaksi kartu debit BCA Anda:\n"
         "Tanggal Transaksi : {d}\nMerchant : {m}\nJumlah : {a}\nStatus : Berhasil\n\n"
         "Terima kasih telah menggunakan layanan BCA.",
         "debit", "slash_s", SHOPS, {"dec": True}),
        ("Transfer Dana Berhasil",
         "Yth. Bapak/Ibu,\n\nTransfer dana dari rekening Anda berhasil.\n"
         "Tanggal : {d}\nPenerima : {m}\nNominal : {a}\nBerita : -\n\nSalam,\nBCA",
         "debit", "abbr_s", PEOPLE, {"dec": True}),
        ("Dana Masuk ke Rekening BCA",
         "Yth. Bapak/Ibu,\n\nRekening Anda telah menerima dana.\n"
         "Tanggal : {d}\nPengirim : {m}\nNominal : {a}\n\nSalam,\nBCA",
         "credit", "abbr_s", PEOPLE, {"dec": True}),
    ]),
    "mandiri": ("Livin' by Mandiri <noreply.livin@bankmandiri.co.id>", [
        ("Pembayaran Berhasil",
         "Hai RINA,\nPembayaran kamu berhasil!\n\nPenerima\n{m}\nTanggal\n{d}\n"
         "Nominal Transaksi\n{a}\nNo. Referensi\n702611210000123\n\n"
         "Terima kasih telah menggunakan Livin' by Mandiri.",
         "debit", "mandiri", SHOPS, {"dec": True}),
        ("Transfer Berhasil",
         "Hai RINA,\nTransfer kamu berhasil!\n\nPenerima\n{m}\nBank BCA - ****1234\nTanggal\n{d}\n"
         "Nominal Transaksi\n{a}\nBiaya Transaksi\nRp 0,00\n",
         "debit", "mandiri", PEOPLE, {"dec": True}),
        ("Dana Masuk",
         "Hai RINA,\nAda dana masuk ke rekening kamu.\n\nPengirim\n{m}\nTanggal\n{d}\n"
         "Nominal Transaksi\n{a}\n",
         "credit", "mandiri", PEOPLE, {"dec": True}),
    ]),
    "bri": ("BRImo <noreply@bri.co.id>", [
        ("Transaksi BRImo Berhasil",
         "Transaksi BRImo Berhasil\n\nTanggal: {d}\nJenis Transaksi: Pembayaran QRIS\n"
         "Tujuan: {m}\nNominal: {a}\nBiaya Admin: Rp0\nTotal: {total}\n",
         "debit", "full_s", SHOPS, {"space": False}),
        ("Transaksi BRImo Berhasil",
         "Transaksi BRImo Berhasil\n\nTanggal: {d}\nJenis Transaksi: Transfer Antar Bank\n"
         "Tujuan: {m}\nNominal: {a}\nBiaya Admin: {fee}\nTotal: {total}\n",
         "debit", "full_s", PEOPLE, {"space": False, "fee": 250_000}),
        ("Dana Masuk ke Rekening BRI",
         "Dana Masuk\n\nTanggal: {d}\nSumber: {m}\nNominal: {a}\nKeterangan: Transfer masuk\n",
         "credit", "full_s", PEOPLE, {"space": False}),
    ]),
    "jenius": ("Jenius <no-reply@jenius.com>", [
        ("Transaksi Kartu Debit Jenius",
         "Hai Rina,\n\nKamu telah melakukan transaksi menggunakan Kartu Debit Jenius di {m} "
         "sebesar {a} pada {d}.\n\nJika bukan kamu, segera hubungi Jenius Help.",
         "debit", "pukul", SHOPS, {}),
        ("Kirim Uang Berhasil",
         "Hai Rina,\n\nKamu telah mengirim uang sebesar {a} ke {m} pada {d}.\n\nSalam,\nJenius",
         "debit", "pukul", PEOPLE, {}),
        ("Kamu Menerima Uang",
         "Hai Rina,\n\nKamu menerima uang sebesar {a} dari {m} pada {d}.\n\nSalam,\nJenius",
         "credit", "pukul", PEOPLE, {}),
    ]),
    "gopay": ("GoPay <no-reply@gopay.co.id>", [
        ("Pembayaran berhasil",
         "Kamu membayar {a} ke {m} pakai GoPay.\n\nWaktu: {d}\nID transaksi: 0120261121ABCDEF\n",
         "debit", "abbr_m", SHOPS, {"space": False}),
        ("Kirim saldo berhasil",
         "Kamu mengirim {a} ke {m} pakai GoPay.\n\nWaktu: {d}\nID transaksi: 0120261121FEDCBA\n",
         "debit", "abbr_m", PEOPLE, {"space": False}),
        ("Top Up berhasil",
         "Top up GoPay sebesar {a} dari {m} berhasil.\n\nWaktu: {d}\n",
         "credit", "abbr_m", BANKS, {"space": False}),
        ("Kamu menerima saldo",
         "Kamu menerima {a} dari {m} di GoPay.\n\nWaktu: {d}\n",
         "credit", "abbr_m", PEOPLE, {"space": False}),
    ]),
    "ovo": ("OVO <no-reply@ovo.id>", [
        ("Pembayaran Berhasil",
         "Pembayaran Berhasil\n\nNama Merchant\n{m}\nTanggal & Waktu\n{d}\nMetode Pembayaran\n"
         "OVO Cash\nTotal Pembayaran\n{a}\n",
         "debit", "abbr_m", SHOPS, {"space": False}),
        ("Top Up Berhasil",
         "Top Up Berhasil\n\nMetode\n{m}\nTanggal & Waktu\n{d}\nJumlah Top Up\n{a}\n",
         "credit", "abbr_m", BANKS, {"space": False}),
    ]),
    "dana": ("DANA <no-reply@dana.id>", [
        ("Pembayaran berhasil",
         "Hai,\n\nKamu berhasil membayar {a} ke {m}.\nTanggal: {d}\nMetode: Saldo DANA\n",
         "debit", "slash_m", SHOPS, {}),
        ("Kamu menerima uang",
         "Hai,\n\nKamu menerima {a} dari {m}.\nTanggal: {d}\n",
         "credit", "slash_m", PEOPLE, {}),
        ("Top Up berhasil",
         "Hai,\n\nTop Up saldo DANA sebesar {a} melalui {m} berhasil.\nTanggal: {d}\n",
         "credit", "slash_m", BANKS, {}),
    ]),
    "shopeepay": ("ShopeePay <noreply@shopee.co.id>", [
        ("Pembayaran ShopeePay berhasil",
         "Pembayaran ShopeePay berhasil\n\nMerchant: {m}\nTotal Pembayaran: {a}\n"
         "Waktu Transaksi: {d}\nNo. Pesanan: 261121ABC123\n",
         "debit", "dash_m", SHOPS, {"space": False}),
        ("Isi Saldo ShopeePay berhasil",
         "Isi Saldo ShopeePay berhasil\n\nMetode: {m}\nJumlah: {a}\nWaktu Transaksi: {d}\n",
         "credit", "dash_m", BANKS, {"space": False}),
        ("Pengembalian Dana ShopeePay",
         "Pengembalian Dana ShopeePay berhasil\n\nMerchant: {m}\nJumlah Pengembalian: {a}\n"
         "Waktu Transaksi: {d}\n",
         "credit", "dash_m", SHOPS, {"space": False}),
    ]),
}

# Sender tak dikenal -> aturan generik. (from, subject, body, type, amount_minor)
GENERIC = [
    ("BNI <notifikasi@bni.co.id>", "Transaksi Berhasil",
     "Transaksi Anda sebesar Rp 75.000 di ALFAMIDI telah berhasil.", "debit", 7_500_000),
    ("CIMB Niaga <noreply@cimbniaga.co.id>", "Notifikasi Pembayaran",
     "Pembayaran tagihan sebesar IDR 1.250.000,00 telah diproses.", "debit", 125_000_000),
    ("PermataBank <no-reply@permatabank.co.id>", "Dana Masuk",
     "Rekening Anda menerima dana sebesar Rp5.000.000 dari PT ABC.", "credit", 500_000_000),
    ("SeaBank <noreply@seabank.co.id>", "Transfer Berhasil",
     "Transfer Rp. 350.000 ke rekening tujuan berhasil.", "debit", 35_000_000),
    ("LinkAja <info@linkaja.id>", "Top Up Berhasil",
     "Saldo LinkAja kamu bertambah Rp100.000.", "credit", 10_000_000),
    ("blu <noreply@blubybcadigital.id>", "Transaksi QRIS berhasil",
     "Kamu bayar Rp 18.500 pakai QRIS.\nBiaya: Rp0", "debit", 1_850_000),
    ("Bank Danamon <notif@danamon.co.id>", "Notifikasi Transaksi Kartu Kredit",
     "Transaksi Kartu Kredit Anda sebesar Rp 2.345.678,90 di TRAVELOKA.", "debit", 234_567_890),
    ("Bank Neo <noreply@bankneo.co.id>", "Cashback diterima - transaksi berhasil",
     "Selamat! Cashback Rp 5.000 sudah masuk ke saldo kamu.", "credit", 500_000),
    ("OCBC <notifikasi@ocbc.id>", "Pembayaran Berhasil",
     "Biaya admin: Rp0\nJumlah pembayaran: Rp 420.000", "debit", 42_000_000),
    ("Maybank <noreply@maybank.co.id>", "Payment Receipt",
     "Your payment of IDR 99,000.00 was successful.", "debit", 9_900_000),
    ("Bank Jago <noreply@jago.com>", "Refund berhasil",
     "Pengembalian dana Rp 64.900 dari merchant sudah kami proses.", "credit", 6_490_000),
]

# Harus gagal parse -> unparsed_emails. (from, subject, body)
UNPARSED = [
    ("BCA <bca@bca.co.id>", "Kode OTP BCA", "Kode OTP Anda 123456. Jangan berikan kepada siapa pun."),
    ("GoPay <no-reply@gopay.co.id>", "Promo akhir tahun: cashback hingga Rp50.000",
     "Nikmati cashback hingga Rp50.000 untuk pembelian pertama kamu."),
    ("DANA <no-reply@dana.id>", "Pembaruan Syarat dan Ketentuan",
     "Kami memperbarui Syarat dan Ketentuan layanan DANA."),
    ("Jenius <no-reply@jenius.com>", "Login baru terdeteksi",
     "Kami mendeteksi login baru ke akun Jenius kamu dari perangkat lain."),
    ("Livin' by Mandiri <noreply.livin@bankmandiri.co.id>", "Pembayaran Berhasil",
     "Hai RINA,\nPembayaran kamu berhasil! Rincian tersedia di aplikasi."),
]


def iso(t):
    return t.astimezone(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def main():
    out = []
    start = dt.datetime(2026, 10, 1, 7, 5, 9, tzinfo=WIB)
    step = dt.timedelta(days=4, hours=3, minutes=17, seconds=11)

    for p, (provider, (sender, templates)) in enumerate(T.items()):
        for i in range(12):
            subject, body, typ, date_key, pool, opt = templates[i % len(templates)]
            fmt_date, has_seconds = DATE[date_key]
            t = start + step * i + dt.timedelta(hours=p)
            amount = AMOUNTS[(i + p) % len(AMOUNTS)]
            merchant = pool[(i + p) % len(pool)]
            fee = opt.get("fee", 0)
            style = {"space": opt.get("space", True), "dec": opt.get("dec", False)}
            out.append({
                "id": f"{sender.split('@')[1].rstrip('>')}-{i:02d}",
                "from": sender,
                "subject": subject,
                "body": body.format(m=merchant, a=rp(amount, **style), d=fmt_date(t),
                                    fee=rp(fee, **style), total=rp(amount + fee, **style)),
                "received_at": iso(t + dt.timedelta(minutes=1)),
                "expect": {
                    "provider": provider,
                    "amount_minor": amount + fee,
                    "type": typ,
                    "merchant": merchant,
                    "occurred_at": iso(t if has_seconds else t.replace(second=0)),
                },
            })

    for i, (sender, subject, body, typ, amount) in enumerate(GENERIC):
        received = iso(start + step * i + dt.timedelta(hours=12))
        out.append({
            "id": f"generic-{i:02d}", "from": sender, "subject": subject, "body": body,
            "received_at": received,
            "expect": {"provider": "generic", "amount_minor": amount, "type": typ,
                       "merchant": None, "occurred_at": received},
        })

    for i, (sender, subject, body) in enumerate(UNPARSED):
        out.append({
            "id": f"unparsed-{i:02d}", "from": sender, "subject": subject, "body": body,
            "received_at": iso(start + step * i + dt.timedelta(hours=18)), "expect": None,
        })

    path = pathlib.Path(__file__).with_name("emails.json")
    path.write_text(json.dumps(out, ensure_ascii=False, indent=1) + "\n")
    print(f"{len(out)} email -> {path}")


if __name__ == "__main__":
    main()
