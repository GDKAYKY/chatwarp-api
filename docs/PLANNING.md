# PLANNING.md

> Plano de execução do chatwarp-api (Direct WA Client).

---

## Status por milestone

| Milestone | Status | Observação |
|---|---|---|
| M0 Bootstrap | ✅ Entregue | Runtime mínimo + fallback 501 |
| M1 Transport | ✅ Entregue | WebSocket + framing + ping/pong |
| M2 Noise/Handshake | ✅ Entregue | Noise state + handshake sintético offline |
| M3 Binary Node | ⏳ Pendente | Próxima fase |
| M4 Auth/QR/Persistência | ⏳ Pendente | Próxima fase |
| M5 Signal E2E | ⏳ Pendente | Próxima fase |
| M6 Instance Manager | ⏳ Pendente | Próxima fase |
| M7 Message API | ⏳ Pendente | Próxima fase |
| M8 Event Pipeline | ⏳ Pendente | Próxima fase |
| M9 Rotas chat/group | ⏳ Pendente | Próxima fase |
| M10 Hardening/Obs | ⏳ Pendente | Próxima fase |

---

## Entregas M0-M2

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

---

## Observações desta fase

1. O handshake implementado em M2 é sintético e validado offline (mock server).
2. Não há integração WA real nesta fase.
3. Fixtures reais de protocolo ficam como requisito para M3.

---

## Regras mantidas

1. Sem sidecar/processo externo.
2. Uma task por commit.
3. `cargo clippy --all-targets -- -D warnings` sem warnings.
4. `cargo test` obrigatório antes de avançar.
