# Status das Rotas

## Implementadas (funcionais)

### Core
- `GET /`
- `POST /verify-creds`
- `GET /metrics`
- `GET /ws`
- `GET /manager`
- `GET /manager/{*path}`
- `GET /assets/*file`

### Instance (`/instance`)
- `POST /create`
- `POST /restart/:instance_name`
- `GET /connect/:instance_name`
- `GET /connectionState/:instance_name`
- `GET /fetchInstances`
- `POST /setPresence/:instance_name`
- `DELETE /logout/:instance_name`
- `DELETE /delete/:instance_name`

### Message (`/message`)
- `POST /:operation/:instance_name` com operações permitidas:
  - `sendTemplate`, `sendText`, `sendMedia`, `sendPtv`, `sendWhatsAppAudio`, `sendStatus`, `sendSticker`, `sendLocation`, `sendContact`, `sendReaction`, `sendPoll`, `sendList`, `sendButtons`

### Call (`/call`)
- `POST /offer/:instance_name`

### Channel / Event (parcial)
- `POST /webhook/evolution`
- `GET /webhook/meta`
- `POST /webhook/meta`
- `POST /baileys/:operation/:instance_name`
- `POST /webhook/set/:instance_name`
- `GET /webhook/find/:instance_name`
- `POST /websocket/set/:instance_name`
- `GET /websocket/find/:instance_name`
- `POST /rabbitmq/set/:instance_name`
- `GET /rabbitmq/find/:instance_name`

## Mapeadas, mas ainda retornam 501 (Not Implemented)
- `/chat/*`
- `/business/*`
- `/group/*`
- `/template/*`
- `/settings/*`
- `/proxy/*`
- `/label/*`
- `/chatbot/*` (`evolutionBot`, `chatwoot`, `typebot`, `openai`, `dify`, `flowise`, `n8n`, `evoai`)
- `/s3/*`
- Providers de evento fora do escopo R1 (nats/sqs/pusher/kafka)

## Observações
- Guard de autenticação por `apikey` aplicado nas rotas protegidas.
- A forma do payload segue compatibilidade semântica com Evolution v2 (não byte a byte).
- Rotas parametrizadas usam sintaxe Axum com colon (`:param`), não curly braces.
- Rotas 501 estão mapeadas e validam operações permitidas, mas delegação ao sidecar ainda não implementada.
