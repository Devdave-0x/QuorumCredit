use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use chrono::{Duration, Utc};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token expired")]
    TokenExpired,
    #[error("Missing authorization header")]
    MissingHeader,
    #[error("Invalid authorization header format")]
    InvalidHeaderFormat,
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
    pub api_key: String,
}

pub struct JwtAuth {
    secret: String,
}

impl JwtAuth {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn generate_token(&self, api_key: &str, expires_in_hours: i64) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = (now + Duration::hours(expires_in_hours)).timestamp();
        let iat = now.timestamp();

        let claims = Claims {
            sub: api_key.to_string(),
            exp,
            iat,
            api_key: api_key.to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )?;

        Ok(token)
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )?;

        Ok(data.claims)
    }

    pub fn extract_token_from_header(auth_header: &str) -> Result<String, AuthError> {
        let parts: Vec<&str> = auth_header.split_whitespace().collect();
        
        if parts.len() != 2 || parts[0] != "Bearer" {
            return Err(AuthError::InvalidHeaderFormat);
        }

        Ok(parts[1].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_token() {
        let auth = JwtAuth::new("test_secret".to_string());
        let token = auth.generate_token("test_api_key", 24).unwrap();
        let claims = auth.verify_token(&token).unwrap();
        
        assert_eq!(claims.api_key, "test_api_key");
    }

    #[test]
    fn test_extract_token_from_header() {
        let header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let token = JwtAuth::extract_token_from_header(header).unwrap();
        assert_eq!(token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
    }

    #[test]
    fn test_invalid_header_format() {
        let header = "InvalidFormat token";
        let result = JwtAuth::extract_token_from_header(header);
        assert!(result.is_err());
    }
}
