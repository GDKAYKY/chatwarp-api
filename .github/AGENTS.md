# AGENTS.md

> Instruções para agentes de codificação (Codex, GitHub Copilot).
> Leia este arquivo antes de qualquer tarefa.

---

## O que é este projeto

**chatwarp-api** — API HTTP em Rust que implementa um cliente WhatsApp Web completo.
Não há sidecar. Não há processo externo. A lógica de protocolo (Noise, Signal, BinaryNode,
WebSocket) vive diretamente neste repositório.

Documentação de referência (ler antes de codar):

```
docs/PLANNING.md              milestones, tasks priorizadas
docs/ROUTES.md                status de todas as rotas HTTP
docs/PROJECT_ARCHITECTURE.md  arquitetura de módulos e dependências
docs/ENV.md                   variáveis de ambiente e defaults
```

---

## Stack

| Responsabilidade | Crate |
|---|---|
| Runtime async | `tokio` |
| HTTP | `axum` |
| WebSocket (WA) | `tokio-tungstenite` |
| Protobuf | `prost` + `prost-build` |
| Noise / Crypto | `x25519-dalek`, `aes-gcm`, `hkdf`, `sha2` |
| Signal E2E | `libsignal-client` |
| Banco | `sqlx` + PostgreSQL |
| Serialização | `serde`, `serde_json`, `bytes` |
| Erros | `thiserror` (libs) + `anyhow` (handlers) |
| Logging | `tracing` + `tracing-subscriber` |

---

## Regras Obrigatórias

### Código

- Sem `unwrap()` ou `expect()` em código de produção. Usar `?`.
- Sem `panic!()` fora de testes.
- Tipos de erro em módulos internos: `thiserror`. Em handlers: `anyhow`.
- Toda função pública com doc comment (`///`).
- `bytes::Bytes` para payloads binários. Não usar `Vec<u8>` no caminho crítico.
- Sem `clone()` desnecessário em tipos grandes — preferir referências ou `Arc`.

### Estrutura

- Handlers HTTP apenas: deserializar request → chamar service → serializar response.
- Services não importam tipos de Axum. Sem `axum::extract` fora de handlers.
- Protocolo WA (Noise, Signal, BinaryNode, Transport) vive em `src/wa/`.
- Camadas inferiores não importam camadas superiores. Ver DAG em `PROJECT_ARCHITECTURE.md`.

### Testes

- Toda função nova: pelo menos um teste unitário.
- `cargo test` deve passar antes de qualquer commit.
- Fixtures em `tests/fixtures/` — frames `.bin` capturados do WA real.
- Módulos de protocolo (`noise`, `binary_node`, `transport`) devem ter testes com fixtures reais.

### Git

- Um commit por task do `PLANNING.md`.
- Formato: `[T{id}] descrição curta` — ex: `[T1.1] add WsTransport connect`.
- Não misturar tasks de milestones diferentes no mesmo commit.
- Branch por milestone: `m1-transport`, `m2-noise`, `m3-binary-node`, etc.

---

## Workflow por Task

```
1. Ler a task em PLANNING.md
2. Verificar dependências no DAG (PROJECT_ARCHITECTURE.md)
3. Verificar se a rota está documentada em ROUTES.md (se for task de rota)
4. Implementar
5. Escrever testes
6. cargo clippy --all-targets -- -D warnings   (zero warnings)
7. cargo test
8. Commit
```

---

## Rotas

- Rotas `✅` em `ROUTES.md`: não alterar comportamento existente.
- Rotas `❌ 501`: retornar:
  ```json
  { "error": "not_implemented", "route": "<path>" }
  ```
- Novas rotas: adicionar em `ROUTES.md` antes de implementar.
- Parâmetros: sempre `:param`. Nunca `{param}`. Axum não falha em compile-time com sintaxe errada.

---

## Instâncias WA

Cada instância é uma sessão WhatsApp independente com:
- Conexão WebSocket própria (`src/wa/transport.rs`)
- Estado Noise próprio (`src/wa/noise.rs`)
- AuthState persistido no banco por `instance_name`
- Signal store próprio por instância

Não compartilhar estado entre instâncias. Sem globals mutáveis.

---

## MCP Servers Configurados

### Context7 (`@upstash/context7-mcp`)

Documentação atualizada de crates e frameworks.

Quando usar:
- Dúvida sobre API de `axum`, `sqlx`, `tokio`, `prost`, `tonic`
- Verificar versão atual de crate antes de adicionar no `Cargo.toml`

```
use context7 to find axum middleware documentation
use context7 to check current sqlx version and query macro usage
```

---

### `rust-analyzer-mcp` (`zeenix/rust-analyzer-mcp`)

Integração nativa com rust-analyzer via MCP.

Instalação:
```bash
cargo install rust-analyzer-mcp
rustup component add rust-analyzer
```

Configuração (`.mcp.json` na raiz):
```json
{
  "mcpServers": {
    "rust-analyzer": {
      "command": "rust-analyzer-mcp"
    }
  }
}
```

| Tool | Quando usar |
|---|---|
| `rust_analyzer_hover` | Verificar tipo antes de escrever código |
| `rust_analyzer_diagnostics` | Checar erros de um arquivo sem rodar `cargo check` |
| `rust_analyzer_workspace_diagnostics` | Validar workspace inteiro antes de commitar |
| `rust_analyzer_definition` | Navegar definição de trait sem perder contexto |
| `rust_analyzer_code_actions` | Quick fixes e refatorações disponíveis |
| `rust_analyzer_symbols` | Listar símbolos de um módulo |

> https://github.com/zeenix/rust-analyzer-mcp

---

### Sugestões Adicionais

#### `@modelcontextprotocol/server-github`
Acesso à API do GitHub.

Útil para:
- Ler arquivos do Baileys sem clonar (`WhiskeySockets/Baileys`)
- Buscar token dictionary: `Baileys/src/WABinary/token.ts`
- Ler `.proto` files: `Baileys/src/WAProto/`

#### `@modelcontextprotocol/server-postgres`
Conexão direta com o banco de desenvolvimento.

Útil para:
- Verificar schema antes de escrever queries SQLx
- Validar migrations
- Inspecionar AuthState persistido por instância

#### `@modelcontextprotocol/server-filesystem`
Acesso ao filesystem do projeto.

Útil para:
- Ler fixtures `tests/fixtures/*.bin`
- Inspecionar `.proto` files antes de gerar código

---

## O que NÃO fazer

- Não criar processo externo ou sidecar. Tudo roda in-process.
- Não adicionar crates sem justificativa. Comentar no `Cargo.toml`.
- Não duplicar configuração fora de `src/config.rs`.
- Não ignorar warnings do clippy. Zero warnings é requisito.
- Não implementar chatbot providers sem task explícita no `PLANNING.md`.
- Não alterar rotas marcadas como "Segurar" no `ROUTES.md`.
- Não compartilhar estado Noise ou Signal entre instâncias diferentes.