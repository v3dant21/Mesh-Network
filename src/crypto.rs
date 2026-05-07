use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::rngs::OsRng;
use rand::RngCore;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

pub struct CryptoState {
    secret: EphemeralSecret,
    pub public_key: PublicKey,
}

impl CryptoState {
    pub fn new() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public_key = PublicKey::from(&secret);
        Self { secret, public_key }
    }

    pub fn compute_shared_secret(self, peer_public: PublicKey) -> SessionCrypto {
        let shared_secret = self.secret.diffie_hellman(&peer_public);
        SessionCrypto::new(shared_secret)
    }
}

pub struct SessionCrypto {
    cipher: Aes256Gcm,
}

impl SessionCrypto {
    pub fn new(shared_secret: SharedSecret) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(shared_secret.as_bytes());
        let cipher = Aes256Gcm::new(key);
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let mut ciphertext = self.cipher.encrypt(nonce, plaintext)?;
        
        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.append(&mut ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, payload: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        if payload.len() < 12 {
            return Err(aes_gcm::Error);
        }
        let (nonce_bytes, ciphertext) = payload.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher.decrypt(nonce, ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let alice = CryptoState::new();
        let bob = CryptoState::new();

        let alice_pub = alice.public_key.clone();
        let bob_pub = bob.public_key.clone();

        let alice_session = alice.compute_shared_secret(bob_pub);
        let bob_session = bob.compute_shared_secret(alice_pub);

        let message = b"Hello Secure World";
        let encrypted = alice_session.encrypt(message).unwrap();
        
        assert_ne!(message, encrypted.as_slice());
        
        let decrypted = bob_session.decrypt(&encrypted).unwrap();
        assert_eq!(message, decrypted.as_slice());
    }

    #[test]
    fn test_tampered_payload_fails() {
        let alice = CryptoState::new();
        let bob = CryptoState::new();
        
        let alice_pub = alice.public_key.clone();
        let bob_pub = bob.public_key.clone();
        
        let alice_session = alice.compute_shared_secret(bob_pub);
        let bob_session = bob.compute_shared_secret(alice_pub);

        let mut encrypted = alice_session.encrypt(b"Secret").unwrap();
        // Tamper with the ciphertext
        let len = encrypted.len();
        encrypted[len - 1] ^= 1;
        
        assert!(bob_session.decrypt(&encrypted).is_err());
    }
}
