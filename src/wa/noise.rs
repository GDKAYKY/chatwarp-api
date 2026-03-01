use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};

use crate::wa::{
    error::NoiseError,
    types::{NOISE_PROTOCOL_NAME, WA_NOISE_PROLOGUE},
};

/// Stateful Noise primitives for the WA session.
#[derive(Debug, Clone)]
pub struct NoiseState {
    h: [u8; 32],
    chaining_key: [u8; 32],
    session_key: [u8; 32],
    send_counter: u32,
    recv_counter: u32,
}

impl NoiseState {
    /// Creates a new Noise state and mixes the provided prologue.
    pub fn new(prologue: &[u8]) -> Self {
        let h = initialize_handshake_hash(NOISE_PROTOCOL_NAME);

        let mut state = Self {
            h,
            chaining_key: h,
            session_key: [0_u8; 32],
            send_counter: 0,
            recv_counter: 0,
        };
        state.mix_hash(prologue);
        state
    }

    /// Creates a new Noise state using the WhatsApp prologue.
    pub fn new_wa() -> Self {
        Self::new(WA_NOISE_PROLOGUE)
    }

    /// Mixes data into handshake hash.
    pub fn mix_hash(&mut self, data: &[u8]) {
        let mut hasher = Sha256::new();
        hasher.update(self.h);
        hasher.update(data);
        self.h.copy_from_slice(&hasher.finalize());
    }

    /// Mixes key material using HKDF-SHA256 and updates chaining/session keys.
    pub fn mix_into_key(&mut self, ikm: &[u8]) {
        let hk = Hkdf::<Sha256>::new(Some(&self.chaining_key), ikm);
        let mut output = [0_u8; 64];

        if hk.expand(&[], &mut output).is_err() {
            return;
        }

        self.chaining_key.copy_from_slice(&output[..32]);
        self.session_key.copy_from_slice(&output[32..]);
    }

    /// Encrypts plaintext using the current sending counter.
    pub fn encrypt_with_ad(&mut self, plaintext: &[u8], ad: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let nonce = build_nonce(self.send_counter);
        self.send_counter = self.send_counter.wrapping_add(1);

        let cipher = Aes256Gcm::new_from_slice(&self.session_key).map_err(|_| NoiseError::Cipher)?;
        cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: ad,
                },
            )
            .map_err(|_| NoiseError::Cipher)
    }

    /// Decrypts ciphertext using the current receiving counter.
    pub fn decrypt_with_ad(&mut self, ciphertext: &[u8], ad: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let nonce = build_nonce(self.recv_counter);
        self.recv_counter = self.recv_counter.wrapping_add(1);

        let cipher = Aes256Gcm::new_from_slice(&self.session_key).map_err(|_| NoiseError::Cipher)?;
        cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: ciphertext,
                    aad: ad,
                },
            )
            .map_err(|_| NoiseError::Cipher)
    }

    /// Returns the current handshake hash for associated data.
    pub fn handshake_hash(&self) -> [u8; 32] {
        self.h
    }
}

fn build_nonce(counter: u32) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[8..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

fn initialize_handshake_hash(protocol_name: &[u8]) -> [u8; 32] {
    let mut hash = [0_u8; 32];
    if protocol_name.len() <= hash.len() {
        hash[..protocol_name.len()].copy_from_slice(protocol_name);
        return hash;
    }

    hash.copy_from_slice(&Sha256::digest(protocol_name));
    hash
}
