use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Generates a 256-bit random token, base64url-encoded without padding.
/// Used for OAuth authorization codes, access tokens and refresh tokens.
pub fn generate_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    use base64::Engine;
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Hex-encoded SHA-256 of a token. Tokens are high-entropy random values, so a
/// fast hash is sufficient for storage (unlike passwords, which need Argon2).
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Verifies a PKCE `code_verifier` against an `S256` `code_challenge` (RFC 7636).
pub fn verify_pkce_s256(code_verifier: &str, code_challenge: &str) -> bool {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    // RFC 7636 §4.1: 43-128 characters from the unreserved set.
    let valid_verifier = (43..=128).contains(&code_verifier.len())
        && code_verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'));
    if !valid_verifier {
        return false;
    }
    let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(code_verifier.as_bytes()));
    computed == code_challenge
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256_matches_independent_vector() {
        // Challenge computed with: printf '%s' <verifier> | openssl dgst -sha256 -binary | base64url
        let verifier = "test-verifier-0123456789abcdefghijklmnopqrstuvwxyz";
        let challenge = "_i87_9qTgIdW6lIGSOGQx4iLENnqnygSha1TvdNTuWo";
        assert!(verify_pkce_s256(verifier, challenge));
        assert!(!verify_pkce_s256(verifier, "wrong"));
    }

    #[test]
    fn pkce_rejects_malformed_verifier() {
        assert!(!verify_pkce_s256("too-short", "x"));
        let with_space = "a b".repeat(20);
        assert!(!verify_pkce_s256(&with_space, "x"));
    }

    #[test]
    fn hash_token_is_hex_sha256() {
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn generated_tokens_are_unique_and_url_safe() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(a.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
    }
}
