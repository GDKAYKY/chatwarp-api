use sha2::{Digest, Sha256};

use crate::wa::{
    error::SignalError,
    signal::store::{SessionBundle, SignalSession, SignalStore},
};

/// Initializes a synthetic peer session using bundle material.
pub fn init_session<S>(jid: &str, bundle: &SessionBundle, store: &S) -> Result<(), SignalError>
where
    S: SignalStore,
{
    let local_identity = store.local_identity_key()?;

    let _ = store.load_pre_key(bundle.pre_key_id)?;
    let _ = store.load_signed_pre_key(bundle.signed_pre_key_id)?;

    let mut key_a = local_identity;
    let mut key_b = bundle.peer_identity_key;
    if key_a > key_b {
        std::mem::swap(&mut key_a, &mut key_b);
    }

    let mut hasher = Sha256::new();
    hasher.update(bundle.shared_secret);
    hasher.update(key_a);
    hasher.update(key_b);

    let mut root_key = [0_u8; 32];
    root_key.copy_from_slice(&hasher.finalize());

    store.store_session(
        jid,
        SignalSession {
            root_key,
            send_counter: 0,
            recv_counter: 0,
        },
    )
}

/// Encrypts a payload for a peer session.
pub fn encrypt<S>(jid: &str, plaintext: &[u8], store: &S) -> Result<Vec<u8>, SignalError>
where
    S: SignalStore,
{
    let mut session = store
        .load_session(jid)?
        .ok_or(SignalError::MissingSession)?;

    let counter = session.send_counter;
    let mask = derive_mask(session.root_key, counter, plaintext.len());
    let ciphertext = xor_bytes(plaintext, &mask);

    session.send_counter = session.send_counter.wrapping_add(1);
    store.store_session(jid, session)?;

    let mut output = Vec::with_capacity(4 + ciphertext.len());
    output.extend_from_slice(&counter.to_be_bytes());
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypts a payload for a peer session.
pub fn decrypt<S>(jid: &str, payload: &[u8], store: &S) -> Result<Vec<u8>, SignalError>
where
    S: SignalStore,
{
    if payload.len() < 4 {
        return Err(SignalError::InvalidCiphertext);
    }

    let mut session = store
        .load_session(jid)?
        .ok_or(SignalError::MissingSession)?;

    let mut counter_bytes = [0_u8; 4];
    counter_bytes.copy_from_slice(&payload[..4]);
    let counter = u32::from_be_bytes(counter_bytes);

    let ciphertext = &payload[4..];
    let mask = derive_mask(session.root_key, counter, ciphertext.len());
    let plaintext = xor_bytes(ciphertext, &mask);

    if counter >= session.recv_counter {
        session.recv_counter = counter.saturating_add(1);
    }
    store.store_session(jid, session)?;

    Ok(plaintext)
}

fn derive_mask(root_key: [u8; 32], counter: u32, len: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(len);
    let mut block_index: u32 = 0;

    while output.len() < len {
        let mut hasher = Sha256::new();
        hasher.update(root_key);
        hasher.update(counter.to_be_bytes());
        hasher.update(block_index.to_be_bytes());

        let block = hasher.finalize();
        let remaining = len - output.len();
        let take = remaining.min(block.len());
        output.extend_from_slice(&block[..take]);
        block_index = block_index.wrapping_add(1);
    }

    output
}

fn xor_bytes(input: &[u8], mask: &[u8]) -> Vec<u8> {
    input
        .iter()
        .zip(mask.iter())
        .map(|(left, right)| left ^ right)
        .collect()
}
