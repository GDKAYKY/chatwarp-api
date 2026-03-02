# Migração para wa-rs (wacore)

> O **wa-rs** é um fork do [whatsapp-rust](https://github.com/jlucaso1/whatsapp-rust) com suporte a **Rust estável** (sem nightly). O ecossistema original **wacore** usa `portable_simd`, que exige nightly; o wa-rs substitui por implementações escalares.

---

## Status

| Item | Status |
|------|--------|
| Dependência `wa-rs` no Cargo.toml | ✅ |
| Compilação com Rust estável | ✅ |
| Integração no runner/instance | 🔲 Pendente |

---

## Estrutura do wa-rs

```
wa-rs/
├── wa-rs              # Cliente principal (Bot, handlers)
├── wa-rs-core         # Tipos e traits (events, store)
├── wa-rs-binary       # Protocolo binário WA (marshal/unmarshal)
├── wa-rs-libsignal    # Signal Protocol (E2E)
├── wa-rs-noise        # Noise Protocol (transport)
├── wa-rs-appstate     # App state sync
└── wa-rs-proto        # Protocol Buffers
```

---

## Próximos passos (migração incremental)

### Opção A — Usar Bot do wa-rs como runner

Substituir o `instance::runner` por um `wa_rs::bot::Bot` por instância. Exige:

1. **Backend customizado**: Implementar `wa_rs::traits::Backend` usando o `AuthStore` atual (Postgres).
2. **Transport**: Usar `wa-rs-tokio-transport` ou adaptar o `WsTransport` existente.
3. **Eventos**: Mapear `wa_rs::types::events::Event` para `crate::wa::events::Event`.

### Opção B — Substituir módulos internos

Migrar gradualmente:

1. **wa-rs-binary** em vez de `wa::binary_node` (formato real).
2. **wa-rs-libsignal** em vez de `wa::signal`.
3. **wa-rs-noise** em vez de `wa::noise` (se compatível).

O modo **Synthetic** (testes) permanece customizado; apenas o modo **RealMd** usaria wa-rs.

---

## Uso atual

A dependência está disponível. Exemplo de uso do Bot (quando integrado):

```rust
use std::sync::Arc;
use wa_rs::bot::Bot;
use wa_rs_sqlite_storage::SqliteStore;  // ou backend customizado
use wa_rs_tokio_transport::TokioWebSocketTransportFactory;
use wa_rs_ureq_http::UreqHttpClient;
use wa_rs_core::types::events::Event;

let backend = Arc::new(SqliteStore::new("whatsapp.db").await?);
let mut bot = Bot::builder()
    .with_backend(backend)
    .with_transport_factory(TokioWebSocketTransportFactory::new())
    .with_http_client(UreqHttpClient::new())
    .on_event(|event, _| async move {
        match event {
            Event::PairingQrCode { code, .. } => println!("QR:\n{}", code),
            Event::Message(msg, info) => println!("From {}: {:?}", info.source.sender, msg),
            _ => {}
        }
    })
    .build()
    .await?;
bot.run().await?;
```

---

## Referências

- [wa-rs no crates.io](https://crates.io/crates/wa-rs)
- [whatsapp-rust (original, requer nightly)](https://github.com/jlucaso1/whatsapp-rust)
- [wa-rs (fork stable)](https://github.com/homunbot/wa-rs)
