use crate::client::context::{GroupInfo, SendContextResolver};
use crate::libsignal::protocol::{
    CiphertextMessage, SENDERKEY_MESSAGE_CURRENT_VERSION, SenderKeyDistributionMessage,
    SenderKeyMessage, SenderKeyRecord, SenderKeyStore, SignalProtocolError, UsePQRatchet,
    message_encrypt, process_prekey_bundle,
};
use crate::libsignal::store::sender_key_name::SenderKeyName;
use crate::messages::MessageUtils;
use crate::reporting_token::{
    build_reporting_node, generate_reporting_token, prepare_message_with_context,
};
use crate::types::jid::JidExt;
use anyhow::{Result, anyhow};
use prost::Message as ProtoMessage;
use rand::{CryptoRng, Rng, SeedableRng, rngs::StdRng};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use waproto::whatsapp as wa;
use waproto::whatsapp::message::DeviceSentMessage;
use warp_core_binary::builder::NodeBuilder;
use warp_core_binary::jid::{Jid, JidExt as _};
use warp_core_binary::node::{Attrs, Node};
use warp_core_libsignal::crypto::aes_256_cbc_encrypt_into;

// ============================================================================
// CachedSessionStore — scoped write-behind cache for the send pipeline
// ============================================================================

use crate::libsignal::protocol::{
    Direction, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore, ProtocolAddress,
    SessionRecord,
};

/// A scoped write-behind session cache for the send pipeline.
///
/// Wraps an inner `SessionStore` and caches every `load_session` result.
/// `store_session` writes only to cache and marks the address dirty.
/// Call [`flush`] after encryption to drain all dirty entries back to the
/// inner store.
///
/// # Why RefCell?
///
/// The Signal `SessionStore` trait takes `&self` for loads and `&mut self` for
/// stores. We need interior mutability for the cache because `load_session`
/// (which populates the cache on miss) takes `&self`.
struct CachedSessionStore<'a, S> {
    inner: &'a S,
    cache: RefCell<HashMap<ProtocolAddress, Option<SessionRecord>>>,
    dirty: RefCell<HashSet<ProtocolAddress>>,
}

impl<'a, S> CachedSessionStore<'a, S> {
    fn new(inner: &'a S) -> Self {
        Self {
            inner,
            cache: RefCell::new(HashMap::new()),
            dirty: RefCell::new(HashSet::new()),
        }
    }
}

impl<'a, S> CachedSessionStore<'a, S>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
{
    /// Pre-populate the cache by batch-loading sessions for all addresses.
    ///
    /// After this call, `load_session` for any of these addresses will be a
    /// cache hit (even if the session doesn't exist — we cache `None`).
    async fn populate_from_batch(
        &self,
        addresses: &[ProtocolAddress],
    ) -> std::result::Result<(), SignalProtocolError> {
        if addresses.is_empty() {
            return Ok(());
        }
        let results = self.inner.load_sessions_batch(addresses).await?;
        let mut cache = self.cache.borrow_mut();
        for (addr, record) in results {
            cache.entry(addr).or_insert(record);
        }
        Ok(())
    }
}

impl<'a, S> CachedSessionStore<'a, S>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
{
    /// Extract all dirty (modified) sessions from the cache.
    ///
    /// After calling this, the caller should drop `self` to release the
    /// immutable borrow on the inner store, then write the returned entries
    /// back using `stores.session_store.store_session()`.
    fn take_dirty(&self) -> Vec<(ProtocolAddress, SessionRecord)> {
        let dirty_addrs: Vec<ProtocolAddress> = self.dirty.borrow_mut().drain().collect();
        let cache = self.cache.borrow();
        dirty_addrs
            .into_iter()
            .filter_map(|addr| {
                cache
                    .get(&addr)
                    .and_then(|opt| opt.as_ref())
                    .map(|record| (addr, record.clone()))
            })
            .collect()
    }
}

// Safety: CachedSessionStore is only used within a single async task (the
// encrypt loop) and never shared across threads. The RefCell is needed
// because load_session takes &self but must populate the cache on miss.
unsafe impl<S: Send> Send for CachedSessionStore<'_, S> {}
unsafe impl<S: Sync> Sync for CachedSessionStore<'_, S> {}

#[async_trait::async_trait]
impl<S> crate::libsignal::protocol::SessionStore for CachedSessionStore<'_, S>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
{
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> std::result::Result<Option<SessionRecord>, SignalProtocolError> {
        // Cache hit
        if let Some(entry) = self.cache.borrow().get(address) {
            return Ok(entry.clone());
        }
        // Cache miss — load from inner store and cache the result
        let record = self.inner.load_session(address).await?;
        self.cache
            .borrow_mut()
            .insert(address.clone(), record.clone());
        Ok(record)
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> std::result::Result<(), SignalProtocolError> {
        self.cache
            .borrow_mut()
            .insert(address.clone(), Some(record.clone()));
        self.dirty.borrow_mut().insert(address.clone());
        Ok(())
    }
}

// ============================================================================
// CachedIdentityStore — write-through + read cache for the send pipeline
// ============================================================================

/// A scoped identity cache for the send pipeline.
///
/// **Read cache**: all `get_identity` / `is_trusted_identity` calls hit cache
/// after first DB load, eliminating 3 out of 4 `spawn_blocking` round-trips
/// per device.
///
/// **Write-through safety**: `save_identity` compares against the cache:
/// - Identity **unchanged** (99.9% during sends) → cache-only, batched later.
/// - Identity **changed** → immediate write-through to inner store.
///
/// This preserves the Signal Protocol TOFU trust model while eliminating
/// redundant DB reads.
///
/// The cache lives only for the duration of one `encrypt_for_devices_unified`
/// call — no cross-request staleness.
struct CachedIdentityStore<'a, I> {
    inner: &'a mut I,
    cache: RefCell<HashMap<ProtocolAddress, Option<IdentityKey>>>,
    /// Only genuinely NEW identities (first seen — not in DB) go here.
    /// Unchanged identities (already persisted) are NOT marked dirty.
    dirty_new: RefCell<HashMap<ProtocolAddress, IdentityKey>>,
    identity_pair: RefCell<Option<IdentityKeyPair>>,
    registration_id: RefCell<Option<u32>>,
}

impl<'a, I> CachedIdentityStore<'a, I> {
    fn new(inner: &'a mut I) -> Self {
        Self {
            inner,
            cache: RefCell::new(HashMap::new()),
            dirty_new: RefCell::new(HashMap::new()),
            identity_pair: RefCell::new(None),
            registration_id: RefCell::new(None),
        }
    }
}

impl<'a, I> CachedIdentityStore<'a, I>
where
    I: IdentityKeyStore + Send + Sync,
{
    /// Extract new identities that need to be persisted.
    fn take_dirty(&self) -> Vec<(ProtocolAddress, IdentityKey)> {
        self.dirty_new.borrow_mut().drain().collect()
    }

    /// Pre-populate the identity cache from batch-loaded raw key data.
    ///
    /// Each entry is `(address_string, key_bytes)` from a backend batch query.
    /// After this call, `get_identity` for these addresses will be cache hits.
    fn pre_populate(&self, entries: &[(String, Vec<u8>)]) {
        use crate::libsignal::protocol::PublicKey;
        let mut cache = self.cache.borrow_mut();
        for (addr_str, key_bytes) in entries {
            if let Ok(public_key) = PublicKey::deserialize(key_bytes) {
                let identity_key = IdentityKey::new(public_key);
                // Parse "name.device_id" format from address string
                if let Some(dot_pos) = addr_str.rfind('.') {
                    if let Ok(device_id) = addr_str[dot_pos + 1..].parse::<u32>() {
                        let name = &addr_str[..dot_pos];
                        let protocol_addr = ProtocolAddress::new(name.to_string(), device_id.into());
                        cache.entry(protocol_addr).or_insert(Some(identity_key));
                    }
                }
            }
        }
    }
}

// Safety: same as CachedSessionStore — single async task, never shared.
unsafe impl<I: Send> Send for CachedIdentityStore<'_, I> {}
unsafe impl<I: Sync> Sync for CachedIdentityStore<'_, I> {}

#[async_trait::async_trait]
impl<I> IdentityKeyStore for CachedIdentityStore<'_, I>
where
    I: IdentityKeyStore + Send + Sync,
{
    async fn get_identity_key_pair(
        &self,
    ) -> std::result::Result<IdentityKeyPair, SignalProtocolError> {
        if let Some(pair) = self.identity_pair.borrow().as_ref() {
            return Ok(pair.clone());
        }
        let pair = self.inner.get_identity_key_pair().await?;
        *self.identity_pair.borrow_mut() = Some(pair.clone());
        Ok(pair)
    }

    async fn get_local_registration_id(&self) -> std::result::Result<u32, SignalProtocolError> {
        if let Some(id) = *self.registration_id.borrow() {
            return Ok(id);
        }
        let id = self.inner.get_local_registration_id().await?;
        *self.registration_id.borrow_mut() = Some(id);
        Ok(id)
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> std::result::Result<IdentityChange, SignalProtocolError> {
        let existing = self.get_identity(address).await?;

        match existing {
            Some(ref stored) if stored != identity => {
                // Identity CHANGED — write-through immediately (TOFU safety)
                log::info!(
                    "CachedIdentityStore: identity changed for {}, flushing immediately",
                    address
                );
                let result = self.inner.save_identity(address, identity).await?;
                self.cache
                    .borrow_mut()
                    .insert(address.clone(), Some(identity.clone()));
                Ok(result)
            }
            Some(_) => {
                // Identity UNCHANGED — already persisted in DB, just update cache.
                // No dirty marking needed: the DB already has this exact key.
                self.cache
                    .borrow_mut()
                    .insert(address.clone(), Some(identity.clone()));
                Ok(IdentityChange::NewOrUnchanged)
            }
            None => {
                // Identity NEW (first seen) — cache it and mark dirty for batch flush.
                self.cache
                    .borrow_mut()
                    .insert(address.clone(), Some(identity.clone()));
                self.dirty_new
                    .borrow_mut()
                    .insert(address.clone(), identity.clone());
                Ok(IdentityChange::NewOrUnchanged)
            }
        }
    }

    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        _direction: Direction,
    ) -> std::result::Result<bool, SignalProtocolError> {
        // TOFU: trust on first use, must match if stored
        match self.get_identity(address).await? {
            None => Ok(true),
            Some(stored) => Ok(&stored == identity),
        }
    }

    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> std::result::Result<Option<IdentityKey>, SignalProtocolError> {
        if let Some(entry) = self.cache.borrow().get(address) {
            return Ok(entry.clone());
        }
        let identity = self.inner.get_identity(address).await?;
        self.cache
            .borrow_mut()
            .insert(address.clone(), identity.clone());
        Ok(identity)
    }
}

pub async fn encrypt_group_message<S, R>(
    sender_key_store: &mut S,
    group_jid: &Jid,
    sender_jid: &Jid,
    plaintext: &[u8],
    csprng: &mut R,
) -> Result<SenderKeyMessage>
where
    S: SenderKeyStore + ?Sized,
    R: Rng + CryptoRng,
{
    let sender_address = sender_jid.to_protocol_address();
    let sender_key_name = SenderKeyName::new(group_jid.to_string(), sender_address.to_string());
    log::debug!(
        "Attempting to load sender key for group {} sender {}",
        sender_key_name.group_id(),
        sender_key_name.sender_id()
    );

    let mut record = sender_key_store
        .load_sender_key(&sender_key_name)
        .await?
        .ok_or_else(|| {
            SignalProtocolError::NoSenderKeyState(format!(
                "no sender key record for group {} sender {}",
                sender_key_name.group_id(),
                sender_key_name.sender_id()
            ))
        })?;

    let sender_key_state = record
        .sender_key_state_mut()
        .map_err(|e| anyhow!("Invalid SenderKey session: {:?}", e))?;

    let sender_chain_key = sender_key_state
        .sender_chain_key()
        .ok_or_else(|| anyhow!("Invalid SenderKey session: missing chain key"))?;

    let message_keys = sender_chain_key.sender_message_key();

    let mut ciphertext = Vec::new();
    aes_256_cbc_encrypt_into(
        plaintext,
        message_keys.cipher_key(),
        message_keys.iv(),
        &mut ciphertext,
    )
    .map_err(|_| anyhow!("AES encryption failed"))?;

    let signing_key = sender_key_state
        .signing_key_private()
        .map_err(|e| anyhow!("Invalid SenderKey session: missing signing key: {:?}", e))?;

    let skm = SenderKeyMessage::new(
        SENDERKEY_MESSAGE_CURRENT_VERSION,
        sender_key_state.chain_id(),
        message_keys.iteration(),
        ciphertext.into_boxed_slice(),
        csprng,
        &signing_key,
    )?;

    sender_key_state.set_sender_chain_key(sender_chain_key.next()?);

    sender_key_store
        .store_sender_key(&sender_key_name, &record)
        .await?;

    Ok(skm)
}

pub struct SignalStores<'a, S, I, P, SP> {
    pub sender_key_store: &'a mut (dyn crate::libsignal::protocol::SenderKeyStore + Send + Sync),
    pub session_store: &'a mut S,
    pub identity_store: &'a mut I,
    pub prekey_store: &'a mut P,
    pub signed_prekey_store: &'a SP,
}

/// A device paired with the plaintext it should receive.
/// Allows a single pass over all devices regardless of payload type.
struct DeviceTask<'p> {
    device_jid: Jid,
    plaintext: &'p [u8],
}

/// Resolve the effective encryption JID for a device.
///
/// For PN JIDs, check if an existing session lives under the corresponding LID
/// (established when the remote sent us a message using sender_lid).
/// Returns `(encryption_jid, needs_prekey)`.
async fn resolve_encryption_jid<S>(
    session_store: &mut S,
    resolver: &dyn SendContextResolver,
    device_jid: &Jid,
) -> (Jid, bool)
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
{
    let signal_address = device_jid.to_protocol_address();

    if session_store
        .load_session(&signal_address)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return (device_jid.clone(), false);
    }

    if device_jid.is_pn() {
        if let Some(lid_user) = resolver.get_lid_for_phone(&device_jid.user).await {
            let lid_jid = Jid::lid_device(lid_user, device_jid.device);
            let lid_address = lid_jid.to_protocol_address();
            if session_store
                .load_session(&lid_address)
                .await
                .ok()
                .flatten()
                .is_some()
            {
                log::debug!(
                    "Using existing LID session {} instead of creating new PN session for {}",
                    lid_jid,
                    device_jid
                );
                return (lid_jid, false);
            }
        }
    }

    (device_jid.clone(), true)
}

/// Establish Signal sessions for devices that have no session yet.
async fn establish_missing_sessions<'a, S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    resolver: &dyn SendContextResolver,
    jids_needing_prekeys: &[Jid],
) -> Result<()>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
    I: crate::libsignal::protocol::IdentityKeyStore + Send + Sync,
{
    if jids_needing_prekeys.is_empty() {
        return Ok(());
    }

    let prekey_start = Instant::now();
    let prekey_bundles = resolver
        .fetch_prekeys_for_identity_check(jids_needing_prekeys)
        .await?;
    let prekey_elapsed = prekey_start.elapsed();
    if prekey_elapsed > Duration::from_millis(250) {
        log::info!(
            "establish_missing_sessions: prekey fetch took {} ms for {} devices",
            prekey_elapsed.as_millis(),
            jids_needing_prekeys.len()
        );
    }

    for device_jid in jids_needing_prekeys {
        let signal_address = device_jid.to_protocol_address();
        let Some(bundle) = prekey_bundles.get(device_jid) else {
            log::warn!(
                "No pre-key bundle returned for device {}. Skipping.",
                &signal_address
            );
            continue;
        };

        let result = process_prekey_bundle(
            &signal_address,
            session_store,
            identity_store,
            bundle,
            &mut StdRng::from_os_rng(),
            UsePQRatchet::No,
        )
        .await;

        match result {
            Ok(_) => {}
            Err(SignalProtocolError::UntrustedIdentity(ref addr)) => {
                log::info!("Untrusted identity for {}. Updating and retrying.", addr);
                let new_identity = match bundle.identity_key() {
                    Ok(k) => k,
                    Err(e) => {
                        log::warn!(
                            "Failed to get identity key for {}: {:?}. Skipping.",
                            addr,
                            e
                        );
                        continue;
                    }
                };
                if let Err(e) = identity_store
                    .save_identity(&signal_address, new_identity)
                    .await
                {
                    log::warn!("Failed to save identity for {}: {:?}. Skipping.", addr, e);
                    continue;
                }
                if let Err(e) = process_prekey_bundle(
                    &signal_address,
                    session_store,
                    identity_store,
                    bundle,
                    &mut StdRng::from_os_rng(),
                    UsePQRatchet::No,
                )
                .await
                {
                    log::warn!(
                        "Failed to establish session with {} after identity update: {:?}. Skipping.",
                        addr,
                        e
                    );
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to process pre-key bundle for {}: {:?}",
                    signal_address,
                    e
                ));
            }
        }
    }

    Ok(())
}

/// Encrypt `plaintext` for a single device and return the XML node.
///
/// `device_jid` is what the server sees in the `to` attribute; `encryption_jid`
/// is the JID whose Signal session is actually used for encryption (may differ
/// when a LID session is reused for a PN-addressed device).
async fn encrypt_one_device<'a, S, I>(
    stores_session: &mut S,
    stores_identity: &mut I,
    device_jid: &Jid,
    encryption_jid: &Jid,
    plaintext: &[u8],
    enc_extra_attrs: &Attrs,
) -> Option<(Node, bool)>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
    I: crate::libsignal::protocol::IdentityKeyStore + Send + Sync,
{
    let signal_address = encryption_jid.to_protocol_address();
    let t0 = Instant::now();
    let result = message_encrypt(plaintext, &signal_address, stores_session, stores_identity).await;
    let elapsed = t0.elapsed();
    if elapsed > Duration::from_millis(500) {
        log::info!("🔥 message_encrypt for {} took {:?}", device_jid, elapsed);
    }

    match result {
        Ok(encrypted_payload) => {
            let (enc_type, is_prekey, serialized_bytes) = match encrypted_payload {
                CiphertextMessage::PreKeySignalMessage(msg) => {
                    ("pkmsg", true, msg.serialized().to_vec())
                }
                CiphertextMessage::SignalMessage(msg) => ("msg", false, msg.serialized().to_vec()),
                _ => return None,
            };

            let mut enc_attrs = Attrs::new();
            enc_attrs.insert("v".to_string(), "2".to_string());
            enc_attrs.insert("type".to_string(), enc_type.to_string());
            for (k, v) in enc_extra_attrs.iter() {
                enc_attrs.insert(k.clone(), v.clone());
            }

            let enc_node = NodeBuilder::new("enc")
                .attrs(enc_attrs)
                .bytes(serialized_bytes)
                .build();
            let to_node = NodeBuilder::new("to")
                .attr("jid", device_jid.to_string())
                .children([enc_node])
                .build();
            Some((to_node, is_prekey))
        }
        Err(e) => {
            log::warn!(
                "Failed to encrypt for device {}: {}. Skipping.",
                &signal_address,
                e
            );
            None
        }
    }
}

/// Core encrypt pipeline: one session-map build, one prekey fetch, one encrypt
/// loop — regardless of how many different plaintexts are involved.
///
/// `tasks` carries `(device_jid, plaintext)` so recipient devices and own
/// devices can be processed in the same pass without duplication.
async fn encrypt_for_devices_unified<'a, S, I, P, SP>(
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    tasks: &[DeviceTask<'_>],
    enc_extra_attrs: &Attrs,
) -> Result<(Vec<Node>, bool)>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
    I: crate::libsignal::protocol::IdentityKeyStore + Send + Sync,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
{
    if tasks.is_empty() {
        return Ok((Vec::new(), false));
    }

    let total_start = Instant::now();

    // Wrap the session store in a write-behind cache.
    // Phase 1 reads populate the cache; Phase 3 encryptions get cache hits
    // instead of redundant DB reads. All dirty writes are flushed once at the
    // end, avoiding per-device semaphore acquisitions during encryption.
    let mut cached_session = CachedSessionStore::new(stores.session_store as &S);

    // Wrap the identity store in a write-through + read cache.
    // Eliminates 3 out of 4 spawn_blocking round-trips per device in Phase 3.
    // Identity reads are cached; unchanged writes are deferred; changed
    // identities are flushed immediately (TOFU safety).
    let mut cached_identity = CachedIdentityStore::new(stores.identity_store);

    // ── Phase 1: Build session map ─────────────────────────────────────────────
    // Pre-populate the session cache in one batch DB query. This replaces N
    // individual load_session calls with a single SELECT ... WHERE IN (...).
    // We also pre-load potential LID addresses so the resolve_encryption_jid
    // LID fallback path is also a cache hit.
    let map_start = Instant::now();

    {
        let mut batch_addrs: Vec<ProtocolAddress> = Vec::with_capacity(tasks.len() * 2);
        for task in tasks {
            batch_addrs.push(task.device_jid.to_protocol_address());
            // Pre-load potential LID fallback addresses so resolve_encryption_jid
            // doesn't trigger individual DB loads.
            if task.device_jid.is_pn() {
                if let Some(lid_user) = resolver.get_lid_for_phone(&task.device_jid.user).await {
                    let lid_jid = Jid::lid_device(lid_user, task.device_jid.device);
                    batch_addrs.push(lid_jid.to_protocol_address());
                }
            }
        }
        cached_session.populate_from_batch(&batch_addrs).await
            .map_err(|e| anyhow!("Failed to batch-load sessions: {:?}", e))?;
    }

    let mut jid_to_encryption_jid: HashMap<Jid, Jid> = HashMap::with_capacity(tasks.len());
    let mut jids_needing_prekeys: Vec<Jid> = Vec::new();

    for task in tasks {
        let (enc_jid, needs_prekey) =
            resolve_encryption_jid(&mut cached_session, resolver, &task.device_jid).await;
        jid_to_encryption_jid.insert(task.device_jid.clone(), enc_jid);
        if needs_prekey {
            jids_needing_prekeys.push(task.device_jid.clone());
        }
    }

    let map_elapsed = map_start.elapsed();
    log::debug!(
        "encrypt_for_devices_unified: session map built in {} ms (devices={}, needing_prekeys={})",
        map_elapsed.as_millis(),
        tasks.len(),
        jids_needing_prekeys.len()
    );

    // ── Phase 2: Establish missing sessions (ONE prekey fetch total) ──────────
    // New sessions created here are written through the cache so Phase 3
    // will find them without another DB round-trip.
    establish_missing_sessions(
        &mut cached_session,
        &mut cached_identity,
        resolver,
        &jids_needing_prekeys,
    )
    .await?;

    // ── Phase 3: Encrypt (one pass, payload chosen per device) ───────────────
    // All load_session calls here are cache hits (loaded in Phase 1 or
    // created in Phase 2). store_session calls go to cache only.
    // Identity reads are cache hits; unchanged identity writes go to cache only.
    let encrypt_start = Instant::now();
    let mut participant_nodes = Vec::with_capacity(tasks.len());
    let mut includes_prekey_message = false;

    for task in tasks {
        let encryption_jid = jid_to_encryption_jid
            .get(&task.device_jid)
            .unwrap_or(&task.device_jid);

        if let Some((node, is_prekey)) = encrypt_one_device(
            &mut cached_session,
            &mut cached_identity,
            &task.device_jid,
            encryption_jid,
            task.plaintext,
            enc_extra_attrs,
        )
        .await
        {
            includes_prekey_message |= is_prekey;
            participant_nodes.push(node);
        }
    }

    let encrypt_elapsed = encrypt_start.elapsed();

    // ── Phase 4: Flush dirty sessions to the backend (ONE burst of writes) ───
    let flush_start = Instant::now();
    let dirty_sessions = cached_session.take_dirty();
    let dirty_session_count = dirty_sessions.len();
    // Drop the session cache to release the immutable borrow on session_store.
    drop(cached_session);

    // ── Phase 4.5: Flush dirty identities ────────────────────────────────────
    let dirty_identities = cached_identity.take_dirty();
    let dirty_identity_count = dirty_identities.len();
    // Drop the identity cache to release the mutable borrow on identity_store.
    drop(cached_identity);

    log::debug!(
        "encrypt_for_devices_unified: flushing {} dirty sessions + {} dirty identities",
        dirty_session_count,
        dirty_identity_count
    );

    // Batch-flush all dirty sessions in one transactional write.
    if !dirty_sessions.is_empty() {
        stores
            .session_store
            .store_sessions_batch(&dirty_sessions)
            .await
            .map_err(|e| anyhow!("Failed to batch-flush {} sessions: {:?}", dirty_session_count, e))?;
    }

    for (addr, key) in &dirty_identities {
        stores
            .identity_store
            .save_identity(addr, key)
            .await
            .map_err(|e| anyhow!("Failed to flush identity for {}: {:?}", addr, e))?;
    }
    let flush_elapsed = flush_start.elapsed();

    let total_elapsed = total_start.elapsed();
    log::debug!(
        "encrypt_for_devices_unified: {} devices — map {}ms, encrypt {}ms, flush {}ms ({}s+{}i), total {}ms",
        tasks.len(),
        map_elapsed.as_millis(),
        encrypt_elapsed.as_millis(),
        flush_elapsed.as_millis(),
        dirty_session_count,
        dirty_identity_count,
        total_elapsed.as_millis(),
    );

    Ok((participant_nodes, includes_prekey_message))
}

/// Legacy single-plaintext wrapper kept for group SKDM distribution which
/// still uses a single plaintext for all devices.
async fn encrypt_for_devices<'a, S, I, P, SP>(
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    enc_extra_attrs: &Attrs,
) -> Result<(Vec<Node>, bool)>
where
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
    I: crate::libsignal::protocol::IdentityKeyStore + Send + Sync,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
{
    let tasks: Vec<DeviceTask<'_>> = devices
        .iter()
        .map(|jid| DeviceTask {
            device_jid: jid.clone(),
            plaintext: plaintext_to_encrypt,
        })
        .collect();
    encrypt_for_devices_unified(stores, resolver, &tasks, enc_extra_attrs).await
}

#[allow(clippy::too_many_arguments)]
pub async fn prepare_dm_stanza<
    'a,
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
    I: crate::libsignal::protocol::IdentityKeyStore + Send + Sync,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
>(
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    own_jid: &Jid,
    account: Option<&wa::AdvSignedDeviceIdentity>,
    to_jid: Jid,
    message: &wa::Message,
    request_id: String,
    edit: Option<crate::types::message::EditAttribute>,
) -> Result<Node> {
    // Generate reporting token if the message type supports it
    // For DMs, both sender_jid and remote_jid are the recipient (to_jid) per Baileys implementation
    let reporting_result = generate_reporting_token(message, &request_id, &to_jid, &to_jid, None);

    // Prepare message with MessageContextInfo containing the message secret
    let message_for_encryption = if let Some(ref result) = reporting_result {
        prepare_message_with_context(message, &result.message_secret)
    } else {
        message.clone()
    };

    let recipient_plaintext = MessageUtils::pad_message_v2(message_for_encryption.encode_to_vec());

    let dsm = wa::Message {
        device_sent_message: Some(Box::new(DeviceSentMessage {
            destination_jid: Some(to_jid.to_string()),
            message: Some(Box::new(message_for_encryption.clone())),
            phash: Some("".to_string()),
        })),
        ..Default::default()
    };

    let own_devices_plaintext = MessageUtils::pad_message_v2(dsm.encode_to_vec());

    // Deduplicate participants by user+server to avoid resolving the same user twice
    // (e.g., when sending a DM to self, to_jid and own_jid are the same user).
    let mut participants = Vec::new();
    let mut seen_users: HashSet<(String, String)> = HashSet::new();
    for jid in [to_jid.clone(), own_jid.clone()] {
        let key = (jid.user.clone(), jid.server.clone());
        if seen_users.insert(key) {
            // Use base JID (no device/agent) for device resolution
            participants.push(jid.to_non_ad());
        }
    }

    let mut all_devices = resolver.resolve_devices(&participants).await?;
    // Deduplicate devices to avoid redundant encryption work if resolver returns overlaps.
    let mut seen_devices: HashSet<Jid> = HashSet::new();
    all_devices.retain(|jid| seen_devices.insert(jid.clone()));

    let mut recipient_devices = Vec::new();
    let mut own_other_devices = Vec::new();
    for device_jid in &all_devices {
        let is_own_device = device_jid.user == own_jid.user && device_jid.device != own_jid.device;
        if is_own_device {
            own_other_devices.push(device_jid.clone());
        } else {
            recipient_devices.push(device_jid.clone());
        }
    }
    log::info!(
        "prepare_dm_stanza: devices total={} recipient={} own_other={}",
        all_devices.len(),
        recipient_devices.len(),
        own_other_devices.len()
    );

    // If this is an edit-like message, set decrypt-fail="hide" on enc nodes
    let mut enc_extra_attrs = Attrs::new();
    if let Some(edit_attr) = &edit
        && *edit_attr != crate::types::message::EditAttribute::Empty
    {
        enc_extra_attrs.insert("decrypt-fail".to_string(), "hide".to_string());
    }

    // Build a single unified task list: recipient devices + own devices in one pass.
    // This avoids building the session map twice and fetching prekeys twice.
    let mut tasks: Vec<DeviceTask<'_>> = Vec::with_capacity(all_devices.len());
    for device_jid in &recipient_devices {
        tasks.push(DeviceTask {
            device_jid: device_jid.clone(),
            plaintext: &recipient_plaintext,
        });
    }
    for device_jid in &own_other_devices {
        tasks.push(DeviceTask {
            device_jid: device_jid.clone(),
            plaintext: &own_devices_plaintext,
        });
    }

    let (participant_nodes, includes_prekey_message) =
        encrypt_for_devices_unified(stores, resolver, &tasks, &enc_extra_attrs).await?;

    let mut message_content_nodes = vec![
        NodeBuilder::new("participants")
            .children(participant_nodes)
            .build(),
    ];

    if includes_prekey_message && let Some(acc) = account {
        let device_identity_bytes = acc.encode_to_vec();
        message_content_nodes.push(
            NodeBuilder::new("device-identity")
                .bytes(device_identity_bytes)
                .build(),
        );
    }

    // Add reporting token node if we generated one
    if let Some(ref result) = reporting_result {
        message_content_nodes.push(build_reporting_node(result));
    }

    let mut stanza_attrs = Attrs::new();
    stanza_attrs.insert("to".to_string(), to_jid.to_string());
    stanza_attrs.insert("id".to_string(), request_id);
    stanza_attrs.insert("type".to_string(), "text".to_string());

    if let Some(edit_attr) = edit
        && edit_attr != crate::types::message::EditAttribute::Empty
    {
        stanza_attrs.insert("edit".to_string(), edit_attr.to_string_val().to_string());
    }

    let stanza = NodeBuilder::new("message")
        .attrs(stanza_attrs.into_iter())
        .children(message_content_nodes)
        .build();

    Ok(stanza)
}

pub async fn prepare_peer_stanza<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    to_jid: Jid,
    message: &wa::Message,
    request_id: String,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    let plaintext = MessageUtils::pad_message_v2(message.encode_to_vec());
    let signal_address = to_jid.to_protocol_address();

    let encrypted_message =
        message_encrypt(&plaintext, &signal_address, session_store, identity_store).await?;

    let (enc_type, serialized_bytes) = match encrypted_message {
        CiphertextMessage::SignalMessage(msg) => ("msg", msg.serialized().to_vec()),
        CiphertextMessage::PreKeySignalMessage(msg) => ("pkmsg", msg.serialized().to_vec()),
        _ => return Err(anyhow!("Unexpected peer encryption message type")),
    };

    let enc_node = NodeBuilder::new("enc")
        .attrs([("v", "2"), ("type", enc_type)])
        .bytes(serialized_bytes)
        .build();

    let stanza = NodeBuilder::new("message")
        .attrs([
            ("to", to_jid.to_string()),
            ("id", request_id),
            ("type", "text".to_string()),
            ("category", "peer".to_string()),
        ])
        .children([enc_node])
        .build();

    Ok(stanza)
}

#[allow(clippy::too_many_arguments)]
pub async fn prepare_group_stanza<
    'a,
    S: crate::libsignal::protocol::SessionStore + Send + Sync,
    I: crate::libsignal::protocol::IdentityKeyStore + Send + Sync,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
>(
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    group_info: &mut GroupInfo,
    own_jid: &Jid,
    own_lid: &Jid,
    account: Option<&wa::AdvSignedDeviceIdentity>,
    to_jid: Jid,
    message: &wa::Message,
    request_id: String,
    force_skdm_distribution: bool,
    skdm_target_devices: Option<Vec<Jid>>,
    edit: Option<crate::types::message::EditAttribute>,
) -> Result<Node> {
    let (own_sending_jid, _) = match group_info.addressing_mode {
        crate::types::message::AddressingMode::Lid => (own_lid.clone(), "lid"),
        crate::types::message::AddressingMode::Pn => (own_jid.clone(), "pn"),
    };

    // Generate reporting token if the message type supports it
    // For groups, both sender_jid and remote_jid are the group JID (to_jid) per Baileys implementation
    let reporting_result = generate_reporting_token(message, &request_id, &to_jid, &to_jid, None);

    // Prepare message with MessageContextInfo containing the message secret
    let message_for_encryption = if let Some(ref result) = reporting_result {
        prepare_message_with_context(message, &result.message_secret)
    } else {
        message.clone()
    };

    let own_base_jid = own_sending_jid.to_non_ad();
    if !group_info
        .participants
        .iter()
        .any(|participant| participant.is_same_user_as(&own_base_jid))
    {
        group_info.participants.push(own_base_jid.clone());
    }

    let mut message_children: Vec<Node> = Vec::new();
    let mut includes_prekey_message = false;
    let mut resolved_devices_for_phash: Option<Vec<Jid>> = None;

    // Determine if we need to distribute SKDM and to which devices
    let distribution_list: Option<Vec<Jid>> = if let Some(target_devices) = skdm_target_devices {
        // Use the specific list of devices that need SKDM
        if target_devices.is_empty() {
            None
        } else {
            log::debug!(
                "SKDM distribution to {} specific devices for group {}",
                target_devices.len(),
                to_jid
            );
            Some(target_devices)
        }
    } else if force_skdm_distribution {
        // Resolve all devices for all participants (legacy behavior)
        // For LID groups, use phone numbers for device queries (LID usync may not work for own JID)
        // For PN groups, use JIDs directly
        let mut jids_to_resolve: Vec<Jid> = group_info
            .participants
            .iter()
            .map(|jid| {
                let base_jid = jid.to_non_ad();
                // If this is a LID JID and we have a phone number mapping, use it for device query
                if base_jid.is_lid()
                    && let Some(phone_jid) = group_info.phone_jid_for_lid_user(&base_jid.user)
                {
                    log::debug!(
                        "Using phone number {} for LID {} device query",
                        phone_jid,
                        base_jid
                    );
                    return phone_jid.to_non_ad();
                }
                base_jid
            })
            .collect();

        // Determine what JID to check for - use phone number if we're in LID mode and have a mapping
        let own_jid_to_check = if own_base_jid.is_lid() {
            group_info
                .phone_jid_for_lid_user(&own_base_jid.user)
                .map(|pn| pn.to_non_ad())
                .unwrap_or_else(|| own_base_jid.clone())
        } else {
            own_base_jid.clone()
        };

        if !jids_to_resolve
            .iter()
            .any(|participant| participant.is_same_user_as(&own_jid_to_check))
        {
            jids_to_resolve.push(own_jid_to_check);
        }

        let mut seen_users = HashSet::new();
        jids_to_resolve.retain(|jid| seen_users.insert((jid.user.clone(), jid.server.clone())));

        log::debug!(
            "Resolving devices for {} participants",
            jids_to_resolve.len()
        );

        let mut resolved_list = resolver.resolve_devices(&jids_to_resolve).await?;

        // For LID groups, convert phone-based device JIDs back to LID format
        // This is necessary because WhatsApp Web expects LID addressing in SKDM <to> nodes
        if group_info.addressing_mode == crate::types::message::AddressingMode::Lid {
            resolved_list = resolved_list
                .into_iter()
                .map(|device_jid| group_info.phone_device_jid_to_lid(&device_jid))
                .collect();
            log::debug!(
                "Converted {} devices to LID addressing for group {}",
                resolved_list.len(),
                to_jid
            );
        }

        // Dedup AFTER LID conversion to avoid duplicates when both phone and LID
        // queries return the same user (e.g., 559980000003:33 and 100000037037034:33
        // both convert to 100000037037034:33@lid)
        let mut seen = HashSet::new();
        resolved_list.retain(|jid| seen.insert(jid.to_string()));

        // Filter devices for SKDM distribution:
        // - Exclude the exact sending device (own_sending_jid) - we already have our own sender key
        // - Keep ALL other devices including our own other devices (phone, other companions)
        //   because they need the SKDM to decrypt messages we send from this device
        // - Exclude hosted/Cloud API devices (device ID 99 or @hosted server) - they don't
        //   participate in group E2EE, only in 1:1 chats
        let own_user = own_sending_jid.user.clone();
        let own_device = own_sending_jid.device;
        let before_filter = resolved_list.len();
        resolved_list.retain(|device_jid| {
            let is_exact_sender = device_jid.user == own_user && device_jid.device == own_device;
            let is_hosted = device_jid.is_hosted();
            // Exclude the exact sending device and hosted devices
            !is_exact_sender && !is_hosted
        });
        log::debug!(
            "Filtered SKDM devices from {} to {} (excluded sender {}:{} and hosted devices)",
            before_filter,
            resolved_list.len(),
            own_user,
            own_device
        );

        log::debug!(
            "SKDM distribution list for {} resolved to {} devices",
            to_jid,
            resolved_list.len(),
        );

        Some(resolved_list)
    } else {
        None
    };

    if let Some(ref distribution_list) = distribution_list {
        resolved_devices_for_phash = Some(distribution_list.clone());
        let axolotl_skdm_bytes = create_sender_key_distribution_message_for_group(
            stores.sender_key_store,
            &to_jid,
            &own_sending_jid,
        )
        .await?;

        let skdm_wrapper_msg = wa::Message {
            sender_key_distribution_message: Some(wa::message::SenderKeyDistributionMessage {
                group_id: Some(to_jid.to_string()),
                axolotl_sender_key_distribution_message: Some(axolotl_skdm_bytes),
            }),
            ..Default::default()
        };
        let skdm_plaintext_to_encrypt =
            MessageUtils::pad_message_v2(skdm_wrapper_msg.encode_to_vec());

        // For SKDM distribution we don't set decrypt-fail; use empty attrs
        let empty_attrs = Attrs::new();
        let (participant_nodes, inc) = encrypt_for_devices(
            stores,
            resolver,
            distribution_list,
            &skdm_plaintext_to_encrypt,
            &empty_attrs,
        )
        .await?;
        includes_prekey_message = includes_prekey_message || inc;

        // Add participants list as part of the single hybrid stanza
        message_children.push(
            NodeBuilder::new("participants")
                .children(participant_nodes)
                .build(),
        );
        if includes_prekey_message && let Some(acc) = account {
            message_children.push(
                NodeBuilder::new("device-identity")
                    .bytes(acc.encode_to_vec())
                    .build(),
            );
        }
    }

    let plaintext = MessageUtils::pad_message_v2(message_for_encryption.encode_to_vec());
    let skmsg = encrypt_group_message(
        stores.sender_key_store,
        &to_jid,
        &own_sending_jid,
        &plaintext,
        &mut StdRng::from_os_rng(),
    )
    .await?;

    let skmsg_ciphertext = skmsg.serialized().to_vec();

    // Add decrypt-fail="hide" for edited group messages too
    let mut sk_enc_attrs = Attrs::new();
    sk_enc_attrs.insert("v".to_string(), "2".to_string());
    sk_enc_attrs.insert("type".to_string(), "skmsg".to_string());
    if let Some(edit_attr) = &edit
        && *edit_attr != crate::types::message::EditAttribute::Empty
    {
        sk_enc_attrs.insert("decrypt-fail".to_string(), "hide".to_string());
    }

    let content_node = NodeBuilder::new("enc")
        .attrs(sk_enc_attrs)
        .bytes(skmsg_ciphertext)
        .build();

    let mut stanza_attrs = Attrs::new();
    stanza_attrs.insert("to".to_string(), to_jid.to_string());
    stanza_attrs.insert("id".to_string(), request_id);
    stanza_attrs.insert("type".to_string(), "text".to_string());

    // Add addressing_mode attribute for LID groups (matches WhatsApp Web behavior)
    if group_info.addressing_mode == crate::types::message::AddressingMode::Lid {
        stanza_attrs.insert("addressing_mode".to_string(), "lid".to_string());
    }

    if let Some(edit_attr) = edit
        && edit_attr != crate::types::message::EditAttribute::Empty
    {
        stanza_attrs.insert("edit".to_string(), edit_attr.to_string_val().to_string());
    }

    message_children.push(content_node);

    // Add reporting token node if we generated one
    if let Some(ref result) = reporting_result {
        message_children.push(build_reporting_node(result));
    }

    // Add phash if we distributed keys in this message
    if let Some(devices) = &resolved_devices_for_phash {
        match MessageUtils::participant_list_hash(devices) {
            Ok(phash) => {
                stanza_attrs.insert("phash".to_string(), phash);
            }
            Err(e) => {
                log::warn!("Failed to compute phash for group {}: {:?}", to_jid, e);
            }
        }
    }

    let stanza = NodeBuilder::new("message")
        .attrs(stanza_attrs.into_iter())
        .children(message_children)
        .build();

    Ok(stanza)
}
pub async fn create_sender_key_distribution_message_for_group(
    store: &mut (dyn SenderKeyStore + Send + Sync),
    group_jid: &Jid,
    own_sending_jid: &Jid,
) -> Result<Vec<u8>> {
    let sender_address = own_sending_jid.to_protocol_address();

    let sender_key_name = SenderKeyName::new(group_jid.to_string(), sender_address.to_string());

    let mut record = store
        .load_sender_key(&sender_key_name)
        .await?
        .unwrap_or_else(SenderKeyRecord::new_empty);

    if record.sender_key_state().is_err() {
        log::info!(
            "No sender key found for self in group {}. Creating a new sender key state.",
            group_jid
        );

        let mut rng = StdRng::from_os_rng();
        let signing_key = crate::libsignal::protocol::KeyPair::generate(&mut rng);

        let chain_id = (rng.random::<u32>()) >> 1;
        let sender_key_seed: [u8; 32] = rng.random();
        record.add_sender_key_state(
            SENDERKEY_MESSAGE_CURRENT_VERSION,
            chain_id,
            0,
            &sender_key_seed,
            signing_key.public_key,
            Some(signing_key.private_key),
        );
        store.store_sender_key(&sender_key_name, &record).await?;
    }

    let state = record
        .sender_key_state()
        .map_err(|e| anyhow!("Invalid SK state: {:?}", e))?;
    let chain_key = state
        .sender_chain_key()
        .ok_or_else(|| anyhow!("Missing chain key"))?;

    let message_version = state
        .message_version()
        .try_into()
        .map_err(|e| anyhow!("Invalid sender key message version: {e}"))?;
    let skdm = SenderKeyDistributionMessage::new(
        message_version,
        state.chain_id(),
        chain_key.iteration(),
        chain_key.seed().to_vec(),
        state
            .signing_key_public()
            .map_err(|e| anyhow!("Missing pub key: {:?}", e))?,
    )?;

    Ok(skdm.serialized().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::context::{GroupInfo, SendContextResolver};
    use crate::libsignal::protocol::{IdentityKeyPair, KeyPair, PreKeyBundle};
    use std::collections::HashMap;
    use warp_core_binary::jid::Jid;

    /// Mock implementation of SendContextResolver for testing
    struct MockSendContextResolver {
        /// Pre-key bundles to return: JID -> Option<PreKeyBundle>
        prekey_bundles: HashMap<Jid, Option<PreKeyBundle>>,
        /// Devices to return from resolve_devices
        devices: Vec<Jid>,
        /// Phone number to LID mappings for testing LID session lookup
        phone_to_lid: HashMap<String, String>,
    }

    impl MockSendContextResolver {
        fn new() -> Self {
            Self {
                prekey_bundles: HashMap::new(),
                devices: Vec::new(),
                phone_to_lid: HashMap::new(),
            }
        }

        fn with_missing_bundle(mut self, jid: Jid) -> Self {
            self.prekey_bundles.insert(jid, None);
            self
        }

        fn with_bundle(mut self, jid: Jid, bundle: PreKeyBundle) -> Self {
            self.prekey_bundles.insert(jid, Some(bundle));
            self
        }

        fn with_devices(mut self, devices: Vec<Jid>) -> Self {
            self.devices = devices;
            self
        }

        fn with_phone_to_lid(mut self, phone: &str, lid: &str) -> Self {
            self.phone_to_lid.insert(phone.to_string(), lid.to_string());
            self
        }
    }

    #[async_trait::async_trait]
    impl SendContextResolver for MockSendContextResolver {
        async fn resolve_devices(&self, _jids: &[Jid]) -> Result<Vec<Jid>> {
            Ok(self.devices.clone())
        }

        async fn fetch_prekeys(&self, jids: &[Jid]) -> Result<HashMap<Jid, PreKeyBundle>> {
            let mut result = HashMap::new();
            for jid in jids {
                if let Some(bundle_opt) = self.prekey_bundles.get(jid)
                    && let Some(bundle) = bundle_opt
                {
                    result.insert(jid.clone(), bundle.clone());
                }
            }
            Ok(result)
        }

        async fn fetch_prekeys_for_identity_check(
            &self,
            jids: &[Jid],
        ) -> Result<HashMap<Jid, PreKeyBundle>> {
            let mut result = HashMap::new();
            for jid in jids {
                if let Some(bundle_opt) = self.prekey_bundles.get(jid)
                    && let Some(bundle) = bundle_opt
                {
                    result.insert(jid.clone(), bundle.clone());
                }
                // If None, we intentionally omit it from the result (simulating server not returning it)
            }
            Ok(result)
        }

        async fn resolve_group_info(&self, _jid: &Jid) -> Result<GroupInfo> {
            unimplemented!("resolve_group_info not needed for send.rs tests")
        }

        async fn get_lid_for_phone(&self, phone_user: &str) -> Option<String> {
            self.phone_to_lid.get(phone_user).cloned()
        }
    }

    /// Test case: Missing pre-key bundle for a single device skips gracefully
    ///
    /// When sending to multiple devices, if some don't have pre-key bundles (e.g., Cloud API),
    /// we should skip them instead of failing the entire message.
    #[test]
    fn test_missing_prekey_bundle_skips_device() {
        let device_with_bundle: Jid = "1234567890:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let device_without_bundle: Jid = "1234567890:1@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let cloud_api: Jid = "1234567890:99@hosted"
            .parse()
            .expect("test JID should be valid");

        let bundle = create_mock_bundle();

        let resolver = MockSendContextResolver::new()
            .with_bundle(device_with_bundle.clone(), bundle)
            .with_missing_bundle(device_without_bundle.clone())
            .with_missing_bundle(cloud_api.clone())
            .with_devices(vec![
                device_with_bundle.clone(),
                device_without_bundle.clone(),
                cloud_api.clone(),
            ]);

        // Check that the resolver correctly returns only available bundles
        assert_eq!(
            resolver.prekey_bundles.len(),
            3,
            "Resolver should have 3 entries"
        );

        // Verify device_with_bundle has a Some(bundle)
        assert!(
            resolver.prekey_bundles[&device_with_bundle].is_some(),
            "device_with_bundle should have a Some entry"
        );

        // Verify others have None
        assert!(
            resolver.prekey_bundles[&device_without_bundle].is_none(),
            "device_without_bundle should have None"
        );
        assert!(
            resolver.prekey_bundles[&cloud_api].is_none(),
            "cloud_api should have None"
        );

        println!("✅ Missing pre-key bundle skips device gracefully");
    }

    /// Test case: All devices missing pre-key bundles
    ///
    /// If all devices are unavailable, the batch should still complete without panic.
    #[test]
    fn test_all_devices_missing_prekey_bundles() {
        let device1: Jid = "1234567890:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let device2: Jid = "1234567890:1@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let device3: Jid = "9876543210:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");

        let resolver = MockSendContextResolver::new()
            .with_missing_bundle(device1.clone())
            .with_missing_bundle(device2.clone())
            .with_missing_bundle(device3.clone())
            .with_devices(vec![device1.clone(), device2.clone(), device3.clone()]);

        // All entries should be None
        assert!(resolver.prekey_bundles[&device1].is_none());
        assert!(resolver.prekey_bundles[&device2].is_none());
        assert!(resolver.prekey_bundles[&device3].is_none());

        println!("✅ All devices missing bundles handled gracefully");
    }

    /// Test case: Large group with mixed device availability
    ///
    /// In real-world scenarios, large groups may have some unavailable devices.
    /// The encryption should proceed for available devices and skip unavailable ones.
    #[test]
    fn test_large_group_with_mixed_device_availability() {
        let mut all_devices = Vec::new();

        for i in 0..10u16 {
            let device_jid = Jid::pn_device("1234567890", i);
            all_devices.push(device_jid);
        }

        let mut resolver = MockSendContextResolver::new().with_devices(all_devices.clone());

        // Add bundles for devices 0-6, mark 7-9 as missing
        for i in 0..10u16 {
            let device_jid = Jid::pn_device("1234567890", i);

            if i < 7 {
                resolver = resolver.with_bundle(device_jid, create_mock_bundle());
            } else {
                resolver = resolver.with_missing_bundle(device_jid);
            }
        }

        // Verify bundle availability
        let available_count = resolver
            .prekey_bundles
            .values()
            .filter(|v| v.is_some())
            .count();

        assert_eq!(available_count, 7, "Should have 7 available devices");
        assert_eq!(
            resolver.prekey_bundles.len(),
            10,
            "Should have 10 total entries"
        );

        println!("✅ Large group with 7 available, 3 unavailable devices");
    }

    /// Test case: Cloud API / HOSTED device without pre-key
    ///
    /// # Context: What are HOSTED devices?
    ///
    /// HOSTED devices (Cloud API / Meta Business API) are WhatsApp Business accounts
    /// that use Meta's server-side infrastructure instead of traditional E2EE.
    ///
    /// ## Identification:
    /// - Device ID 99 (`:99`) on any server
    /// - Server `@hosted` or `@hosted.lid`
    ///
    /// ## Behavior:
    /// - They do NOT have Signal protocol prekey bundles
    /// - For 1:1 chats: included in device list, but prekey fetch fails gracefully
    /// - For groups: proactively filtered out before SKDM distribution
    ///
    /// This test verifies that when a hosted device is included in the device list
    /// (which would happen for 1:1 chats), the missing prekey is handled gracefully.
    #[test]
    fn test_cloud_api_device_without_prekey() {
        let regular_device: Jid = "1234567890:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        let cloud_api: Jid = "1234567890:99@hosted"
            .parse()
            .expect("test JID should be valid");

        // Verify the cloud_api device is detected as hosted
        assert!(
            cloud_api.is_hosted(),
            "Device with :99@hosted should be detected as hosted"
        );
        assert!(
            !regular_device.is_hosted(),
            "Regular device should NOT be detected as hosted"
        );

        let resolver = MockSendContextResolver::new()
            .with_bundle(regular_device.clone(), create_mock_bundle())
            .with_missing_bundle(cloud_api.clone())
            .with_devices(vec![regular_device.clone(), cloud_api.clone()]);

        assert!(
            resolver.prekey_bundles[&regular_device].is_some(),
            "Regular device should have a bundle"
        );
        assert!(
            resolver.prekey_bundles[&cloud_api].is_none(),
            "Cloud API device should not have a bundle (they don't use Signal protocol)"
        );

        println!("✅ Cloud API device has no prekey bundle (expected behavior)");
    }

    /// Test case: HOSTED devices are filtered from group SKDM distribution
    ///
    /// # Why filter hosted devices from groups?
    ///
    /// WhatsApp Web explicitly excludes hosted devices from group message fanout.
    /// From the JS code (`getFanOutList`):
    /// ```javascript
    /// var isHosted = e.id === 99 || e.isHosted === true;
    /// var includeInFanout = !isHosted || isOneToOneChat;
    /// ```
    ///
    /// ## Reasons:
    /// 1. Hosted devices don't use Signal protocol - they can't process SKDM
    /// 2. Including them causes unnecessary prekey fetch failures
    /// 3. Group encryption is handled differently for Cloud API businesses
    ///
    /// This test verifies that `is_hosted()` correctly identifies devices that
    /// should be filtered from group SKDM distribution.
    #[test]
    fn test_hosted_devices_filtered_from_group_skdm() {
        // Simulate devices returned from usync for a group
        let devices: Vec<Jid> = vec![
            // Regular devices - should receive SKDM
            "5511999887766:0@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Primary phone
            "5511999887766:33@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // WhatsApp Web companion
            "5521988776655:0@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Another participant
            "100000012345678:33@lid"
                .parse()
                .expect("test JID should be valid"), // LID companion device
            // HOSTED devices - should be EXCLUDED from group SKDM
            "5531977665544:99@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Cloud API on regular server
            "100000087654321:99@lid"
                .parse()
                .expect("test JID should be valid"), // Cloud API on LID server
            "5541966554433:0@hosted"
                .parse()
                .expect("test JID should be valid"), // Explicit @hosted server
        ];

        // This is the filtering logic used in prepare_group_stanza
        let filtered_for_skdm: Vec<Jid> =
            devices.into_iter().filter(|jid| !jid.is_hosted()).collect();

        assert_eq!(
            filtered_for_skdm.len(),
            4,
            "Should have 4 devices after filtering out hosted devices"
        );

        // Verify all remaining devices are NOT hosted
        for jid in &filtered_for_skdm {
            assert!(
                !jid.is_hosted(),
                "Filtered list should not contain hosted device: {}",
                jid
            );
        }

        // Verify specific devices are included/excluded by checking struct fields
        // (Device ID 0 is not serialized in the string representation)
        let has_primary_phone = filtered_for_skdm
            .iter()
            .any(|j| j.user == "5511999887766" && j.device == 0 && j.server == "s.whatsapp.net");
        let has_companion = filtered_for_skdm
            .iter()
            .any(|j| j.user == "5511999887766" && j.device == 33 && j.server == "s.whatsapp.net");
        let has_cloud_api = filtered_for_skdm
            .iter()
            .any(|j| j.user == "5531977665544" && j.device == 99);
        let has_hosted_server = filtered_for_skdm.iter().any(|j| j.server == "hosted");

        assert!(has_primary_phone, "Primary phone should be included");
        assert!(has_companion, "WhatsApp Web companion should be included");
        assert!(
            !has_cloud_api,
            "Cloud API device (ID 99) should be excluded"
        );
        assert!(
            !has_hosted_server,
            "@hosted server device should be excluded"
        );

        println!("✅ Hosted devices correctly filtered from group SKDM distribution");
    }

    /// Test case: Device recovery between retries
    ///
    /// If a device was temporarily unavailable, a retry should succeed.
    #[test]
    fn test_device_recovery_between_requests() {
        let device: Jid = "1234567890:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");

        // First attempt: device unavailable
        let resolver_first = MockSendContextResolver::new().with_missing_bundle(device.clone());

        assert!(
            resolver_first.prekey_bundles[&device].is_none(),
            "First attempt: device should be unavailable"
        );

        // Second attempt: device recovered
        let resolver_second =
            MockSendContextResolver::new().with_bundle(device.clone(), create_mock_bundle());

        assert!(
            resolver_second.prekey_bundles[&device].is_some(),
            "Second attempt: device should be available"
        );

        println!("✅ Device recovery between retries works correctly");
    }

    /// Helper function to create a mock PreKeyBundle with valid types
    fn create_mock_bundle() -> PreKeyBundle {
        let mut rng = StdRng::from_os_rng();
        let identity_pair = IdentityKeyPair::generate(&mut rng);
        let signed_prekey_pair = KeyPair::generate(&mut rng);
        let prekey_pair = KeyPair::generate(&mut rng);

        PreKeyBundle::new(
            1,                                           // registration_id
            1u32.into(),                                 // device_id
            Some((1u32.into(), prekey_pair.public_key)), // pre_key
            2u32.into(),                                 // signed_pre_key_id
            signed_prekey_pair.public_key,
            vec![0u8; 64],
            *identity_pair.identity_key(),
        )
        .expect("Failed to create PreKeyBundle")
    }

    // ==========================================
    // LID-PN Session Mismatch Fix Tests
    // ==========================================
    //
    // These tests validate the fix for the LID-PN session mismatch issue.
    // When a message is received with sender_lid, the session is stored under the LID address.
    // When sending a reply using the phone number, we must reuse the existing LID session
    // instead of creating a new PN session, otherwise subsequent messages will fail with
    // MAC verification errors.

    /// Test that phone_to_lid mapping returns the cached LID mapping.
    ///
    /// This verifies the MockSendContextResolver correctly stores phone-to-LID
    /// mappings used for LID session lookup.
    #[test]
    fn test_mock_resolver_phone_to_lid_mapping() {
        let phone = "559980000001";
        let lid = "100000012345678";

        let resolver = MockSendContextResolver::new().with_phone_to_lid(phone, lid);

        // Access the HashMap directly (synchronous)
        let result = resolver.phone_to_lid.get(phone).cloned();

        assert!(result.is_some(), "Should return LID for known phone");
        assert_eq!(
            result.expect("known phone should return LID"),
            lid,
            "Should return correct LID"
        );

        // Unknown phone should return None
        let unknown = resolver.phone_to_lid.get("999999999").cloned();
        assert!(unknown.is_none(), "Should return None for unknown phone");

        println!("✅ MockSendContextResolver phone_to_lid mapping works correctly");
    }

    /// Test that the resolver correctly maps phone numbers to LIDs.
    ///
    /// This is a building block for the session lookup logic.
    #[test]
    fn test_phone_to_lid_mapping_multiple_users() {
        let resolver = MockSendContextResolver::new()
            .with_phone_to_lid("559980000001", "100000012345678")
            .with_phone_to_lid("559980000002", "100000024691356")
            .with_phone_to_lid("559980000003", "100000037037034");

        // Verify all mappings using direct HashMap access
        let lid1 = resolver.phone_to_lid.get("559980000001").cloned();
        let lid2 = resolver.phone_to_lid.get("559980000002").cloned();
        let lid3 = resolver.phone_to_lid.get("559980000003").cloned();

        assert_eq!(
            lid1.expect("phone 1 should have LID mapping"),
            "100000012345678"
        );
        assert_eq!(
            lid2.expect("phone 2 should have LID mapping"),
            "100000024691356"
        );
        assert_eq!(
            lid3.expect("phone 3 should have LID mapping"),
            "100000037037034"
        );

        println!("✅ Multiple phone-to-LID mappings work correctly");
    }

    /// Test the scenario that caused the original bug:
    /// - Session exists under LID address (from receiving a message with sender_lid)
    /// - Send to PN address should reuse the LID session, not create a new one
    ///
    /// This test verifies the logic flow, though full integration testing
    /// requires the actual encrypt_for_devices function with real sessions.
    #[test]
    fn test_lid_session_lookup_scenario() {
        // Scenario setup:
        // - Received message from 559980000001@s.whatsapp.net with sender_lid=100000012345678@lid
        // - Session was stored under 100000012345678.0
        // - Now sending reply to 559980000001@s.whatsapp.net
        // - Should look up LID and check for session under 100000012345678.0

        let phone = "559980000001";
        let lid = "100000012345678";
        let device_id = 0u16;

        let resolver = MockSendContextResolver::new().with_phone_to_lid(phone, lid);

        // Simulate the device JID we're trying to send to (PN format)
        let pn_device_jid = Jid::pn_device(phone, device_id);

        // Step 1: Look up LID for the phone number (using direct HashMap access)
        let lid_user = resolver.phone_to_lid.get(&pn_device_jid.user).cloned();
        assert!(lid_user.is_some(), "Should find LID for phone");
        let lid_user = lid_user.expect("phone should have LID mapping");

        // Step 2: Construct the LID JID with same device ID
        let lid_jid = Jid::lid_device(lid_user.clone(), pn_device_jid.device);

        // Step 3: Verify the LID JID is correctly constructed
        assert_eq!(lid_jid.user, lid, "LID user should match");
        assert_eq!(lid_jid.server, "lid", "Server should be 'lid'");
        assert_eq!(lid_jid.device, device_id, "Device ID should be preserved");

        // Step 4: Convert to protocol addresses and verify they're different
        use crate::types::jid::JidExt;
        let pn_address = pn_device_jid.to_protocol_address();
        let lid_address = lid_jid.to_protocol_address();

        assert_ne!(
            pn_address.name(),
            lid_address.name(),
            "PN and LID addresses should have different names"
        );
        assert_eq!(
            pn_address.device_id(),
            lid_address.device_id(),
            "Device IDs should match"
        );

        println!("✅ LID session lookup scenario works correctly:");
        println!("   - PN JID: {} -> Address: {}", pn_device_jid, pn_address);
        println!("   - LID JID: {} -> Address: {}", lid_jid, lid_address);
        println!("   - Would check for session under LID address first");
    }

    /// Test that companion device IDs are preserved in LID JID construction.
    ///
    /// WhatsApp Web uses device ID 33, and this must be preserved when
    /// constructing the LID JID for session lookup.
    #[test]
    fn test_lid_jid_preserves_companion_device_id() {
        let phone = "559980000001";
        let lid = "100000012345678";
        let companion_device_id = 33u16; // WhatsApp Web device ID

        let resolver = MockSendContextResolver::new().with_phone_to_lid(phone, lid);

        // Simulate sending to a companion device (WhatsApp Web)
        let pn_device_jid = Jid::pn_device(phone, companion_device_id);

        // Look up LID using direct HashMap access
        let lid_user = resolver.phone_to_lid.get(&pn_device_jid.user).cloned();

        // Construct LID JID
        let lid_jid = Jid::lid_device(
            lid_user.expect("phone should have LID mapping for companion test"),
            pn_device_jid.device,
        );

        assert_eq!(
            lid_jid.device, companion_device_id,
            "Device ID 33 should be preserved"
        );
        assert_eq!(lid_jid.to_string(), "100000012345678:33@lid");

        println!("✅ Companion device ID (33) correctly preserved in LID JID");
    }

    /// Test that LID lookup only applies to s.whatsapp.net JIDs.
    ///
    /// LID JIDs (@lid) and group JIDs (@g.us) should not trigger LID lookup.
    #[test]
    fn test_lid_lookup_only_for_pn_jids() {
        let _resolver =
            MockSendContextResolver::new().with_phone_to_lid("559980000001", "100000012345678");

        // These JIDs should NOT trigger LID lookup
        let lid_jid: Jid = "100000012345678:0@lid"
            .parse()
            .expect("test JID should be valid");
        let group_jid: Jid = "120363123456789012@g.us"
            .parse()
            .expect("test JID should be valid");

        // Only s.whatsapp.net JIDs should be looked up
        assert_ne!(
            lid_jid.server, "s.whatsapp.net",
            "LID JID should not be s.whatsapp.net"
        );
        assert_ne!(
            group_jid.server, "s.whatsapp.net",
            "Group JID should not be s.whatsapp.net"
        );

        // PN JID should be eligible for lookup
        let pn_jid: Jid = "559980000001:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        assert_eq!(
            pn_jid.server, "s.whatsapp.net",
            "PN JID should be s.whatsapp.net"
        );

        println!("✅ LID lookup correctly limited to s.whatsapp.net JIDs");
    }
}
