# PROJECT_ARCHITECTURE.md

> whatsapp-rs — cliente WhatsApp Web em Rust.
> Protocolo: WebSocket + Noise_XX_25519_AESGCM_SHA256 + Signal Protocol (E2E).

---

## Stack

| Responsabilidade | Crate |
|---|---|
| Runtime async | `tokio` |
| WebSocket | `tokio-tungstenite` |
| Protobuf | `prost` + `prost-build` |
| Noise / Crypto | `x25519-dalek`, `aes-gcm`, `hkdf`, `sha2` |
| Signal E2E | `libsignal-client` |
| Serialização | `serde`, `serde_json`, `bytes` |
| Erros | `thiserror`, `anyhow` |
| Logging | `tracing` |

---

## Camadas

```
User Code
  └── client.rs           API pública (send_text, send_image, run)
        └── socket.rs     Loop de I/O, keepalive, dispatch de eventos
              ├── message.rs      Construção de nodes de saída
              ├── events.rs       Enum de eventos para o caller
              ├── binary_node.rs  Encode/decode do formato binário WA
              ├── noise.rs        Cifra de sessão (AES-GCM + HKDF)
              ├── handshake.rs    Noise XX — troca de chaves inicial
              ├── signal/         E2E encryption (X3DH + Double Ratchet)
              └── transport.rs    WebSocket + framing de 3 bytes
```

Regra de dependência: camadas inferiores não importam camadas superiores.
`transport` não conhece `noise`. `noise` não conhece `binary_node`. Sem exceções.

---

## Módulos

### `transport.rs`

Única responsabilidade: WebSocket e framing de bytes.

```
WsTransport
  connect() → Result<Self>
  send_frame(&[u8]) → Result<()>
  next_frame() → Result<Bytes>
```

Frame format:
```
[ byte0: N>>16 ][ byte1: N>>8 ][ byte2: N&FF ][ payload: N bytes ]
```

---

### `noise.rs`

Estado do Noise Protocol. Sem I/O, sem dependências externas além de crypto.

```
NoiseState
  new(prologue: &[u8]) → Self        prologue = b"WA\x06\x05"
  mix_hash(&[u8])
  mix_into_key(&[u8])                HKDF-SHA256 → chaining_key + session_key
  encrypt_with_ad(&[u8], &[u8]) → Vec<u8>
  decrypt_with_ad(&[u8], &[u8]) → Result<Vec<u8>>
```

Parâmetros fixos: SHA-256 / X25519 / AES-256-GCM / nonce = 8 zeros + counter (4 bytes BE).

---

### `handshake.rs`

Executa Noise XX sobre o Transport. Retorna NoiseState pronto para uso.

```
do_handshake(transport, keypair) → Result<NoiseState>
```

```
Client                            Server
  ephemeral_pub  ── ClientHello ──►
                 ◄── ServerHello ──  ephemeral_pub + static_pub(enc)
  static_pub(enc) + cert  ── ClientFinish ──►
                 ◄── sessão estabelecida ──
```

---

### `binary_node.rs`

Parser do formato binário proprietário do WhatsApp (não é Protobuf).

```
BinaryNode
  tag:     String
  attrs:   HashMap<String, String>
  content: NodeContent
             Nodes(Vec<BinaryNode>)
           | Bytes(Bytes)
           | Empty

decode(&[u8]) → Result<BinaryNode>
encode(&BinaryNode) → Vec<u8>

SINGLE_BYTE_TOKENS: [&str; 256]   ← copiar de Baileys/src/WABinary/token.ts
```

---

### `keys.rs` + `auth.rs`

```
KeyPair { public: [u8;32], private: [u8;32] }
  generate_keypair() → KeyPair
  generate_registration_id() → u32    14-bit random

AuthState   (Serialize + Deserialize)
  identity_key:        KeyPair
  registration_id:     u32
  signed_pre_key:      KeyPair
  signed_pre_key_sig:  [u8;64]
  one_time_pre_keys:   Vec<KeyPair>
  me:                  Option<MeInfo>

  AuthState::new() → Self
  save(path: &Path) → Result<()>
  load(path: &Path) → Result<Self>
```

---

### `qr.rs`

```
generate_qr_string(ref, noise_pub, identity_pub, adv_key) → String
  output: "{ref},{b64},{b64},{b64}"

print_qr_terminal(s: &str)
  crate `qrcode` → ASCII no stdout
```

---

### `signal/`

Wrapper sobre `libsignal-client`. Não reimplementar crypto — só os traits de store.

```
signal/store.rs
  InMemorySignalStore
    impl IdentityKeyStore
    impl PreKeyStore
    impl SignedPreKeyStore
    impl SessionStore

signal/session.rs
  init_session(jid, bundle, store) → Result<()>    X3DH
  encrypt(jid, &[u8], store) → Result<Vec<u8>>
  decrypt(jid, &[u8], store) → Result<Vec<u8>>
```

---

### `message.rs`

```
OutgoingMessage { to: String, content: MessageContent }

MessageContent
  Text(String)
  Image   { data: Bytes, caption: Option<String> }
  Video   { data: Bytes, caption: Option<String> }
  Reaction { key: MessageKey, emoji: String }

build_message_node(msg, auth) → BinaryNode
```

---

### `events.rs`

```
Event
  QrCode(String)
  Connected    { jid: String, name: String }
  Message(IncomingMessage)
  MessageAck   { id: String, status: AckStatus }
  Disconnected { reason: String }
  Error(anyhow::Error)

AckStatus: Sent | Delivered | Read | Played
```

---

### `socket.rs`

Loop principal. Roda em task Tokio dedicada.

```
run_loop(transport, noise, signal_store, auth, tx: Sender<Event>)
```

Recv:
```
frame → noise.decrypt → binary_node::decode → match tag
  "message"  → signal::decrypt → Event::Message
  "ack"      → Event::MessageAck
  "failure"  → Event::Disconnected
  _          → log e ignorar
```

Send:
```
MessageContent → build_message_node → binary_node::encode
  → signal::encrypt → noise.encrypt → transport.send_frame
```

---

### `client.rs`

```
Client
  connect(auth_path: Option<PathBuf>) → Result<(Self, Receiver<Event>)>
  send_text(jid, text) → Result<String>          retorna message_id
  send_image(jid, path, caption) → Result<String>
  send_reaction(jid, message_id, emoji) → Result<()>
  logout() → Result<()>
  run() → Result<()>                             spawnar em task separada
```

---

## Fluxo de Autenticação

```
App              Client                WA Server
 │  connect()      │                       │
 │────────────────►│  WS Upgrade           │
 │                 │──────────────────────►│
 │                 │  ClientHello          │
 │                 │──────────────────────►│
 │                 │         ServerHello   │
 │                 │◄──────────────────────│
 │                 │  ClientFinish         │
 │                 │──────────────────────►│
 │  QrCode(str)    │  <stream ref=...>     │
 │◄────────────────│◄──────────────────────│
 │  [scan]         │                       │
 │                 │         <success jid> │
 │                 │◄──────────────────────│
 │  Connected      │                       │
 │◄────────────────│                       │
```

---

## Estrutura de Arquivos

```
whatsapp-rs/
├── Cargo.toml
├── build.rs                    prost_build
├── proto/
│   ├── WAProto.proto
│   └── WAWeb.proto
├── src/
│   ├── lib.rs                  re-exports públicos
│   ├── client.rs
│   ├── socket.rs
│   ├── transport.rs
│   ├── noise.rs
│   ├── handshake.rs
│   ├── binary_node.rs
│   ├── message.rs
│   ├── events.rs
│   ├── auth.rs
│   ├── keys.rs
│   ├── qr.rs
│   ├── signal/
│   │   ├── mod.rs
│   │   ├── store.rs
│   │   └── session.rs
│   └── bin/
│       └── demo.rs
└── tests/
    ├── fixtures/               frames .bin capturados do WA real
    ├── transport_test.rs
    ├── noise_test.rs
    └── binary_node_test.rs
```

---

## DAG de Dependências

```
client
  └── socket
        ├── transport
        ├── noise
        ├── handshake ──► noise, transport
        ├── binary_node
        ├── message
        ├── events
        └── signal/session ──► signal/store

auth ──► keys
qr   ──► keys
```

---

## Performance

- `bytes::Bytes` no payload de mídia — zero-copy entre camadas.
- Tasks separadas: recv loop / send queue / keepalive. Comunicação via channels.
- Backpressure: `mpsc::channel(100)` no event sender. Caller lento gera pressão de volta.
- Signal sessions em memória com flush assíncrono a cada N mensagens.
- Reconexão: backoff exponencial 1s → 2s → 4s → ... → 30s (cap).

---

## Referências

| Recurso | Localização |
|---|---|
| Baileys | https://github.com/WhiskeySockets/Baileys |
| WAProto | `Baileys/src/WAProto/` |
| Binary tokens | `Baileys/src/WABinary/token.ts` |
| Noise spec | https://noiseprotocol.org/noise.html |
| Signal spec | https://signal.org/docs/ |
| libsignal-client | https://crates.io/crates/libsignal-client |
| WA protocol RE | https://github.com/sigalor/whatsapp-web-reveng |