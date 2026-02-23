# Desenvolvimento

## Arquitetura
- `src/main.rs`: entrypoint async
- `src/bootstrap/`: ciclo de boot (env, tracing, sentry, db, sidecar, router, servidor)
- `src/http/`: router principal, middleware, handlers de root/metrics/static/ws
- `src/domain/`: rotas por domínio
- `src/repo/`: acesso PostgreSQL (`sqlx`)
- `src/sidecar/`: client gRPC (`tonic`) para integração WhatsApp
- `src/events/`: fanout de eventos (Webhook/WebSocket/RabbitMQ)
- `src/config/`: parsing de env + defaults
- `src/errors/`: envelope de erro padrão

## Sequência de boot
1. Carrega `.env`
2. Inicializa tracing
3. Instala panic hook
4. Carrega config
5. Inicializa Sentry (se `SENTRY_DSN`)
6. Conecta no PostgreSQL e valida schema mínimo
7. Conecta no sidecar gRPC e faz health check
8. Inicializa EventManager
9. Precarrega instâncias em memória
10. Sobe servidor HTTP/HTTPS (HTTPS com fallback)

## Comandos úteis
- `cargo check`
- `cargo test`
- `cargo run`

## Docker Compose (dev)
Arquivo: `docker-compose.yml`

Subir dependências:
- `docker compose up -d`

Subir com PgAdmin:
- `docker compose --profile tools up -d`

Serviços expostos:
- PostgreSQL: `localhost:5432`
- RabbitMQ: `localhost:5672`
- RabbitMQ UI: `localhost:15672`
- PgAdmin (profile `tools`): `localhost:4000`

Observação: o sidecar gRPC (`SIDECAR_GRPC_ENDPOINT`) não está no compose e deve rodar separado.

## Protobuf / tonic
Arquivo fonte:
- `proto/whatsapp_v2.proto`

A geração é feita em build-time por:
- `build.rs`
- módulo incluído em `src/proto/mod.rs`

Se mudar `.proto`:
1. `cargo clean`
2. `cargo check`

## Troubleshooting

### rust-analyzer mostra erro antigo
1. Salve arquivos
2. `Rust Analyzer: Restart Server`
3. rode `cargo check`

### erro de definição duplicada `connect` no arquivo gerado
Evite RPC com nome `Connect` em tonic client quando conflitar com método de conexão. No projeto já foi trocado para `ConnectInstance`.

### falha no boot por DB
`DATABASE_CONNECTION_URI` é obrigatório e precisa apontar para uma base com tabela `instance` ou `instances`.

### falha no boot por sidecar
`SIDECAR_GRPC_ENDPOINT` deve estar acessível no startup.

### 404 em rotas parametrizadas
Axum requer sintaxe de colon (`:param`) para path parameters, não curly braces (`{param}`).
Todas as rotas de domínio já usam `/:operation/:instance_name`.

### 503 Service Unavailable em /instance/connect
Indica que o sidecar gRPC está inacessível. Verifique:
1. Sidecar rodando em `SIDECAR_GRPC_ENDPOINT`
2. Conectividade de rede (ex: `host.docker.internal` em Docker)
3. Porta correta (padrão: 50051)
