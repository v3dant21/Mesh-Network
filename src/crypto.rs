use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::rngs::OsRng;
use rand::Rng;
use x25519_dalek::{PublicKey, SharedSecret, StaticSecret};

pub struct CryptoState {
    secret: StaticSecret,
    pub public_key: PublicKey,
}

impl CryptoState {
    pub fn new() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public_key = PublicKey::from(&secret);
        Self { secret, public_key }
    }

    pub fn compute_shared_secret(&self, peer_public: PublicKey) -> SessionCrypto {
        SessionCrypto::new(self.secret.diffie_hellman(&peer_public))
    }
}

pub struct SessionCrypto {
    cipher: Aes256Gcm,
}

impl SessionCrypto {
    pub fn new(shared: SharedSecret) -> Self {
        Self {
            cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(shared.as_bytes())),
        }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let mut combined = nonce_bytes.to_vec();
        combined.append(&mut self.cipher.encrypt(Nonce::from_slice(&nonce_bytes), data).map_err(|e| e.to_string())?);
        Ok(combined)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if data.len() < 12 { return Err("Too short".into()); }
        self.cipher.decrypt(Nonce::from_slice(&data[..12]), &data[12..]).map_err(|e| e.to_string())
    }
}
