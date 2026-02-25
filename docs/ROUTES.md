# ROUTES.md

> Status de rotas HTTP no recorte M0-M10.

## Implementadas

```text
GET /         -> 200
GET /healthz  -> 200
GET /readyz   -> 503 (quando not ready) | 200 (quando ready)
POST /instance/create -> 201
DELETE /instance/delete/:name -> 200
GET /instance/connectionState/:name -> 200
GET /instance/connect/:name -> 200
POST /message/:operation/:instance_name -> 200
POST /chat/findMessages/:instance_name -> 200
GET /chat/findChats/:instance_name -> 200
POST /group/create/:instance_name -> 201
GET /group/fetchAllGroups/:instance_name -> 200
GET /metrics -> 200
```

Operações válidas em `:operation`:
- `sendTemplate`
- `sendText`
- `sendMedia`
- `sendPtv`
- `sendWhatsAppAudio`
- `sendStatus`
- `sendSticker`
- `sendLocation`
- `sendContact`
- `sendReaction`
- `sendPoll`
- `sendList`
- `sendButtons`

## Fallback padrão

Toda rota fora do escopo retorna:

```json
{ "error": "not_implemented", "route": "<path>" }
```

com status HTTP `501 Not Implemented`.

## Backlog de rotas

Fora deste recorte:
- `/call/*`
- `/settings/*`
- integrações (`/webhook/*`, `/websocket/*`, `/rabbitmq/*`, etc.)

Essas rotas seguem para milestones pós-M10.
