# PLANNING.md

> Plano de execução do chatwarp-api (Direct WA Client).

---

## Status por milestone

| Milestone | Status | Observação |
|---|---|---|
| M0 Bootstrap | ✅ Entregue | Runtime mínimo + fallback 501 |
| M1 Transport | ✅ Entregue | WebSocket + framing + ping/pong |
| M2 Noise/Handshake | ✅ Entregue | Noise state + handshake sintético offline |
| M3 Binary Node | ✅ Entregue | Codec sintético + fixtures `.bin` |
| M4 Auth/QR/Persistência | ✅ Entregue | AuthState + QR + repo PostgreSQL |
| M5 Signal E2E | ✅ Entregue | Signal store/session sintéticos |
| M6 Instance Manager | ✅ Entregue | Manager + runner + rotas `/instance/*` |
| M7 Message API | ✅ Entregue | `/message/:operation/:instance_name` + builder sintético |
| M8 Event Pipeline | ✅ Entregue | dispatcher + webhook/ws/rabbit sintéticos |
| M9 Rotas chat/group | ⏳ Pendente | Próxima fase |
| M10 Hardening/Obs | ⏳ Pendente | Próxima fase |

---

## Entregas M0-M8

### M0 — Bootstrap

- [x] `src/lib.rs` com `pub async fn run() -> anyhow::Result<()>`
- [x] Runtime Axum em:
  - [x] `GET /`
  - [x] `GET /healthz`
  - [x] `GET /readyz`
- [x] Fallback `501` com payload padrão:
  - [x] `{ "error": "not_implemented", "route": "<path>" }`

### M1 — WebSocket Transport

- [x] `src/wa/transport.rs`
  - [x] `connect(url)`
  - [x] `send_frame(&[u8])`
  - [x] `next_frame()`
  - [x] Auto resposta `Ping -> Pong`
- [x] `tests/transport_test.rs`
  - [x] round-trip: `0`, `1`, `65535`, `65536` bytes

### M2 — Noise + Handshake

- [x] `src/wa/noise.rs`
  - [x] `new(prologue)`
  - [x] `mix_hash`
  - [x] `mix_into_key`
  - [x] `encrypt_with_ad`
  - [x] `decrypt_with_ad`
- [x] `src/wa/keys.rs`
  - [x] `KeyPair`
  - [x] `generate_keypair()`
- [x] `src/wa/handshake.rs`
  - [x] `do_handshake(...) -> Result<NoiseState, HandshakeError>`
- [x] Testes offline:
  - [x] `tests/noise_test.rs`
  - [x] `tests/handshake_test.rs`
- [x] fixtures sintéticas em `tests/fixtures/noise_synthetic/*.bin`

### M3 — Binary Node

- [x] `src/wa/binary_node.rs`
  - [x] `BinaryNode { tag, attrs, content }`
  - [x] `NodeContent::Nodes | Bytes | Empty`
  - [x] `decode(&[u8]) -> Result<BinaryNode, BinaryNodeError>`
  - [x] `encode(&BinaryNode) -> Result<Vec<u8>, BinaryNodeError>`
  - [x] `SINGLE_BYTE_TOKENS: [&str; 256]` (subset sintético para fase atual)
- [x] Testes e fixtures:
  - [x] `tests/binary_node_test.rs`
  - [x] `tests/fixtures/binary_node_synthetic/message_text.bin`
  - [x] `tests/fixtures/binary_node_synthetic/nested_items.bin`

### M4 — Auth / QR / Persistência

- [x] `src/wa/auth.rs`
  - [x] `AuthState::new()`
  - [x] `IdentityState` separado de `SessionMetadata`
- [x] `src/wa/keys.rs`
  - [x] `generate_registration_id() -> u32` (14-bit)
- [x] `src/wa/qr.rs`
  - [x] `generate_qr_string(ref, noise_pub, identity_pub, adv_key)`
  - [x] emissão não bloqueante via `emit_qr_code(...)`
- [x] `src/db/auth_repo.rs`
  - [x] `save(instance_name, state)` com upsert
  - [x] `load(instance_name)`
- [x] Migração SQL:
  - [x] `migrations/0001_create_auth_states.sql`
- [x] Testes:
  - [x] `tests/auth_state_test.rs`
  - [x] `tests/qr_test.rs`
  - [x] `tests/auth_repo_test.rs` (condicional com `TEST_DATABASE_URL`)

### M5 — Signal E2E (sintético)

- [x] `src/wa/signal/store.rs`
  - [x] `trait SignalStore` + traits de composição
  - [x] `InMemorySignalStore`
  - [x] Store de identity/prekey/signed-prekey/session
- [x] `src/wa/signal/session.rs`
  - [x] `init_session(jid, bundle, store)`
  - [x] `encrypt(jid, payload, store)`
  - [x] `decrypt(jid, payload, store)`
- [x] Testes:
  - [x] `tests/signal_test.rs`

### M6 — Instance Manager

- [x] `src/instance/mod.rs` — `InstanceManager`
  - [x] `create(name, config)`
  - [x] `get(name)`
  - [x] `delete(name)`
- [x] `src/instance/handle.rs`
  - [x] `InstanceHandle` com `tx`, `state`, subscribe de eventos
  - [x] `ConnectionState` (`Connecting | QrPending | Connected | Disconnected`)
- [x] `src/instance/runner.rs`
  - [x] loop de comandos por instância
  - [x] backoff exponencial com cap (`backoff_seconds`)
- [x] Rotas `/instance/*` em `src/app.rs`:
  - [x] `POST /instance/create`
  - [x] `DELETE /instance/delete/:name`
  - [x] `GET /instance/connectionState/:name`
  - [x] `GET /instance/connect/:name`
- [x] Testes:
  - [x] `tests/instance_manager_test.rs`
  - [x] `tests/instance_routes_test.rs`

### M7 — Message API

- [x] `src/wa/message.rs`
  - [x] `OutgoingMessage`
  - [x] `MessageContent` (Text/Image/Video/Audio/Sticker/Location/Contact/Reaction/Poll/List/Buttons/Template/Status)
  - [x] validação de operação permitida
  - [x] `build_message_node(...)`
- [x] `src/handlers/message.rs`
  - [x] `POST /message/:operation/:instance_name`
  - [x] validação de operação
  - [x] encode para `BinaryNode` e envio via `InstanceHandle`
  - [x] retorno `{ \"key\": { \"id\": message_id } }`
- [x] Testes:
  - [x] `tests/message_routes_test.rs`

### M8 — Event Pipeline

- [x] `src/events/dispatcher.rs`
  - [x] roteamento por instância para webhook/ws/rabbit
- [x] `src/events/webhook.rs`
  - [x] retry com backoff
  - [x] timeout por tentativa
- [x] `src/events/websocket.rs`
  - [x] broadcast para subscribers
- [x] `src/events/rabbitmq.rs`
  - [x] publish sintético com routing key `{instance_name}.{event_type}`
- [x] Testes:
  - [x] `tests/events_pipeline_test.rs`

---

## Observações desta fase

1. Noise/handshake continuam sintéticos e validados offline.
2. Binary node usa token dictionary sintético nesta fase.
3. Signal M5 está sintético (sem `libsignal-client`) para validar interfaces e fluxo.
4. Instance M6 usa runner sintético (sem socket WA real).
5. Message API M7 ainda usa payload binário sintético (não serialização WA real).
6. Event pipeline M8 usa transports sintéticos (sem integração externa real).
7. Fixtures reais de protocolo (captura WA real/Baileys) seguem como backlog de hardening.

---

## Regras mantidas

1. Sem sidecar/processo externo.
2. Uma task por commit.
3. `cargo clippy --all-targets -- -D warnings` sem warnings.
4. `cargo test` obrigatório antes de avançar.
