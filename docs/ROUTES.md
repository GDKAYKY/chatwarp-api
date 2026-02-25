# ROUTES.md

> Status de rotas HTTP no recorte M0-M8.

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
- `/chat/*`
- `/group/*`
- `/settings/*`
- integrações (`/webhook/*`, `/websocket/*`, `/rabbitmq/*`, etc.)

Essas rotas serão liberadas a partir dos milestones M9+.
