// Refatoração: Separar I/O de CPU para permitir paralelismo real

use crate::libsignal::protocol::{SessionRecord, ProtocolAddress};
use futures::future::join_all;
use std::collections::HashMap;

/// Fase 1: Carregar todas as sessões (I/O sequencial)
async fn load_sessions_batch<S: SessionStore>(
    store: &mut S,
    addresses: &[ProtocolAddress],
) -> Result<HashMap<ProtocolAddress, SessionRecord>> {
    let mut sessions = HashMap::new();
    for addr in addresses {
        if let Some(record) = store.load_session(addr).await? {
            sessions.insert(addr.clone(), record);
        }
    }
    Ok(sessions)
}

/// Fase 2: Criptografar em paralelo (CPU-bound, sem I/O)
async fn encrypt_batch_parallel(
    sessions: HashMap<ProtocolAddress, SessionRecord>,
    plaintext: &[u8],
    local_identity: &IdentityKey,
    remote_identities: &HashMap<ProtocolAddress, IdentityKey>,
) -> Result<Vec<(ProtocolAddress, CiphertextMessage, SessionRecord)>> {
    let plaintext = plaintext.to_vec();
    
    let tasks = sessions.into_iter().map(|(addr, mut session)| {
        let plaintext = plaintext.clone();
        let local_identity = local_identity.clone();
        let remote_identity = remote_identities.get(&addr).cloned();
        
        tokio::task::spawn_blocking(move || {
            // Criptografia pura (CPU-bound, sem await)
            let result = encrypt_with_session(
                &mut session,
                &plaintext,
                &local_identity,
                &remote_identity.ok_or_else(|| anyhow!("Missing remote identity"))?,
            )?;
            Ok::<_, anyhow::Error>((addr, result, session))
        })
    });
    
    let results = join_all(tasks).await;
    
    results.into_iter()
        .map(|r| r.map_err(|e| anyhow!("Task failed: {}", e))?)
        .collect()
}

/// Fase 3: Salvar sessões atualizadas (I/O sequencial)
async fn save_sessions_batch<S: SessionStore>(
    store: &mut S,
    sessions: Vec<(ProtocolAddress, SessionRecord)>,
) -> Result<()> {
    for (addr, record) in sessions {
        store.save_session(&addr, &record).await?;
    }
    Ok(())
}

/// Criptografia pura sem I/O (pode rodar em thread separada)
fn encrypt_with_session(
    session: &mut SessionRecord,
    plaintext: &[u8],
    local_identity: &IdentityKey,
    remote_identity: &IdentityKey,
) -> Result<CiphertextMessage> {
    let session_state = session
        .session_state_mut()
        .ok_or_else(|| anyhow!("No session state"))?;
    
    let chain_key = session_state.get_sender_chain_key()?;
    let message_keys = chain_key.message_keys().generate_keys();
    let sender_ephemeral = session_state.sender_ratchet_key()?;
    let previous_counter = session_state.previous_counter();
    let session_version = session_state.session_version()?.try_into()?;
    
    // Criptografia AES (CPU-bound)
    let ctext = aes_256_cbc_encrypt(
        plaintext,
        message_keys.cipher_key(),
        message_keys.iv(),
    )?;
    
    let message = if let Some(items) = session_state.unacknowledged_pre_key_message_items()? {
        let signal_msg = SignalMessage::new(
            session_version,
            message_keys.mac_key(),
            sender_ephemeral,
            chain_key.index(),
            previous_counter,
            &ctext,
            local_identity,
            remote_identity,
        )?;
        
        CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::new(
            session_version,
            session_state.local_registration_id(),
            items.pre_key_id(),
            items.signed_pre_key_id(),
            *items.base_key(),
            local_identity.clone(),
            signal_msg,
        )?)
    } else {
        CiphertextMessage::SignalMessage(SignalMessage::new(
            session_version,
            message_keys.mac_key(),
            sender_ephemeral,
            chain_key.index(),
            previous_counter,
            &ctext,
            local_identity,
            remote_identity,
        )?)
    };
    
    session_state.set_sender_chain_key(&chain_key.next_chain_key());
    
    Ok(message)
}

/// Função principal refatorada
async fn encrypt_for_devices_parallel<'a, S, I, P, SP>(
    stores: &mut SignalStores<'a, S, I, P, SP>,
    devices: &[Jid],
    plaintext: &[u8],
) -> Result<Vec<Node>>
where
    S: SessionStore + Send + Sync,
    I: IdentityKeyStore + Send + Sync,
{
    let addresses: Vec<_> = devices.iter()
        .map(|jid| jid.to_protocol_address())
        .collect();
    
    // FASE 1: I/O - Carregar todas as sessões
    log::info!("🔥 Loading {} sessions...", addresses.len());
    let load_start = Instant::now();
    let sessions = load_sessions_batch(stores.session_store, &addresses).await?;
    log::info!("🔥 Loaded sessions in {:?}", load_start.elapsed());
    
    // Carregar identities
    let local_identity = stores.identity_store.get_identity_key_pair().await?.identity_key();
    let mut remote_identities = HashMap::new();
    for addr in &addresses {
        if let Some(identity) = stores.identity_store.get_identity(addr).await? {
            remote_identities.insert(addr.clone(), identity);
        }
    }
    
    // FASE 2: CPU - Criptografar em paralelo
    log::info!("🔥 Encrypting {} devices in parallel...", sessions.len());
    let encrypt_start = Instant::now();
    let encrypted = encrypt_batch_parallel(
        sessions,
        plaintext,
        &local_identity,
        &remote_identities,
    ).await?;
    log::info!("🔥 Encrypted in {:?}", encrypt_start.elapsed());
    
    // FASE 3: I/O - Salvar sessões atualizadas
    let save_start = Instant::now();
    let sessions_to_save: Vec<_> = encrypted.iter()
        .map(|(addr, _, session)| (addr.clone(), session.clone()))
        .collect();
    save_sessions_batch(stores.session_store, sessions_to_save).await?;
    log::info!("🔥 Saved sessions in {:?}", save_start.elapsed());
    
    // Construir nodes
    let nodes = encrypted.into_iter().map(|(addr, msg, _)| {
        build_participant_node(&addr, msg)
    }).collect();
    
    Ok(nodes)
}
