//! Authenticated, encrypted email-change links. Pending addresses are not stored.

use anyhow::{Result, anyhow, ensure};
use axum_extra::extract::cookie::Key;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    aead, digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PURPOSE: &[u8] = b"powpow/email-change/v1";
pub const LIFETIME: chrono::Duration = chrono::Duration::hours(24);

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailChange {
    pub staff_id: Uuid,
    pub token: Uuid,
    pub old_email: String,
    pub new_email: String,
    pub expires_at: i64,
}

fn cipher(key: &Key) -> Result<aead::LessSafeKey> {
    let mut context = digest::Context::new(&digest::SHA256);
    context.update(PURPOSE);
    context.update(key.encryption());
    let derived = context.finish();
    let key = aead::UnboundKey::new(&aead::AES_256_GCM, derived.as_ref())
        .map_err(|_| anyhow!("Cannot initialize email-change encryption"))?;
    Ok(aead::LessSafeKey::new(key))
}

pub fn seal(key: &Key, change: &EmailChange) -> Result<String> {
    let mut nonce = [0; aead::NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| anyhow!("Cannot generate email-change nonce"))?;
    let mut payload = serde_json::to_vec(change)?;
    cipher(key)?
        .seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(PURPOSE),
            &mut payload,
        )
        .map_err(|_| anyhow!("Cannot encrypt email-change link"))?;
    let mut sealed = nonce.to_vec();
    sealed.extend(payload);
    Ok(URL_SAFE_NO_PAD.encode(sealed))
}

pub fn open(key: &Key, token: &str, now: i64) -> Result<EmailChange> {
    ensure!(token.len() <= 4096, "Invalid email-change link");
    let mut sealed = URL_SAFE_NO_PAD.decode(token)?;
    ensure!(sealed.len() > aead::NONCE_LEN, "Invalid email-change link");
    let (nonce, payload) = sealed.split_at_mut(aead::NONCE_LEN);
    let nonce: [u8; aead::NONCE_LEN] = nonce.try_into()?;
    let plaintext = cipher(key)?
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(PURPOSE),
            payload,
        )
        .map_err(|_| anyhow!("Invalid email-change link"))?;
    let change: EmailChange = serde_json::from_slice(plaintext)?;
    ensure!(change.expires_at > now, "Expired email-change link");
    ensure!(
        crate::auto_import::valid_email(&change.new_email),
        "Invalid email address"
    );
    Ok(change)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_links_expire_and_reject_tampering_and_wrong_keys() -> Result<()> {
        let key = Key::generate();
        let change = EmailChange {
            staff_id: Uuid::new_v4(),
            token: Uuid::new_v4(),
            old_email: "old@example.org".into(),
            new_email: "new@example.org".into(),
            expires_at: 100,
        };
        let token = seal(&key, &change)?;
        assert_eq!(open(&key, &token, 99)?.new_email, change.new_email);
        assert!(open(&key, &token, 100).is_err());
        assert!(open(&Key::generate(), &token, 99).is_err());
        let mut bytes = URL_SAFE_NO_PAD.decode(&token)?;
        assert!(!String::from_utf8_lossy(&bytes).contains("new@example.org"));
        bytes[aead::NONCE_LEN] ^= 1;
        assert!(open(&key, &URL_SAFE_NO_PAD.encode(bytes), 99).is_err());
        assert!(open(&key, "invalid", 99).is_err());
        assert_ne!(seal(&key, &change)?, token);
        Ok(())
    }
}
