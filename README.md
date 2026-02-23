# chatwarp-api

Reescrita em Rust (Axum) do runtime principal da Evolution API v2, com integração de WhatsApp via sidecar gRPC.

## Estado atual
- Runtime HTTP/HTTPS funcional com fallback de HTTPS para HTTP.
- Configuração por `.env` com chaves compatíveis com Evolution API v2.
- PostgreSQL obrigatório no boot (com validação básica de schema).
- Sidecar gRPC obrigatório no boot (`SIDECAR_GRPC_ENDPOINT`).
- Métricas `/metrics`, websocket `/ws`, manager `/manager` e assets `/assets/*`.
- Frontend do manager versionado localmente em `manager/dist` (sem dependência externa).
- Parte das rotas já implementada; restante retorna `501 Not Implemented`.

## Requisitos
- Rust toolchain (stable) + Cargo
- PostgreSQL acessível
- Sidecar gRPC do WhatsApp/Baileys acessível

## Setup rápido
1. Ajuste variáveis no `.env` (já existe exemplo dev no repositório).
2. Gere/atualize código protobuf:
   - `cargo clean`
   - `cargo check`
3. Suba a API:
   - `cargo run`

Servidor padrão: `http://localhost:8080`

## Dependências via Docker Compose
Suba API + PostgreSQL + RabbitMQ:
- `docker compose up -d`

Suba também PgAdmin (opcional):
- `docker compose --profile tools up -d`

PgAdmin: `http://localhost:4000`  
RabbitMQ UI: `http://localhost:15672`

Observação: o sidecar gRPC de WhatsApp não está incluído nesse compose.

## Debug no VS Code
Existe `./.vscode/launch.json` com profile `Debug chatwarp-api` (CodeLLDB), usando `envFile=${workspaceFolder}/.env`.

## Endpoints principais
- `GET /` health de boas-vindas
- `POST /verify-creds`
- `GET /metrics`
- `GET /ws`
- `GET /manager`
- `GET /assets/*file`

Rotas de domínio:
- `/instance`
- `/message`
- `/call`
- `/chat`
- `/business`
- `/group`
- `/template`
- `/settings`
- `/proxy`
- `/label`
- integrações (`/webhook/*`, `/baileys/*`, `/s3/*`, etc.)

## Documentação detalhada
- `docs/DEVELOPMENT.md` - arquitetura, boot, debug, troubleshooting
- `docs/ENV.md` - variáveis de ambiente aceitas hoje
- `docs/ROUTES.md` - status de implementação de rotas
