# Expense Tracker

Mencatat transaksi otomatis dari notifikasi email bank / e-wallet di Gmail.
Backend Rust (axum + sqlx + PostgreSQL), frontend Next.js 16 (Auth.js, TanStack Query, shadcn/ui).

```
.
├── backend/    # crate Rust: src/, migrations/, config/senders.toml, fixtures/, tests/
├── frontend/   # Next.js
└── docker-compose.yml   # PostgreSQL 16 (dev)
```

Butuh: Docker, Rust (stable), Node.js 20+.

## Mulai cepat (mode mock, tanpa Google)

Mode mock membaca email contoh dari `backend/fixtures/emails.json` dan menyediakan tombol
"Masuk sebagai user dev", sehingga alur login → sinkron → dashboard bisa dicoba tanpa OAuth.

```bash
docker compose up -d
```

```bash
cd backend && cp .env.example .env && cargo run
```

Ganti `JWT_SECRET` di `backend/.env` (minimal 32 karakter, mis. `openssl rand -hex 32`).

```bash
cd frontend && cp .env.example .env.local && npm install && npm run dev
```

Isi `AUTH_SECRET` di `frontend/.env.local` (`openssl rand -base64 32`), lalu buka
<http://localhost:3000> dan tekan **Masuk sebagai user dev**.

- API: <http://127.0.0.1:8080/api/v1>, dokumentasi Swagger: <http://127.0.0.1:8080/docs>
- Jika port 3000 terpakai: `npm run dev -- -p 3100`, lalu set `FRONTEND_ORIGIN=http://localhost:3100`
  di `backend/.env` (CORS hanya mengizinkan origin itu) dan jalankan ulang backend.

## Memakai Gmail asli

### 1. Google Cloud Console

1. Buat project di <https://console.cloud.google.com>.
2. **APIs & Services → Library**: aktifkan **Gmail API**.
3. **OAuth consent screen**: tipe *External*, status *Testing*. Tambahkan scope
   `.../auth/gmail.readonly`. Di **Test users**, tambahkan akun Gmail yang akan dipakai login.
4. **Credentials → Create credentials → OAuth client ID**, tipe **Web application**.
   Authorized redirect URI: `http://localhost:3000/api/auth/callback/google`
   (sesuaikan port jika bukan 3000).
5. Catat Client ID dan Client Secret.

Catatan: `gmail.readonly` adalah *restricted scope*. Selama consent screen berstatus *Testing*,
hanya test user yang bisa login dan refresh token kedaluwarsa setelah 7 hari (status sinkron
menjadi "Perlu login ulang"). Lepas dari batas itu butuh verifikasi aplikasi oleh Google.

### 2. Environment

`backend/.env`:

| Variabel | Keterangan |
|---|---|
| `DATABASE_URL` | Koneksi PostgreSQL |
| `BIND_ADDR` | Alamat backend, default `127.0.0.1:8080` |
| `FRONTEND_ORIGIN` | Origin frontend untuk CORS, default `http://localhost:3000` |
| `JWT_SECRET` | Penanda tangan access JWT, minimal 32 karakter |
| `MOCK_GMAIL` | Set `false` untuk Gmail asli |
| `TOKEN_ENCRYPTION_KEY` | Kunci AES-256-GCM 32 byte base64: `openssl rand -base64 32` |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | Dari langkah 1 |

`frontend/.env.local`:

| Variabel | Keterangan |
|---|---|
| `AUTH_SECRET` | Penanda tangan cookie sesi Auth.js |
| `BACKEND_URL` | Backend dilihat dari server Next.js |
| `NEXT_PUBLIC_API_URL` | Backend dilihat dari browser |
| `MOCK_GMAIL` | `false` menyembunyikan tombol user dev |
| `AUTH_GOOGLE_ID`, `AUTH_GOOGLE_SECRET` | Sama dengan Client ID / Secret backend |

Jalankan ulang backend dan frontend, lalu login dengan Google. Sinkron pertama menarik email
90 hari terakhir dari pengirim aktif; berikutnya inkremental tiap 5 menit.

## Cara kerja singkat

- **Login**: Auth.js menukar ID token Google + refresh token Gmail ke backend di sisi server
  (`POST /auth/exchange`). Refresh token Gmail disimpan terenkripsi (AES-256-GCM) dan tidak
  pernah sampai ke browser. Backend menerbitkan access JWT 15 menit + refresh token sesi
  (di-hash, dirotasi, 30 hari).
- **Sinkron**: worker latar belakang per user. Email diparse oleh rule engine
  (`backend/config/senders.toml`: pengirim + regex dengan group `amount`, `merchant`, `date`,
  `type`). Yang gagal diparse masuk daftar "Email gagal diproses" di Pengaturan.
- **Nominal** disimpan dalam sen (`amount_minor`), waktu dalam UTC, ditampilkan Asia/Jakarta.

## Menambah bank / e-wallet

Tambahkan `[[rule]]` di `backend/config/senders.toml`, tambahkan contoh email ke
`backend/fixtures/generate.py`, lalu:

```bash
cd backend && python3 fixtures/generate.py && cargo test
```

Aturan global di-seed ulang dari file itu setiap backend dijalankan.

## Pemeriksaan

```bash
cd backend && cargo test && cargo clippy --all-targets -- -D warnings
```

```bash
cd frontend && npm run lint && npm run build
```

## Batasan yang diketahui

- Template email dan domain pengirim di `senders.toml` adalah perkiraan, belum dicocokkan
  dengan email asli tiap bank. Jalur Gmail asli (OAuth, `history.list`, parsing MIME) belum
  pernah dijalankan terhadap akun sungguhan; yang teruji end-to-end hanya mode mock.
- Aturan generik menebak debit/credit dari kata kunci; email transaksi yang gagal/ditolak
  bisa tercatat sebagai transaksi.
- Sinkron penuh dibatasi 500 email (5 halaman × 100).
- Polling saja, tanpa push Pub/Sub. Satu mata uang (IDR). Tanpa konfigurasi deployment produksi.
