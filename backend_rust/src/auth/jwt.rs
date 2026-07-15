use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use crate::errors::AppError;
use crate::config::Config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    pub jti: Uuid,
    pub device_fingerprint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshClaims {
    pub sub: Uuid,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    pub family: String,
}

pub fn generate_access_token(
    user_id: Uuid,
    role: &str,
    device_fingerprint: Option<String>,
    config: &Config,
) -> Result<String, AppError> {
    let now = Utc::now();
    let duration = Duration::minutes(config.jwt_access_token_expire_minutes);
    let exp = now + duration;
    
    let claims = Claims {
        sub: user_id,
        role: role.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        jti: Uuid::new_v4(),
        device_fingerprint,
    };
    
    let header = Header::new(jsonwebtoken::Algorithm::HS256);
    let key = EncodingKey::from_secret(config.jwt_secret_key.as_bytes());
    
    encode(&header, &claims, &key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to generate access token: {}", e)))
}

pub fn generate_refresh_token(
    user_id: Uuid,
    role: &str,
    family: &str,
    config: &Config,
) -> Result<String, AppError> {
    let now = Utc::now();
    let duration = Duration::days(config.jwt_refresh_token_expire_days);
    let exp = now + duration;
    
    let claims = RefreshClaims {
        sub: user_id,
        role: role.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        family: family.to_string(),
    };
    
    let header = Header::new(jsonwebtoken::Algorithm::HS256);
    let key = EncodingKey::from_secret(config.jwt_secret_key.as_bytes());
    
    encode(&header, &claims, &key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to generate refresh token: {}", e)))
}

pub fn decode_access_token(
    token: &str,
    config: &Config,
) -> Result<Claims, AppError> {
    let key = DecodingKey::from_secret(config.jwt_secret_key.as_bytes());
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    
    let token_data = decode::<Claims>(token, &key, &validation)
        .map_err(|e| AppError::Unauthorized(format!("Invalid access token: {}", e)))?;
        
    Ok(token_data.claims)
}

pub fn decode_refresh_token(
    token: &str,
    config: &Config,
) -> Result<RefreshClaims, AppError> {
    let key = DecodingKey::from_secret(config.jwt_secret_key.as_bytes());
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    
    let token_data = decode::<RefreshClaims>(token, &key, &validation)
        .map_err(|e| AppError::Unauthorized(format!("Invalid refresh token: {}", e)))?;
        
    Ok(token_data.claims)
}
