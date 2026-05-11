use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use rand::rngs::OsRng;

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
        let shared = self.secret.diffie_hellman(&peer_public);
        SessionCrypto::new(shared)
    }
}

pub struct SessionCrypto {
    cipher: Aes256Gcm,
}

impl SessionCrypto {
    pub fn new(shared: SharedSecret) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(shared.as_bytes());
        let cipher = Aes256Gcm::new(key);
        Self { cipher }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce_bytes = [0u8; 12];
        rand::Rng::fill(&mut rand::thread_rng(), &mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let mut ciphertext = self.cipher.encrypt(nonce, data).map_err(|e| e.to_string())?;
        // Append nonce to ciphertext for decryption
        let mut combined = nonce_bytes.to_vec();
        combined.append(&mut ciphertext);
        Ok(combined)
    }

    pub fn decrypt(&self, encrypted_data: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted_data.len() < 12 {
            return Err("Data too short".to_string());
        }
        let nonce = Nonce::from_slice(&encrypted_data[..12]);
        let ciphertext = &encrypted_data[12..];
        self.cipher.decrypt(nonce, ciphertext).map_err(|e| e.to_string())
    }
}
