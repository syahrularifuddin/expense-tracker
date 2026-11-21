# Expense Tracker

Records transactions automatically from bank and e-wallet notification emails in Gmail.
Rust backend (axum + sqlx + PostgreSQL), Next.js 16 frontend (Auth.js, TanStack Query, shadcn/ui).

```
.
├── backend/    # Rust crate: src/, migrations/, config/senders.toml, fixtures/, tests/
├── frontend/   # Next.js
└── docker-compose.yml   # PostgreSQL 16 (dev)
```

Requirements: Docker, Rust (stable), Node.js 20+.

## Quick start (mock mode, no Google account)

Mock mode reads sample emails from `backend/fixtures/emails.json` and shows a
"Masuk sebagai user dev" (sign in as dev user) button, so you can try the full
login → sync → dashboard flow without OAuth.

```bash
docker compose up -d
```

```bash
cd backend && cp .env.example .env && cargo run
```

Set `JWT_SECRET` in `backend/.env` to a random value of at least 32 characters
(for example `openssl rand -hex 32`).

```bash
cd frontend && cp .env.example .env.local && npm install && npm run dev
```

Set `AUTH_SECRET` in `frontend/.env.local` (`openssl rand -base64 32`), then open
<http://localhost:3000> and click **Masuk sebagai user dev**.

- API: <http://127.0.0.1:8080/api/v1>, Swagger docs: <http://127.0.0.1:8080/docs>
- If port 3000 is taken: run `npm run dev -- -p 3100`, set
  `FRONTEND_ORIGIN=http://localhost:3100` in `backend/.env` (CORS allows only that
  origin), and restart the backend.

## Using real Gmail

### 1. Google Cloud Console

1. Create a project at <https://console.cloud.google.com>.
2. **APIs & Services → Library**: enable the **Gmail API**.
3. **OAuth consent screen**: user type *External*, publishing status *Testing*.
   Add the `.../auth/gmail.readonly` scope. Under **Test users**, add the Gmail
   account you will sign in with.
4. **Credentials → Create credentials → OAuth client ID**, type **Web application**.
   Authorized redirect URI: `http://localhost:3000/api/auth/callback/google`
   (change the port if you don't use 3000).
5. Copy the Client ID and Client Secret.

Note: `gmail.readonly` is a *restricted scope*. While the consent screen is in
*Testing*, only test users can sign in and refresh tokens expire after 7 days
(the sync status then shows "Perlu login ulang", i.e. sign in again). Lifting
these limits requires Google's app verification.

### 2. Environment

`backend/.env`:

| Variable | Description |
|---|---|
| `DATABASE_URL` | PostgreSQL connection string |
| `BIND_ADDR` | Backend address, default `127.0.0.1:8080` |
| `FRONTEND_ORIGIN` | Frontend origin allowed by CORS, default `http://localhost:3000` |
| `JWT_SECRET` | Access JWT signing secret, at least 32 characters |
| `MOCK_GMAIL` | Set to `false` to use real Gmail |
| `TOKEN_ENCRYPTION_KEY` | 32-byte AES-256-GCM key, base64: `openssl rand -base64 32` |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | From step 1 |

`frontend/.env.local`:

| Variable | Description |
|---|---|
| `AUTH_SECRET` | Auth.js session cookie secret |
| `BACKEND_URL` | Backend URL as seen from the Next.js server |
| `NEXT_PUBLIC_API_URL` | Backend URL as seen from the browser |
| `MOCK_GMAIL` | `false` hides the dev user button |
| `AUTH_GOOGLE_ID`, `AUTH_GOOGLE_SECRET` | Same values as the backend Client ID / Secret |

Restart the backend and frontend, then sign in with Google. The first sync pulls
the last 90 days of email from enabled senders; later syncs are incremental,
every 5 minutes.

## How it works

- **Login**: on the server side, Auth.js sends the Google ID token and the Gmail
  refresh token to the backend (`POST /auth/exchange`). The Gmail refresh token is
  stored encrypted (AES-256-GCM) and never reaches the browser. The backend issues
  a 15-minute access JWT plus a session refresh token (hashed, rotated, 30 days).
- **Sync**: a background worker per user. Emails are parsed by a rule engine
  (`backend/config/senders.toml`: sender + regex with `amount`, `merchant`, `date`,
  `type` groups). Emails that fail to parse appear under "Email gagal diproses"
  (failed emails) in Settings.
- **Amounts** are stored in cents (`amount_minor`), times in UTC, displayed in
  Asia/Jakarta time.

The app UI is in Indonesian.

## Adding a bank or e-wallet

Add a `[[rule]]` to `backend/config/senders.toml`, add sample emails to
`backend/fixtures/generate.py`, then run:

```bash
cd backend && python3 fixtures/generate.py && cargo test
```

Global rules are re-seeded from that file every time the backend starts.

## Checks

```bash
cd backend && cargo test && cargo clippy --all-targets -- -D warnings
```

```bash
cd frontend && npm run lint && npm run build
```

## Known limitations

- The email templates and sender domains in `senders.toml` are guesses and have
  not been matched against real emails from each bank. The real Gmail path (OAuth,
  `history.list`, MIME parsing) has never run against a real account; only mock
  mode is tested end to end.
- The generic rule guesses debit/credit from keywords; failed or declined
  transaction emails may be recorded as transactions.
- A full sync is capped at 500 emails (5 pages × 100).
- Polling only, no Pub/Sub push. Single currency (IDR). No production deployment
  configuration.
