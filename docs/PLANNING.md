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
| M5 Signal E2E | ⏳ Pendente | Próxima fase |
| M6 Instance Manager | ⏳ Pendente | Próxima fase |
| M7 Message API | ⏳ Pendente | Próxima fase |
| M8 Event Pipeline | ⏳ Pendente | Próxima fase |
| M9 Rotas chat/group | ⏳ Pendente | Próxima fase |
| M10 Hardening/Obs | ⏳ Pendente | Próxima fase |

---

## Entregas M0-M4

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

---

## Observações desta fase

1. Noise/handshake continuam sintéticos e validados offline.
2. Binary node usa token dictionary sintético nesta fase.
3. Fixtures reais de protocolo (captura WA real/Baileys) seguem como backlog de hardening.

---

## Regras mantidas

1. Sem sidecar/processo externo.
2. Uma task por commit.
3. `cargo clippy --all-targets -- -D warnings` sem warnings.
4. `cargo test` obrigatório antes de avançar.
