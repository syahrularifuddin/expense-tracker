use crate::parser::BoxError;
use base64::{Engine, engine::general_purpose::STANDARD};

pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub frontend_origin: String,
    pub jwt_secret: String,
    /// Kunci AES-256-GCM untuk refresh token Gmail.
    pub token_key: [u8; 32],
    pub google_client_id: String,
    pub google_client_secret: String,
    pub mock_gmail: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, BoxError> {
        let var = |k: &str| std::env::var(k).map_err(|_| format!("env {k} wajib diisi"));
        let mock_gmail = std::env::var("MOCK_GMAIL").is_ok_and(|v| v == "true");
        // Kredensial Google dan kunci enkripsi hanya wajib di luar mode mock.
        let google = |k: &str| if mock_gmail { Ok(var(k).unwrap_or_default()) } else { var(k) };

        let key_b64 = google("TOKEN_ENCRYPTION_KEY")?;
        let token_key = if key_b64.is_empty() {
            [0; 32]
        } else {
            STANDARD
                .decode(key_b64.trim())?
                .try_into()
                .map_err(|_| "TOKEN_ENCRYPTION_KEY harus 32 byte (base64)")?
        };
        let jwt_secret = var("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err("JWT_SECRET minimal 32 karakter".into());
        }

        Ok(Self {
            database_url: var("DATABASE_URL")?,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            frontend_origin: std::env::var("FRONTEND_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:3000".into()),
            jwt_secret,
            token_key,
            google_client_id: google("GOOGLE_CLIENT_ID")?,
            google_client_secret: google("GOOGLE_CLIENT_SECRET")?,
            mock_gmail,
        })
    }
}
