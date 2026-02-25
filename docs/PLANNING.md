# PLANNING — whatsapp-rs (Baileys in Rust)

> Documento de planejamento para agentes de codificação (Codex/Claude Code).
> Siga as tasks em ordem. Cada task é atômica e testável de forma isolada.

---

## Stack

| Camada | Crate |
|---|---|
| Async runtime | `tokio` (full features) |
| WebSocket | `tokio-tungstenite` + `native-tls` |
| Protobuf | `prost` + `prost-build` |
| Criptografia Noise | `x25519-dalek`, `aes-gcm`, `hkdf`, `sha2` |
| Signal Protocol | `libsignal-client` |
| Serialização | `serde`, `serde_json`, `bytes` |
| Logging | `tracing`, `tracing-subscriber` |
| Erros | `thiserror`, `anyhow` |
| Testes | `tokio-test`, `wiremock` |

---

## Milestones

```
M1 → Transport + Framing
M2 → Noise Handshake
M3 → Binary Node Parser
M4 → Auth / QR Code
M5 → Signal Protocol E2E
M6 → Message API
M7 → Events & Reconnect
M8 → CLI demo
```

---

## Tasks Detalhadas

### M1 — WebSocket Transport & Frame Layer

**Objetivo:** Conectar ao endpoint do WhatsApp Web e trocar frames binários brutos.

- [ ] **T1.1** — Criar `src/transport.rs`
  - Struct `WsTransport` com campo `ws: WebSocketStream<MaybeTlsStream<TcpStream>>`
  - `WsTransport::connect() -> Result<Self>` — conecta em `wss://web.whatsapp.com/ws/chat` com headers `Origin: https://web.whatsapp.com`
  - `send_frame(&mut self, data: &[u8]) -> Result<()>` — prefixo de 3 bytes big-endian com o tamanho do payload
  - `next_frame(&mut self) -> Result<Bytes>` — lê próxima mensagem binária, strip dos 3 bytes de header
  - Ignorar frames `Ping`/`Pong` e responder automaticamente

- [ ] **T1.2** — Testes unitários `transport`
  - Mock server local com `tokio::net::TcpListener` + upgrade WebSocket manual
  - Testar round-trip de frame com tamanhos: 0 bytes, 1 byte, 65535 bytes, 65536 bytes (overflow 3 bytes)

---

### M2 — Noise Protocol Handshake

**Objetivo:** Implementar `Noise_XX_25519_AESGCM_SHA256` customizado pelo WhatsApp.

- [ ] **T2.1** — Criar `src/noise.rs` — struct `NoiseState`
  - Campos: `hash: [u8;32]`, `chaining_key: [u8;32]`, `enc_key: Option<[u8;32]>`, `dec_key: Option<[u8;32]>`, `enc_counter: u64`, `dec_counter: u64`
  - `NoiseState::new(prologue: &[u8]) -> Self` — inicializar `hash` e `chaining_key` com `SHA256(prologue)`; prologue = `b"WA\x06\x05"`
  - `mix_hash(&mut self, data: &[u8])` — `hash = SHA256(hash || data)`
  - `mix_into_key(&mut self, dh_output: &[u8])` — HKDF com salt=`chaining_key`, derive 64 bytes → `[chaining_key, temp_key]`
  - `encrypt_with_ad(&mut self, plaintext: &[u8], ad: &[u8]) -> Vec<u8>` — AES-256-GCM, nonce de 12 bytes (8 zero + 4 bytes counter big-endian), incrementa `enc_counter`, usa `hash` como AD
  - `decrypt_with_ad(&mut self, ciphertext: &[u8], ad: &[u8]) -> Result<Vec<u8>>`

- [ ] **T2.2** — Criar `src/handshake.rs` — struct `Handshake`
  - `do_handshake(transport: &mut WsTransport, keypair: &KeyPair) -> Result<NoiseState>`
  - Passo 1: gerar ephemeral keypair X25519
  - Passo 2: enviar `ClientHello` (Protobuf `HandshakeMessage`) com `ephemeralPublic`
  - Passo 3: receber `ServerHello`, extrair `ephemeralPublic` do servidor e `staticPublic` encriptado
  - Passo 4: DH(client_ephemeral_private, server_ephemeral_public) → `mix_into_key`
  - Passo 5: descriptografar `staticPublic` do servidor
  - Passo 6: DH(client_ephemeral_private, server_static_public) → `mix_into_key`
  - Passo 7: encriptar `certificate` do cliente + `static_public` → enviar `ClientFinish`
  - Retornar `NoiseState` com chaves de sessão estabelecidas

- [ ] **T2.3** — Testes
  - Testar `mix_hash` e `mix_into_key` com vetores conhecidos do Noise Protocol spec
  - Testar encrypt/decrypt round-trip com counters incrementais

---

### M3 — Binary Node Parser

**Objetivo:** Parsear o formato binário proprietário do WhatsApp (não é Protobuf puro).

- [ ] **T3.1** — Criar `src/binary_node.rs`
  - Struct `BinaryNode { tag: String, attrs: HashMap<String, String>, content: NodeContent }`
  - Enum `NodeContent { Nodes(Vec<BinaryNode>), Bytes(Bytes), Empty }`
  - `decode(data: &[u8]) -> Result<BinaryNode>` — implementar o decodificador baseado na tag dictionary do WhatsApp (lista de ~200 tokens predefinidos)
  - `encode(node: &BinaryNode) -> Vec<u8>` — codificador inverso
  - Incluir a `SINGLE_BYTE_TOKENS: [&str; 256]` dictionary completa (copiar do Baileys `src/WABinary/token.ts`)

- [ ] **T3.2** — Testes
  - Fixture: capturar frames reais via Baileys em modo debug, salvar como `.bin`, parsear e verificar campos

---

### M4 — Auth State & QR Code

**Objetivo:** Gerenciar credenciais e fluxo de autenticação via QR.

- [ ] **T4.1** — Criar `src/keys.rs`
  - `KeyPair { public: [u8;32], private: [u8;32] }`
  - `fn generate_keypair() -> KeyPair` usando `x25519-dalek`
  - `fn generate_registration_id() -> u32` — random 14-bit

- [ ] **T4.2** — Criar `src/auth.rs`
  - Struct `AuthState` com serde Serialize/Deserialize
    ```
    identity_key: KeyPair
    registration_id: u32
    signed_pre_key: KeyPair
    signed_pre_key_sig: [u8;64]
    one_time_pre_keys: Vec<KeyPair>
    me: Option<{ jid: String, name: String }>
    ```
  - `AuthState::new() -> Self` — gera todos os keys
  - `AuthState::save(path: &Path) -> Result<()>`
  - `AuthState::load(path: &Path) -> Result<Self>`

- [ ] **T4.3** — Criar `src/qr.rs`
  - `fn generate_qr_string(ref: &str, noise_pub: &[u8;32], identity_pub: &[u8;32], adv_key: &[u8;32]) -> String`
  - Formato: `{ref},{base64(noise_pub)},{base64(identity_pub)},{base64(adv_key)}`
  - `fn print_qr_terminal(qr_string: &str)` — usar crate `qrcode` para imprimir no terminal
  - Emitir evento `QrCode(String)` via channel

- [ ] **T4.4** — Testar ciclo completo de save/load de AuthState

---

### M5 — Signal Protocol (E2E Encryption)

**Objetivo:** Implementar X3DH + Double Ratchet para mensagens E2E.

- [ ] **T5.1** — Adicionar `libsignal-client` no `Cargo.toml`
- [ ] **T5.2** — Criar `src/signal/store.rs`
  - Implementar traits `IdentityKeyStore`, `PreKeyStore`, `SignedPreKeyStore`, `SessionStore` do `libsignal-client`
  - Backend: `HashMap` in-memory + serialização JSON opcional para disco
- [ ] **T5.3** — Criar `src/signal/session.rs`
  - `fn init_session(their_jid: &str, their_bundle: PreKeyBundle, store: &mut dyn SignalStore) -> Result<()>` — X3DH
  - `fn encrypt_message(jid: &str, plaintext: &[u8], store: &mut dyn SignalStore) -> Result<Vec<u8>>`
  - `fn decrypt_message(jid: &str, ciphertext: &[u8], store: &mut dyn SignalStore) -> Result<Vec<u8>>`
- [ ] **T5.4** — Testes de round-trip encrypt/decrypt entre dois stores locais

---

### M6 — Message API

**Objetivo:** Enviar e receber mensagens de texto, mídia e reações.

- [ ] **T6.1** — Gerar protos em `build.rs`
  - Colocar `WAProto.proto` e `WAWeb.proto` em `proto/`
  - `build.rs`: `prost_build::compile_protos(&["proto/WAProto.proto"], &["proto/"])?`

- [ ] **T6.2** — Criar `src/message.rs`
  - Struct `OutgoingMessage { to: String, content: MessageContent }`
  - Enum `MessageContent { Text(String), Image { data: Bytes, caption: Option<String> }, Reaction { key: MessageKey, emoji: String } }`
  - `fn build_message_node(msg: &OutgoingMessage, auth: &AuthState) -> BinaryNode`

- [ ] **T6.3** — Criar `src/socket.rs` — método `send_message`
  - Serializar com Protobuf
  - Encriptar com Signal (T5.3)
  - Encodar como `BinaryNode`
  - Encriptar com Noise (T2.1)
  - Enviar via Transport (T1.1)

- [ ] **T6.4** — Handler de mensagens recebidas
  - Decodificar frame → BinaryNode
  - Identificar tag `message` vs `ack` vs `notification`
  - Descriptografar payload Signal
  - Emitir `Event::Message(IncomingMessage)`

---

### M7 — Event System & Reconnect

**Objetivo:** API reativa baseada em channels + reconexão automática.

- [ ] **T7.1** — Criar `src/events.rs`
  ```rust
  pub enum Event {
      QrCode(String),
      Connected { jid: String },
      Message(IncomingMessage),
      MessageAck { id: String, status: AckStatus },
      Disconnected { reason: String },
      Error(anyhow::Error),
  }
  ```

- [ ] **T7.2** — Criar `src/client.rs` — struct `Client`
  - `Client::connect(auth_path: Option<PathBuf>) -> Result<(Self, mpsc::Receiver<Event>)>`
  - `client.send_text(jid: &str, text: &str) -> Result<String>` — retorna message ID
  - `client.send_image(jid: &str, path: &Path, caption: Option<&str>) -> Result<String>`
  - `client.run() -> Result<()>` — loop principal, deve ser spawnado em task separada

- [ ] **T7.3** — Reconnect logic em `src/client.rs`
  - Backoff exponencial: 1s, 2s, 4s, 8s, 16s, 30s (cap)
  - Tentar restaurar sessão Noise antes de pedir novo QR
  - Emitir `Event::Disconnected` antes de reconectar

- [ ] **T7.4** — Keep-alive
  - Task separada: enviar node `<ping/>` a cada 20s
  - Se não receber `<pong/>` em 5s → reconectar

---

### M8 — CLI Demo

**Objetivo:** Binário funcional que demonstra envio e recebimento de mensagens.

- [ ] **T8.1** — Criar `src/bin/demo.rs`
  ```rust
  #[tokio::main]
  async fn main() {
      let (client, mut events) = Client::connect(Some("session.json".into())).await?;
      tokio::spawn(async move { client.run().await });
      while let Some(event) = events.recv().await {
          match event {
              Event::QrCode(qr) => println!("Scan QR:\n{}", qr),
              Event::Connected { jid } => println!("Connected as {}", jid),
              Event::Message(msg) => println!("[{}] {}: {}", msg.timestamp, msg.from, msg.text),
              _ => {}
          }
      }
  }
  ```
- [ ] **T8.2** — Comando `send`: `cargo run --bin demo -- send +5511999999999 "Hello"`

---

## Regras para o Agente

1. **Uma task por commit.** Não misture tasks de milestones diferentes.
2. **Todo código novo precisa de pelo menos um teste.** `cargo test` deve passar antes de avançar.
3. **Não use `unwrap()` em código de produção.** Use `?` e `thiserror`.
4. **Não implemente criptografia do zero** além do Noise State. Use as crates listadas.
5. **Mantenha `src/lib.rs` como re-export** de todos os módulos públicos.
6. **Binary Node tokens:** copiar a lista exata de `Baileys/src/WABinary/token.ts` — não inventar.
7. Se um passo depende de um frame real do WhatsApp para testar, salve fixtures em `tests/fixtures/`.
8. **Feature flags:** colocar Signal Protocol atrás de `features = ["e2e"]` para não bloquear builds que só precisam de MVP.