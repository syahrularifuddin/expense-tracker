# Prompt: Aplikasi Expense Tracker dari Notifikasi Email (Rust + Next.js)

Kamu adalah senior fullstack engineer. Bangun aplikasi **Expense Tracker** yang otomatis mencatat transaksi pengguna dengan membaca notifikasi email (bank / e-wallet) di Gmail mereka. Pengguna login dengan Google. Backend dengan **Rust**, frontend dengan **Next.js**, tampilan **minimalis lembut dengan kartu membulat dan kontrol pill** (lihat bagian "Desain UI").

## Tech Stack (wajib)

- **Backend**: Rust — `axum`, `tokio`, `sqlx` (PostgreSQL), `utoipa` + `utoipa-swagger-ui` (OpenAPI + Swagger UI), `validator`, `tracing`, `jsonwebtoken`, `aes-gcm`, `reqwest`. Gmail API dan OAuth Google dipanggil langsung via REST dengan `reqwest` (tidak memakai crate `google-gmail1`).
- **Frontend**: Next.js 16 (App Router, TypeScript), Tailwind CSS + shadcn/ui, Auth.js (`next-auth` v5) untuk Google OAuth, TanStack Query, Recharts.
- **DB**: PostgreSQL 16 via docker compose (dev).
- **Struktur monorepo** (root = direktori repo ini):
  ```
  .
  ├── backend/    # satu crate Rust: src/, migrations/, fixtures/, tests/
  ├── frontend/   # Next.js
  ├── docker-compose.yml
  └── README.md
  ```

## Alur Autentikasi

1. Frontend: login Google via Auth.js dengan scope `openid email profile https://www.googleapis.com/auth/gmail.readonly`, plus `access_type=offline` **dan `prompt=consent`** (tanpa `prompt=consent` Google hanya mengirim refresh_token pada persetujuan pertama).
2. Di callback `jwt` Auth.js (**sisi server, bukan browser**), frontend memanggil `POST /api/v1/auth/exchange` membawa ID token Google + refresh token Gmail. Refresh token Gmail tidak boleh pernah sampai ke browser. Backend:
   - verifikasi ID token via JWKS Google (cek `iss`, `aud` = `GOOGLE_CLIENT_ID`, `exp`, `email_verified`);
   - upsert user;
   - simpan refresh token Gmail **terenkripsi AES-256-GCM** (kunci 32 byte dari env `TOKEN_ENCRYPTION_KEY`, nonce acak per record);
   - terbitkan **access JWT** (exp 15 menit) + **refresh token sesi** (opaque, acak, disimpan sebagai hash di tabel `sessions`, dirotasi setiap dipakai, exp 30 hari).
3. Auth.js menyimpan token sesi backend di cookie sesi terenkripsi dan memperbaruinya lewat `POST /auth/refresh` saat access JWT hampir kedaluwarsa. Browser memanggil backend dengan `Authorization: Bearer <access JWT>`.
4. Semua endpoint selain `healthz`, `auth/exchange`, `auth/refresh`, dan `/docs` dijaga middleware JWT. CORS dibatasi hanya origin frontend (`FRONTEND_ORIGIN`). Refresh token (Gmail maupun sesi) tidak boleh pernah ter-log.
5. Backend memegang `GOOGLE_CLIENT_ID` + `GOOGLE_CLIENT_SECRET` untuk menukar refresh token Gmail menjadi access token saat sinkronisasi. Jika Google membalas `invalid_grant`, tandai koneksi user sebagai terputus (`sync_state.status = needs_reauth`) dan hentikan polling user itu sampai login ulang.
6. `disconnect-google`: revoke token di `https://oauth2.googleapis.com/revoke`, hapus baris `google_tokens`, hentikan sinkronisasi. Transaksi yang sudah ada tetap disimpan.

**Mode dev tanpa Google** (`MOCK_GMAIL=true`): backend membuka `POST /api/v1/auth/dev-login` (hanya aktif di mode ini) yang menerbitkan sesi untuk satu user dev; frontend menampilkan tombol "Masuk sebagai user dev" via provider Credentials Auth.js. Tanpa ini alur end-to-end tidak bisa dites tanpa OAuth.

## Ingest Email → Transaksi

- **Background worker** (tokio task, polling tiap 5 menit per user, dengan jitter; backoff eksponensial pada HTTP 429/5xx):
  - Sinkron pertama: `users.messages.list` dengan query `from:(a OR b ...) newer_than:90d`, lalu simpan `historyId` terbaru.
  - Sinkron berikutnya: `users.history.list` dengan `startHistoryId` + `historyTypes=messageAdded`; pesan yang sender-nya tidak ada di daftar dilewati. Jika Gmail membalas **HTTP 404** (historyId kedaluwarsa/invalid), fallback ke full search lalu reset `historyId`.
  - Ambil isi dengan `users.messages.get?format=full`: decode base64url, pilih part `text/plain`; jika hanya ada `text/html`, buang tag menjadi teks sebelum diparse.
- **Daftar sender**: default berasal dari satu file konfigurasi (`backend/config/senders.toml`) yang di-seed sebagai aturan global (`parser_rules.user_id IS NULL`). User dapat menonaktifkan sender global atau menambah sender sendiri di settings.
- **Parser berbasis aturan (rule engine)**:
  - Tabel `parser_rules`: matcher (sender / pola subject) + regex dengan named capture groups: `amount`, `merchant`, `date`, `type`.
  - Seed aturan untuk notifikasi umum bank & e-wallet Indonesia (BCA, Mandiri, BRI, Jenius, GoPay, OVO, DANA, ShopeePay) + 1 aturan generik fallback.
  - Nominal format Indonesia: `Rp1.500.000`, `Rp 12.500,50`. Disimpan sebagai `amount_minor` BIGINT dalam **sen** (`Rp 12.500,50` → `1250050`, `Rp1.500.000` → `150000000`). Tidak memakai float.
  - Nilai `type` hanya `debit` atau `credit`. Top-up/penerimaan → `credit` (bukan pengeluaran). Jika regex tidak menangkap `type`, pakai `default_type` dari aturan.
  - Tanggal: parse dari body dengan zona Asia/Jakarta; jika tidak ada, pakai `internalDate` Gmail. Simpan sebagai `timestamptz` (UTC).
- **Dedup**: unique constraint `(user_id, gmail_message_id)`; transaksi manual memakai `gmail_message_id = NULL` (tidak bentrok di Postgres).
- **Gagal parse**: masuk tabel `unparsed_emails` (bisa dilihat/diabaikan user). Simpan hanya sender, subject, snippet, dan tanggal — **bukan** body penuh.
- **Kategorisasi otomatis**: tabel `category_rules` (keyword merchant → kategori, case-insensitive). Override manual oleh user selalu menang: kolom `transactions.category_locked = true` membuat kategorisasi otomatis tidak menyentuh baris itu lagi.

## Skema DB (minimum)

- `users` (id, google_sub unik, email, name, avatar_url)
- `google_tokens` (user_id, refresh_token_ciphertext, nonce)
- `sessions` (id, user_id, refresh_token_hash, expires_at, revoked_at)
- `transactions` (id, user_id, occurred_at, amount_minor, currency, type, merchant, description, category_id, category_locked, source = `gmail`/`manual`, gmail_message_id, raw_subject)
- `categories` (default: user_id NULL; custom: per user)
- `parser_rules` (user_id nullable, sender, subject_pattern, body_regex, default_type, enabled, priority)
- `category_rules` (user_id nullable, keyword, category_id)
- `unparsed_emails` (user_id, gmail_message_id, sender, subject, snippet, received_at, ignored)
- `sync_state` (user_id, history_id, last_synced_at, status, last_error)

Gunakan `sqlx migrate`. Index: `transactions (user_id, occurred_at DESC)`.

## API (REST, prefix `/api/v1`, dokumentasi utoipa di `/docs`)

- **auth**: `POST exchange`, `POST refresh`, `GET me`, `POST logout`, `POST disconnect-google`, `POST dev-login` (hanya `MOCK_GMAIL=true`)
- **transactions**: GET (filter `from`/`to`/`category_id`/`source`/`type`/`q` + pagination `page`/`per_page`), POST manual, PATCH, DELETE
- **categories**: CRUD (kategori default tidak bisa dihapus)
- **senders**: GET daftar sender (global + milik user), POST tambah, PATCH aktif/nonaktif, DELETE milik user
- **unparsed-emails**: GET (pagination), PATCH `ignored`
- **reports**: GET `/summary?month=YYYY-MM` (total pengeluaran, total income, breakdown per kategori & per hari; batas bulan dihitung di Asia/Jakarta), GET `/trends?months=6` (total debit/credit per bulan)
- **sync**: POST `/sync` (trigger manual, balas 202), GET `/sync/status`
- **healthz** (tanpa auth, cek koneksi DB)

Error selalu `application/problem+json` (RFC 9457: `type`, `title`, `status`, `detail`).

## Halaman Frontend

- `/login` — tombol login Google (+ tombol user dev jika mode mock)
- `/` — dashboard: kartu ringkasan bulan ini, donat per kategori, grafik harian, 10 transaksi terakhir
- `/transactions` — tabel dengan filter, search, pagination, edit kategori inline, tambah transaksi manual
- `/reports` — ringkasan bulanan + tren
- `/settings` — status sinkronisasi, tombol "Sync sekarang", kelola kategori, kelola sender, daftar email gagal parse (abaikan), disconnect Google
- Dark mode, responsif. Rupiah pakai `Intl.NumberFormat('id-ID', { style: 'currency', currency: 'IDR' })`, timezone Asia/Jakarta.

## Desain UI (minimalis lembut, kartu membulat + kontrol pill)

Acuan visual: dashboard bergaya Apple-ish yang lembut — latar abu sangat muda, kartu putih dengan sudut sangat membulat dan bayangan halus, semua kontrol berbentuk pill, satu aksen oranye. Implementasi dengan **Tailwind + shadcn/ui** yang di-tema lewat CSS variables (`frontend/app/globals.css`); tanpa paket UI tambahan.

- **Token warna**:

  | Token | Light | Dark |
  |---|---|---|
  | Latar halaman | `#F3F3F5` | `#111113` |
  | Kartu / popover | `#FFFFFF` | `#1C1C1F` |
  | Pill sekunder, track nav | `#EDEDF0` | `#2A2A2E` |
  | Track meter | `#ECECEF` | `#2C2C31` |
  | Border tipis | `#E6E6EA` | `#2C2C31` |
  | Teks utama | `#1D1D1F` | `#F2F2F4` |
  | Teks sekunder | `#6E6E73` | `#A1A1A8` |
  | Aksen (brand, fokus, grafik utama) | `#E8833A` | `#F0954F` |
  | Aksi utama (tombol primary) | `#1D1D1F` | `#F2F2F4` |
  | Sukses (pemasukan, "Tersinkron") | `#2F8F5B` | `#5CC285` |
  | Bahaya (hapus) | `#C93C37` | `#F2736B` |

  Oranye tidak dipakai sebagai latar teks putih (kontras kurang); untuk itu aksi utama memakai warna gelap/netral.
- **Tipografi**: Inter, dasar 15px/22px. Judul halaman 28px semibold rapat (`tracking-tight`), judul kartu 18px semibold. Label metrik memakai teks sekunder ukuran normal (tidak uppercase), kecuali header tabel: 12px uppercase spasi lebar. Angka uang `tabular-nums`.
- **Bentuk & jarak**: kartu radius 28px, padding 24px, tanpa border, bayangan sangat halus. Tombol, input, select, lozenge, dan nav = `rounded-full`. Jarak antar kartu 20px.
- **Layout aplikasi**: tanpa sidebar. Header melayang berbentuk pill (logo bulat oranye, nama aplikasi + subjudul abu, di kanan: pill status sinkron, tombol "Sync", toggle tema, avatar menu). Di bawahnya nav tab segmented (track abu, tab aktif = pill putih berbayangan), bisa di-scroll horizontal di mobile. Konten maksimum ~1240px.
- **Komponen**:
  - Tombol: primary gelap solid, secondary pill abu muda (paling sering dipakai), ghost untuk ikon.
  - Lozenge/status pill: latar lembut + teks berwarna + titik bulat di depan (Tersinkron hijau, Menyinkronkan oranye, Gagal merah). Badge info tanpa titik (mis. "Gmail", "Mode mock") memakai oranye lembut.
  - **Baris metrik** (seperti panel mesin): kolom kiri label abu, kolom kanan nilai tebal + keterangan kanan abu, di bawahnya meter tipis 6px membulat. Dipakai untuk breakdown kategori (menggantikan donut).
  - Tabel: tanpa latar header, baris 48px, pemisah horizontal tipis, nominal rata kanan.
  - Edit inline: klik sel kategori → select pill, simpan otomatis.
  - Empty state (ikon garis + satu kalimat + satu aksi) dan skeleton membulat saat memuat.
- **Grafik (Recharts)**: bar membulat penuh, oranye untuk pengeluaran, abu-biru `#7C8597` untuk pemasukan, grid horizontal tipis, tooltip kartu membulat tanpa border, tanpa animasi/gradien.
- **Aksesibilitas**: kontras teks minimal WCAG AA di kedua tema, semua kontrol bisa dipakai dengan keyboard dengan ring fokus oranye terlihat, meter punya `role="meter"` + label, warna tidak menjadi satu-satunya pembeda debit/credit (tanda `−`/`+` dan lozenge).

## Kualitas & Verifikasi (syarat "selesai")

1. **Unit test parser WAJIB**: ≥ 10 sampel subject/body email yang realistis (dianonimkan) untuk setiap provider yang di-seed, mencakup debit dan credit serta kedua format nominal; jalankan dengan `cargo test` — semua hijau.
2. **Mode dev tanpa kredensial Google**: env `MOCK_GMAIL=true` → worker membaca fixture email JSON dari `backend/fixtures/` alih-alih Gmail asli, dan login memakai `dev-login`, sehingga alur end-to-end bisa dites tanpa OAuth.
3. `docker compose up` menghidupkan Postgres; backend & frontend bisa dijalankan dari langkah README.
4. README berisi: setup Google Cloud Console (buat OAuth Client tipe Web, redirect URI `http://localhost:3000/api/auth/callback/google`, enable Gmail API, tambah test user), catatan bahwa `gmail.readonly` adalah *restricted scope* dan pada status "Testing" refresh token kedaluwarsa setelah 7 hari, `.env.example` lengkap (backend & frontend), cara run backend & frontend.
5. Tidak ada panic di handler (tanpa `unwrap`/`expect` di jalur request); error konsisten sebagai problem JSON; `cargo clippy -- -D warnings` dan `next build` sukses tanpa error TypeScript.
6. UI sesuai bagian "Desain UI" di light dan dark mode, dan tetap dapat dipakai pada lebar 375px.

## Non-goals (jangan dikerjakan)

- Gmail push notification (Pub/Sub) — polling cukup untuk v1.
- OCR lampiran / PDF statement.
- Multi-currency (simpan field currency, tampilkan IDR saja).
- Konfigurasi deployment selain docker-compose dev.
- Editor regex `parser_rules` di UI (user hanya mengelola daftar sender; aturan diubah lewat seed/migrasi).
- Paket komponen UI tambahan atau design system di luar tema shadcn.

## Urutan Pengerjaan

1. Scaffold backend + migrasi + docker-compose → tulis test parser dulu → `cargo test` hijau.
2. Auth exchange + refresh + middleware JWT (+ `dev-login` untuk mode mock).
3. Gmail sync worker + parser (+ mode `MOCK_GMAIL`) → transaksi terbentuk dari fixture.
4. API transactions / categories / senders / unparsed-emails / reports.
5. Frontend: tema & layout (token warna, header pill + tab nav) → auth → dashboard → transactions.
6. Settings + reports + README.

Selesai berarti semua poin di bagian "Kualitas & Verifikasi" terpenuhi dan alur login → sinkron email → transaksi muncul di dashboard berjalan end-to-end.
