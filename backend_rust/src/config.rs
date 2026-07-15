use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub sqlite_url: String,
    pub jwt_secret_key: String,
    pub encryption_key: String,
    #[serde(default = "default_frontend_api_key")]
    pub frontend_api_key: String,
    #[serde(default = "default_jwt_algorithm")]
    pub jwt_algorithm: String,
    #[serde(default = "default_jwt_access_expire")]
    pub jwt_access_token_expire_minutes: i64,
    #[serde(default = "default_jwt_refresh_expire")]
    pub jwt_refresh_token_expire_days: i64,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub admin_email: String,
    pub admin_password: String,
    #[serde(default = "default_upload_dir")]
    pub upload_dir: String,
    #[serde(default = "default_max_upload_size")]
    pub max_upload_size_mb: i64,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_rust_log")]
    pub rust_log: String,
}

fn default_jwt_algorithm() -> String {
    "HS256".to_string()
}

fn default_frontend_api_key() -> String {
    "default_orbit_key_12345".to_string()
}

fn default_jwt_access_expire() -> i64 {
    30
}

fn default_jwt_refresh_expire() -> i64 {
    7
}

fn default_upload_dir() -> String {
    "./uploads".to_string()
}

fn default_max_upload_size() -> i64 {
    10
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_rust_log() -> String {
    "info".to_string()
}

impl Config {
    pub fn load() -> Result<Self, envy::Error> {
        dotenvy::dotenv().ok();
        envy::from_env::<Self>()
    }
}
