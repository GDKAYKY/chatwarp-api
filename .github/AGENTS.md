# AGENTS.md

> Instruções para agentes de codificação (Codex, GitHub Copilot, Claude Code).
> Leia este arquivo antes de qualquer tarefa. Ele define contexto, regras e workflow.

---

## Projeto

**chatwarp-api** — API HTTP em Rust que atua como proxy/gateway para um sidecar WhatsApp Web (gRPC).
Não é um cliente WhatsApp direto. A lógica de protocolo (Noise, Signal, BinaryNode) vive no sidecar.

Documentação de referência (ler antes de codar):

```
docs/PLANNING.md             tarefas priorizadas e milestones
docs/ROUTES.md               status de todas as rotas HTTP
docs/PROJECT_ARCHITECTURE.md arquitetura de módulos e dependências
docs/ENV.md                  variáveis de ambiente e defaults
```

---

## Stack

- **Linguagem:** Rust (edition 2021)
- **Runtime:** Tokio
- **HTTP:** Axum
- **Banco:** PostgreSQL via SQLx
- **gRPC:** Tonic (sidecar)
- **Serialização:** serde + serde_json
- **Erros:** thiserror (libs) + anyhow (bins/handlers)
- **Logging:** tracing + tracing-subscriber

---

## Regras Obrigatórias

### Código

- Sem `unwrap()` ou `expect()` em código de produção. Usar `?` e propagar erros.
- Sem `panic!()` fora de testes.
- Tipos de erro de biblioteca: `thiserror`. Handlers e bins: `anyhow`.
- Toda função pública precisa de doc comment (`///`).
- Sem `clone()` desnecessário em tipos grandes — preferir referências ou `Arc`.
- `bytes::Bytes` para payloads binários. Não usar `Vec<u8>` no caminho crítico.

### Estrutura

- Cada módulo tem responsabilidade única. Não adicionar lógica de negócio em handlers HTTP.
- Handlers apenas: deserializar request → chamar service → serializar response.
- Services não importam tipos de Axum (sem `axum::extract` fora de handlers).
- Camadas inferiores não importam camadas superiores. Ver DAG em `PROJECT_ARCHITECTURE.md`.

### Testes

- Toda função nova precisa de pelo menos um teste unitário.
- `cargo test` deve passar antes de qualquer commit.
- Fixtures de request/response em `tests/fixtures/`.
- Mocks de gRPC: usar `tonic::transport::Channel` mockado, não chamar sidecar real em testes.

### Git

- Um commit por task do `PLANNING.md`.
- Mensagem de commit: `[T{id}] descrição curta` — ex: `[T1.1] add WsTransport connect`.
- Não misturar tasks de milestones diferentes no mesmo commit.
- Branch por milestone: `m1-transport`, `m2-noise`, etc.

---

## Workflow por Task

```
1. Ler a task em PLANNING.md
2. Verificar dependências no DAG (PROJECT_ARCHITECTURE.md)
3. Verificar se a rota está documentada em ROUTES.md
4. Implementar
5. Escrever testes
6. cargo clippy --all-targets -- -D warnings    (zero warnings)
7. cargo test
8. Commit
```

---

## Rotas — Comportamento Esperado

- Rotas com status `✅` em `ROUTES.md`: não alterar comportamento existente.
- Rotas com status `❌ 501`: retornar `501 Not Implemented` com body:
  ```json
  { "error": "not_implemented", "route": "<path>" }
  ```
- Novas rotas: adicionar em `ROUTES.md` antes de implementar.
- Parâmetros de rota: sempre `:param` (sintaxe Axum). Nunca `{param}`.

---

## Variáveis de Ambiente

- Nunca hardcodar valores que estão em `ENV.md`.
- Defaults devem ser definidos em `src/config.rs` via `std::env::var(...).unwrap_or(...)`.
- `AUTHENTICATION_API_KEY` default (`BQYHJGJHJ`) é só para dev. Não usar em produção.
- Ver `docs/ENV.md` para lista completa.

---

## gRPC / Sidecar

- Endpoint: `SIDECAR_GRPC_ENDPOINT` (default: `http://127.0.0.1:50051`)
- Timeout de conexão: `SIDECAR_CONNECT_TIMEOUT_MS` (default: `3000`)
- Se sidecar estiver indisponível: retornar `503 Service Unavailable`, não `500`.
- Não fazer retry automático sem backoff. Ver lógica de reconexão em `PLANNING.md`.

---

## MCP Servers Configurados

### Context7 (`@upstash/context7-mcp`)

Usado para buscar documentação atualizada de crates e frameworks.

Quando usar:
- Dúvida sobre API de `axum`, `tonic`, `sqlx`, `tokio`, `prost`
- Verificar versão atual de uma crate antes de adicionar no `Cargo.toml`
- Buscar exemplos de uso oficiais

Como usar (Codex):
```
use context7 to find axum extractor documentation
use context7 to check current sqlx version and query macro usage
```

---

### Sugestões de MCP Adicionais

#### `@modelcontextprotocol/server-github`
Acesso direto ao repositório via API GitHub.

Útil para:
- Buscar implementações de referência no Baileys (`WhiskeySockets/Baileys`)
- Verificar issues abertas antes de implementar algo
- Ler arquivos específicos do repo sem clonar

```
use github mcp to read Baileys/src/WABinary/token.ts
use github mcp to search issues for "noise handshake"
```

#### `@modelcontextprotocol/server-postgres`
Conexão direta com o banco de desenvolvimento.

Útil para:
- Verificar schema atual antes de escrever queries SQLx
- Validar migrations
- Inspecionar dados de teste

```
use postgres mcp to describe table instances
use postgres mcp to check current schema version
```

#### `@modelcontextprotocol/server-filesystem`
Acesso ao filesystem do projeto.

Útil para:
- Ler fixtures de teste em `tests/fixtures/`
- Inspecionar `.proto` files antes de gerar código
- Verificar estrutura atual de pastas sem `tree`

#### `rust-analyzer-mcp` (`zeenix/rust-analyzer-mcp`)
Servidor MCP com integração nativa ao rust-analyzer. Escrito em Rust, roda via stdio.

Instalação:
```bash
cargo install rust-analyzer-mcp
rustup component add rust-analyzer   # dependência
```

Configuração (`.mcp.json` na raiz do projeto):
```json
{
  "mcpServers": {
    "rust-analyzer": {
      "command": "rust-analyzer-mcp"
    }
  }
}
```

Tools disponíveis:

| Tool | Uso |
|---|---|
| `rust_analyzer_symbols` | Lista símbolos de um arquivo (funções, structs, enums) |
| `rust_analyzer_definition` | Navega para definição de símbolo por posição |
| `rust_analyzer_references` | Encontra todas as referências de um símbolo |
| `rust_analyzer_hover` | Tipo e documentação de um símbolo |
| `rust_analyzer_diagnostics` | Erros e warnings de um arquivo específico |
| `rust_analyzer_workspace_diagnostics` | Diagnósticos de todo o workspace |
| `rust_analyzer_completion` | Sugestões de completion por posição |
| `rust_analyzer_code_actions` | Quick fixes e refatorações disponíveis |
| `rust_analyzer_format` | Formatar arquivo via rustfmt |

Útil para:
- Verificar tipos antes de escrever código (`hover`)
- Checar erros sem rodar `cargo check` (`diagnostics`)
- Navegar definições de trait sem sair do contexto (`definition`)
- Validar workspace inteiro antes de commitar (`workspace_diagnostics`)

> Repositório: https://github.com/zeenix/rust-analyzer-mcp

#### `@modelcontextprotocol/server-brave-search`
Busca na web com Brave Search.

Útil para:
- Buscar soluções para erros obscuros de compilação Rust
- Encontrar exemplos de uso de crates menos documentadas
- Pesquisar mudanças recentes no protocolo WhatsApp Web

---

## O que NÃO fazer

- Não implementar lógica de protocolo WhatsApp diretamente neste repo. Isso é responsabilidade do sidecar.
- Não adicionar dependências sem justificativa. Toda crate nova precisa de comentário no `Cargo.toml`.
- Não criar arquivos de configuração duplicados. Tudo em `src/config.rs`.
- Não ignorar warnings do clippy. Zero warnings é requisito.
- Não implementar providers de chatbot (evolutionBot, chatwoot, etc.) sem task explícita em `PLANNING.md`. Retornar 501.
- Não alterar rotas marcadas como "Segurar" em `ROUTES.md` sem discussão prévia.