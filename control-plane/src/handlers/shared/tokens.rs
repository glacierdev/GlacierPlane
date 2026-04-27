use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};

pub(crate) fn parse_authorization_token(header: &str) -> Option<String> {
    let token = header
        .strip_prefix("Token ")
        .or_else(|| header.strip_prefix("Bearer "));
    token.filter(|t| !t.is_empty()).map(|t| t.to_string())
}

pub(crate) fn extract_registration_token(header: &str) -> Option<String> {
    tracing::info!("{}", header);
    parse_authorization_token(header)
}

pub(crate) fn generate_secure_token(length: usize) -> String {
    let mut rng = OsRng;
    let bytes: String = (0..length)
        .map(|_| rng.sample(Alphanumeric) as char)
        .collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_token_prefix() {
        assert_eq!(
            parse_authorization_token("Token abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn parse_bearer_prefix() {
        assert_eq!(
            parse_authorization_token("Bearer xyz789"),
            Some("xyz789".to_string())
        );
    }

    #[test]
    fn reject_unknown_prefix() {
        assert_eq!(parse_authorization_token("Basic abc123"), None);
    }

    #[test]
    fn reject_empty_string() {
        assert_eq!(parse_authorization_token(""), None);
    }

    #[test]
    fn reject_prefix_only_no_token() {
        assert_eq!(parse_authorization_token("Token "), None);
    }

    #[test]
    fn reject_bearer_prefix_only() {
        assert_eq!(parse_authorization_token("Bearer "), None);
    }

    #[test]
    fn reject_prefix_without_space() {
        assert_eq!(parse_authorization_token("Tokenabc123"), None);
    }

    #[test]
    fn parse_token_with_special_chars() {
        assert_eq!(
            parse_authorization_token("Token abc+/123="),
            Some("abc+/123=".to_string())
        );
    }

    #[test]
    fn parse_bearer_with_long_token() {
        let long_token = "a".repeat(256);
        assert_eq!(
            parse_authorization_token(&format!("Bearer {}", long_token)),
            Some(long_token)
        );
    }

    #[test]
    fn generate_token_has_reasonable_length() {
        let token = generate_secure_token(32);
        assert!(token.len() >= 32);
    }

    #[test]
    fn generate_token_is_unique() {
        let a = generate_secure_token(32);
        let b = generate_secure_token(32);
        assert_ne!(a, b);
    }

    #[test]
    fn generate_token_is_url_safe_base64() {
        let token = generate_secure_token(32);
        assert!(
            token
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        );
    }
}
