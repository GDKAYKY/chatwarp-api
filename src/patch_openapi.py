#!/usr/bin/env python3
"""
patch_openapi.py
----------------
Aplica documentação real (request bodies, parâmetros, respostas) nas rotas
marcadas como implementadas (✅) no openapi.json.

Uso:
    python patch_openapi.py src/openapi.json          # edita in-place
    python patch_openapi.py src/openapi.json out.json  # grava em arquivo separado
"""

import json
import sys
from copy import deepcopy

# ---------------------------------------------------------------------------
# Schemas reutilizáveis que serão inseridos em components/schemas
# ---------------------------------------------------------------------------
NEW_SCHEMAS = {
    "WebhookConfig": {
        "type": "object",
        "properties": {
            "url":            {"type": "string", "format": "uri"},
            "enabled":        {"type": "boolean", "default": True},
            "webhookByEvents":{"type": "boolean", "default": False},
            "webhookBase64":  {"type": "boolean", "default": False},
            "headers":        {"type": "object", "additionalProperties": {"type": "string"}},
            "events":         {"type": "array", "items": {"type": "string"}}
        }
    },
    "CreateSessionRequest": {
        "type": "object",
        "properties": {
            "session":      {"type": "string", "example": "default"},
            "phone_number": {"type": "string", "example": "5511999999999"},
            "webhook":      {"$ref": "#/components/schemas/WebhookConfig"}
        }
    },
    "SessionResponse": {
        "type": "object",
        "properties": {
            "session":           {"type": "string"},
            "status":            {"type": "string"},
            "webhook_url":       {"type": "string", "nullable": True},
            "webhook_enabled":   {"type": "boolean"},
            "webhook_by_events": {"type": "boolean"},
            "webhook_base64":    {"type": "boolean"},
            "phone_number":      {"type": "string", "nullable": True},
            "created_at":        {"type": "string", "format": "date-time"},
            "updated_at":        {"type": "string", "format": "date-time"},
            "runtime": {
                "type": "object",
                "properties": {
                    "connection_state": {"type": "string"},
                    "qr_code":          {"type": "string", "nullable": True},
                    "pair_code":        {"type": "string", "nullable": True},
                    "last_seen":        {"type": "string", "format": "date-time", "nullable": True}
                }
            }
        }
    },
    "SendMessageRequest": {
        "type": "object",
        "required": ["session", "chatId"],
        "properties": {
            "session":  {"type": "string", "example": "default"},
            "chatId":   {"type": "string", "example": "5511999999999@s.whatsapp.net"},
            "text":     {"type": "string"},
            "caption":  {"type": "string"},
            "url":      {"type": "string", "format": "uri", "description": "URL da mídia (image/file/voice/video)"},
            "base64":   {"type": "string", "description": "Conteúdo em base64 (alternativa a url)"},
            "filename": {"type": "string"},
            "mimetype": {"type": "string"},
            "quoted": {
                "type": "object",
                "properties": {
                    "messageId": {"type": "string"},
                    "chatId":    {"type": "string"}
                }
            }
        }
    },
    "SendMessageResponse": {
        "type": "object",
        "properties": {
            "id":       {"type": "string", "format": "uuid"},
            "session":  {"type": "string"},
            "chatId":   {"type": "string"},
            "type":     {"type": "string"},
            "status":   {"type": "string", "example": "queued"},
            "payload":  {"type": "object"},
            "created_at": {"type": "string", "format": "date-time"}
        }
    },
    "MessageIdRequest": {
        "type": "object",
        "required": ["messageId"],
        "properties": {
            "session":   {"type": "string"},
            "messageId": {"type": "string", "format": "uuid"}
        }
    },
    "TypingRequest": {
        "type": "object",
        "required": ["session", "chatId"],
        "properties": {
            "session": {"type": "string"},
            "chatId":  {"type": "string"}
        }
    },
    "ProfileNameRequest": {
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": {"type": "string", "example": "Meu Nome"}
        }
    },
    "ProfileStatusRequest": {
        "type": "object",
        "required": ["status"],
        "properties": {
            "status": {"type": "string", "example": "Disponível"}
        }
    },
    "ProfilePictureRequest": {
        "type": "object",
        "required": ["picture"],
        "properties": {
            "picture": {"type": "string", "description": "URL ou base64 da imagem"}
        }
    },
    "QrResponse": {
        "type": "object",
        "properties": {
            "session": {"type": "string"},
            "qr": {"type": "string", "description": "QR code em base64 PNG"}
        }
    },
    "RequestCodeResponse": {
        "type": "object",
        "properties": {
            "session": {"type": "string"},
            "code": {"type": "string"}
        }
    },
    "PresenceRequest": {
        "type": "object",
        "required": ["presence"],
        "properties": {
            "presence": {"type": "string", "enum": ["available", "unavailable", "composing", "recording", "paused"]}
        }
    },
    "PresenceResponse": {
        "type": "object",
        "properties": {
            "chatId":    {"type": "string"},
            "presence":  {"type": "string"},
            "updated_at":{"type": "string", "format": "date-time"}
        }
    },
    "LabelRequest": {
        "type": "object",
        "required": ["name"],
        "properties": {
            "name":  {"type": "string"},
            "color": {"type": "integer"}
        }
    },
    "LabelResponse": {
        "type": "object",
        "properties": {
            "id":    {"type": "string"},
            "name":  {"type": "string"},
            "color": {"type": "integer"}
        }
    },
    "MediaConvertRequest": {
        "type": "object",
        "required": ["url"],
        "properties": {
            "url":    {"type": "string", "format": "uri"},
            "base64": {"type": "string"}
        }
    },
    "MediaConvertResponse": {
        "type": "object",
        "properties": {
            "url":      {"type": "string"},
            "base64":   {"type": "string"},
            "mimetype": {"type": "string"}
        }
    },
    "ApiKeyRequest": {
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": {"type": "string"}
        }
    },
    "ApiKeyResponse": {
        "type": "object",
        "properties": {
            "id":         {"type": "string", "format": "uuid"},
            "name":       {"type": "string"},
            "key":        {"type": "string"},
            "created_at": {"type": "string", "format": "date-time"}
        }
    },
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
def ok(schema_ref: str, description: str = "Success") -> dict:
    return {
        "200": {
            "description": description,
            "content": {"application/json": {"schema": {"$ref": f"#/components/schemas/{schema_ref}"}}}
        }
    }

def ok_array(schema_ref: str, description: str = "Success") -> dict:
    return {
        "200": {
            "description": description,
            "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": f"#/components/schemas/{schema_ref}"}}}}
        }
    }

def ok_inline(props: dict, description: str = "Success") -> dict:
    return {
        "200": {
            "description": description,
            "content": {"application/json": {"schema": {"type": "object", "properties": props}}}
        }
    }

def created(schema_ref: str) -> dict:
    return {
        "201": {
            "description": "Created",
            "content": {"application/json": {"schema": {"$ref": f"#/components/schemas/{schema_ref}"}}}
        }
    }

def body(schema_ref: str, required: bool = True) -> dict:
    return {
        "required": required,
        "content": {"application/json": {"schema": {"$ref": f"#/components/schemas/{schema_ref}"}}}
    }

def err_response() -> dict:
    return {"$ref": "#/components/schemas/ErrorResponse"}

def with_errors(responses: dict) -> dict:
    r = deepcopy(responses)
    r.setdefault("400", {"description": "Bad Request",           "content": {"application/json": {"schema": err_response()}}})
    r.setdefault("500", {"description": "Internal Server Error", "content": {"application/json": {"schema": err_response()}}})
    return r

# ---------------------------------------------------------------------------
# Patches por path+method  (apenas rotas ✅)
# ---------------------------------------------------------------------------
PATCHES = {
    # ── Sessions ────────────────────────────────────────────────────────────
    "/sessions": {
        "get": {
            "summary": "Listar todas as sessões",
            "operationId": "listSessions",
            "responses": with_errors(ok_array("SessionResponse", "Lista de sessões"))
        },
        "post": {
            "summary": "Criar ou upsert de sessão",
            "operationId": "createSession",
            "requestBody": body("CreateSessionRequest"),
            "responses": with_errors(created("SessionResponse"))
        }
    },
    "/sessions/{session}": {
        "get": {
            "summary": "Obter sessão por nome",
            "operationId": "getSession",
            "responses": with_errors({
                "200": {"description": "Sessão encontrada", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SessionResponse"}}}},
                "404": {"description": "Sessão não encontrada", "content": {"application/json": {"schema": err_response()}}}
            })
        },
        "delete": {
            "summary": "Deletar sessão",
            "operationId": "deleteSession",
            "responses": with_errors(ok_inline({"session": {"type": "string"}, "status": {"type": "string"}}))
        }
    },
    "/sessions/{session}/start": {
        "post": {
            "summary": "Iniciar sessão",
            "operationId": "startSession",
            "responses": with_errors(ok_inline({"session": {"type": "string"}, "status": {"type": "string"}}))
        }
    },
    "/sessions/{session}/stop": {
        "post": {
            "summary": "Parar sessão",
            "operationId": "stopSession",
            "responses": with_errors(ok_inline({"session": {"type": "string"}, "status": {"type": "string"}}))
        }
    },

    # ── Auth ───────────────────────────────────────────────────────────────
    "/{session}/auth/qr": {
        "get": {
            "summary": "Obter QR code da sessão",
            "operationId": "getQr",
            "responses": with_errors({
                "200": {
                    "description": "QR code disponível",
                    "content": {"application/json": {"schema": {"$ref": "#/components/schemas/QrResponse"}}}
                },
                "404": {
                    "description": "QR code indisponível",
                    "content": {"application/json": {"schema": err_response()}}
                }
            })
        }
    },
    "/{session}/auth/request-code": {
        "post": {
            "summary": "Gerar código de pareamento da sessão",
            "operationId": "requestCode",
            "responses": with_errors(ok("RequestCodeResponse"))
        }
    },

    # ── Profile ──────────────────────────────────────────────────────────────
    "/{session}/profile": {
        "get": {
            "summary": "Obter perfil da sessão",
            "operationId": "getProfile",
            "responses": with_errors(ok_inline({
                "name":   {"type": "string"},
                "status": {"type": "string"},
                "jid":    {"type": "string"}
            }))
        }
    },
    "/{session}/profile/name": {
        "put": {
            "summary": "Atualizar nome do perfil",
            "operationId": "updateProfileName",
            "requestBody": body("ProfileNameRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/{session}/profile/status": {
        "put": {
            "summary": "Atualizar status/bio do perfil",
            "operationId": "updateProfileStatus",
            "requestBody": body("ProfileStatusRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/{session}/profile/picture": {
        "put": {
            "summary": "Atualizar foto do perfil",
            "operationId": "updateProfilePicture",
            "requestBody": body("ProfilePictureRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },

    # ── Chat Manager ─────────────────────────────────────────────────────────
    "/sendText": {
        "post": {
            "summary": "Enviar mensagem de texto",
            "operationId": "sendText",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendImage": {
        "post": {
            "summary": "Enviar imagem",
            "operationId": "sendImage",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendFile": {
        "post": {
            "summary": "Enviar arquivo",
            "operationId": "sendFile",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendVoice": {
        "post": {
            "summary": "Enviar áudio/voz",
            "operationId": "sendVoice",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendVideo": {
        "post": {
            "summary": "Enviar vídeo",
            "operationId": "sendVideo",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/send/link-custom-preview": {
        "post": {
            "summary": "Enviar link com preview customizado",
            "operationId": "sendLinkCustomPreview",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendButtons": {
        "post": {
            "summary": "Enviar mensagem com botões",
            "operationId": "sendButtons",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendList": {
        "post": {
            "summary": "Enviar mensagem com lista",
            "operationId": "sendList",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/forwardMessage": {
        "post": {
            "summary": "Encaminhar mensagem",
            "operationId": "forwardMessage",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/reply": {
        "post": {
            "summary": "Responder mensagem",
            "operationId": "replyMessage",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendSeen": {
        "post": {
            "summary": "Marcar mensagem como vista",
            "operationId": "sendSeen",
            "requestBody": body("MessageIdRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}, "id": {"type": "string"}}))
        }
    },
    "/startTyping": {
        "post": {
            "summary": "Iniciar indicador de digitação",
            "operationId": "startTyping",
            "requestBody": body("TypingRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/stopTyping": {
        "post": {
            "summary": "Parar indicador de digitação",
            "operationId": "stopTyping",
            "requestBody": body("TypingRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/reaction": {
        "put": {
            "summary": "Adicionar/remover reação a mensagem",
            "operationId": "setReaction",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "required": ["messageId", "reaction"],
                    "properties": {
                        "session":   {"type": "string"},
                        "messageId": {"type": "string"},
                        "reaction":  {"type": "string", "example": "👍"}
                    }
                }}}
            },
            "responses": with_errors(ok_inline({"status": {"type": "string"}, "id": {"type": "string"}}))
        }
    },
    "/star": {
        "put": {
            "summary": "Favoritar/desfavoritar mensagem",
            "operationId": "starMessage",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "required": ["messageId"],
                    "properties": {
                        "session":   {"type": "string"},
                        "messageId": {"type": "string"},
                        "starred":   {"type": "boolean"}
                    }
                }}}
            },
            "responses": with_errors(ok_inline({"status": {"type": "string"}, "id": {"type": "string"}}))
        }
    },
    "/sendPoll": {
        "post": {
            "summary": "Enviar enquete",
            "operationId": "sendPoll",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendPollVote": {
        "post": {
            "summary": "Votar em enquete",
            "operationId": "sendPollVote",
            "requestBody": body("SendMessageRequest"),
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendLocation": {
        "post": {
            "summary": "Enviar localização",
            "operationId": "sendLocation",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "required": ["session", "chatId", "latitude", "longitude"],
                    "properties": {
                        "session":   {"type": "string"},
                        "chatId":    {"type": "string"},
                        "latitude":  {"type": "number"},
                        "longitude": {"type": "number"},
                        "name":      {"type": "string"},
                        "address":   {"type": "string"}
                    }
                }}}
            },
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/sendContactVcard": {
        "post": {
            "summary": "Enviar contato como vCard",
            "operationId": "sendContactVcard",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "required": ["session", "chatId", "vcard"],
                    "properties": {
                        "session": {"type": "string"},
                        "chatId":  {"type": "string"},
                        "vcard":   {"type": "string", "description": "vCard em formato texto"}
                    }
                }}}
            },
            "responses": with_errors(ok("SendMessageResponse"))
        }
    },
    "/messages": {
        "get": {
            "summary": "Listar mensagens",
            "operationId": "listMessages",
            "parameters": [
                {"name": "session", "in": "query", "schema": {"type": "string"}, "description": "Nome da sessão"},
                {"name": "chatId",  "in": "query", "schema": {"type": "string"}, "description": "Filtrar por chat"},
                {"name": "limit",   "in": "query", "schema": {"type": "integer", "default": 50}},
                {"name": "offset",  "in": "query", "schema": {"type": "integer", "default": 0}}
            ],
            "responses": with_errors(ok_array("SendMessageResponse"))
        }
    },

    # ── Presence ─────────────────────────────────────────────────────────────
    "/{session}/presence": {
        "post": {
            "summary": "Definir presença da sessão",
            "operationId": "setPresence",
            "requestBody": body("PresenceRequest"),
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/{session}/presence/{chatId}": {
        "get": {
            "summary": "Obter presença de um chat",
            "operationId": "getChatPresence",
            "responses": with_errors(ok("PresenceResponse"))
        }
    },
    "/{session}/presence/{chatId}/subscribe": {
        "post": {
            "summary": "Assinar presença de um chat",
            "operationId": "subscribePresence",
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },

    # ── Labels ───────────────────────────────────────────────────────────────
    "/{session}/labels": {
        "get": {
            "summary": "Listar labels",
            "operationId": "listLabels",
            "responses": with_errors(ok_array("LabelResponse"))
        },
        "post": {
            "summary": "Criar label",
            "operationId": "createLabel",
            "requestBody": body("LabelRequest"),
            "responses": with_errors(created("LabelResponse"))
        }
    },
    "/{session}/labels/chats/{chatId}": {
        "put": {
            "summary": "Definir labels de um chat",
            "operationId": "setChatLabels",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "properties": {
                        "labels": {"type": "array", "items": {"type": "string"}}
                    }
                }}}
            },
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/{session}/labels/{labelId}/chats": {
        "get": {
            "summary": "Listar chats de uma label",
            "operationId": "getLabelChats",
            "responses": with_errors(ok_array("SendMessageResponse", "Chats com essa label"))
        }
    },

    # ── Media ─────────────────────────────────────────────────────────────────
    "/{session}/media/convert/voice": {
        "post": {
            "summary": "Converter áudio para formato de voz do WhatsApp (ogg/opus)",
            "operationId": "convertVoice",
            "requestBody": body("MediaConvertRequest"),
            "responses": with_errors(ok("MediaConvertResponse"))
        }
    },
    "/{session}/media/convert/video": {
        "post": {
            "summary": "Converter vídeo para formato compatível",
            "operationId": "convertVideo",
            "requestBody": body("MediaConvertRequest"),
            "responses": with_errors(ok("MediaConvertResponse"))
        }
    },

    # ── Channels ──────────────────────────────────────────────────────────────
    "/{session}/channels": {
        "get": {
            "summary": "Listar canais",
            "operationId": "listChannels",
            "responses": with_errors(ok_array("SessionResponse", "Lista de canais"))
        }
    },
    "/{session}/channels/{id}/follow": {
        "post": {
            "summary": "Seguir canal",
            "operationId": "followChannel",
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },
    "/{session}/channels/search/by-text": {
        "post": {
            "summary": "Buscar canais por texto",
            "operationId": "searchChannelsByText",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": {"type": "string"}
                    }
                }}}
            },
            "responses": with_errors(ok_array("SessionResponse", "Canais encontrados"))
        }
    },

    # ── ApiKeys ───────────────────────────────────────────────────────────────
    "/keys": {
        "get": {
            "summary": "Listar API keys",
            "operationId": "listApiKeys",
            "responses": with_errors(ok_array("ApiKeyResponse"))
        },
        "post": {
            "summary": "Criar API key",
            "operationId": "createApiKey",
            "requestBody": body("ApiKeyRequest"),
            "responses": with_errors(created("ApiKeyResponse"))
        }
    },
    "/keys/{id}": {
        "delete": {
            "summary": "Revogar API key",
            "operationId": "revokeApiKey",
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },

    # ── Calls ─────────────────────────────────────────────────────────────────
    "/{session}/calls/reject": {
        "post": {
            "summary": "Rejeitar chamada",
            "operationId": "rejectCall",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "properties": {
                        "callId": {"type": "string"}
                    }
                }}}
            },
            "responses": with_errors(ok_inline({"status": {"type": "string"}}))
        }
    },

    # ── Events ────────────────────────────────────────────────────────────────
    "/{session}/events": {
        "get": {
            "summary": "Listar eventos da sessão",
            "operationId": "getEvents",
            "parameters": [
                {
                    "name": "type",
                    "in": "query",
                    "required": False,
                    "schema": {"type": "string"},
                    "description": "Filtra por tipo de evento (ex: CHAT_PRESENCE)"
                },
                {
                    "name": "limit",
                    "in": "query",
                    "required": False,
                    "schema": {"type": "integer", "default": 50}
                },
                {
                    "name": "offset",
                    "in": "query",
                    "required": False,
                    "schema": {"type": "integer", "default": 0}
                }
            ],
            "responses": with_errors(ok_inline({
                "events": {"type": "array", "items": {"type": "object"}}
            }))
        },
        "post": {
            "summary": "Registrar evento na sessão",
            "operationId": "postEvent",
            "requestBody": {
                "required": False,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "properties": {
                        "event": {"type": "string"},
                        "payload": {"type": "object"}
                    }
                }}}
            },
            "responses": with_errors(ok_inline({
                "status": {"type": "string"}
            }))
        }
    },

    # ── Contacts ────────────────────────────────────────────────────────────
    "/contacts": {
        "get": {
            "summary": "Listar contatos (opcional por sessão)",
            "operationId": "listContacts",
            "parameters": [
                {
                    "name": "session",
                    "in": "query",
                    "required": False,
                    "schema": {"type": "string"}
                }
            ],
            "responses": with_errors(ok_inline({
                "contacts": {"type": "array", "items": {"type": "object"}}
            }))
        }
    },
    "/contacts/all": {
        "get": {
            "summary": "Listar todos os contatos",
            "operationId": "listContactsAll",
            "responses": with_errors(ok_inline({
                "contacts": {"type": "array", "items": {"type": "object"}}
            }))
        }
    },
    "/contacts/profile-picture": {
        "get": {
            "summary": "Buscar foto de perfil de contato",
            "operationId": "getContactProfilePicture",
            "parameters": [
                {
                    "name": "session",
                    "in": "query",
                    "required": False,
                    "schema": {"type": "string"}
                },
                {
                    "name": "id",
                    "in": "query",
                    "required": True,
                    "schema": {"type": "string"},
                    "description": "JID ou número do contato"
                }
            ],
            "responses": with_errors(ok_inline({
                "url": {"type": "string"},
                "id": {"type": "string"},
                "status": {"type": "string"}
            }))
        }
    },

    # ── Groups ──────────────────────────────────────────────────────────────
    "/{session}/groups": {
        "get": {
            "summary": "Listar grupos da sessão",
            "operationId": "listGroups",
            "responses": with_errors(ok_inline({
                "groups": {"type": "array", "items": {"type": "object"}}
            }))
        }
    },
    "/{session}/groups/{id}": {
        "get": {
            "summary": "Obter grupo por ID",
            "operationId": "getGroup",
            "responses": with_errors(ok_inline({
                "group": {"type": "object"}
            }))
        }
    },

    # ── Apps ──────────────────────────────────────────────────────────────────
    "/apps": {
        "get": {
            "summary": "Listar apps",
            "operationId": "listApps",
            "responses": with_errors(ok_array("SessionResponse", "Lista de apps"))
        },
        "post": {
            "summary": "Criar app",
            "operationId": "createApp",
            "requestBody": {
                "required": True,
                "content": {"application/json": {"schema": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string"}
                    }
                }}}
            },
            "responses": with_errors(created("SessionResponse"))
        }
    },

    # ── Observability ─────────────────────────────────────────────────────────
    "/ping": {
        "get": {
            "summary": "Ping",
            "operationId": "ping",
            "responses": {
                "200": {"description": "pong", "content": {"application/json": {"schema": {
                    "type": "object",
                    "properties": {"pong": {"type": "boolean"}}
                }}}}
            }
        }
    },
    "/health": {
        "get": {
            "summary": "Health check detalhado",
            "operationId": "health",
            "responses": {
                "200": {"description": "Serviço saudável", "content": {"application/json": {"schema": {
                    "type": "object",
                    "properties": {
                        "status":  {"type": "string"},
                        "version": {"type": "string"}
                    }
                }}}}
            }
        }
    },
    "/server/status": {
        "get": {
            "summary": "Status do servidor",
            "operationId": "serverStatus",
            "responses": with_errors(ok_inline({
                "uptime":   {"type": "number"},
                "sessions": {"type": "integer"},
                "status":   {"type": "string"}
            }))
        }
    },
}


# ---------------------------------------------------------------------------
# Aplicar patches
# ---------------------------------------------------------------------------
def apply_patches(spec: dict) -> tuple[dict, list[str], list[str]]:
    spec.setdefault("components", {}).setdefault("schemas", {})
    spec["components"]["schemas"].update(NEW_SCHEMAS)

    patched = []
    not_found = []

    paths = spec.get("paths", {})
    for path, methods in PATCHES.items():
        if path not in paths:
            paths[path] = {}
        for method, operation in methods.items():
            if method not in paths[path]:
                paths[path][method] = operation
                patched.append(f"{method.upper()} {path}")
                continue
            # Preserva tags e parâmetros existentes, substitui o resto
            existing = paths[path][method]
            merged = {}
            if "tags" in existing:
                merged["tags"] = existing["tags"]
            merged.update(operation)
            paths[path][method] = merged
            patched.append(f"{method.upper()} {path}")

    return spec, patched, not_found


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
def main():
    if len(sys.argv) < 2:
        print("Uso: python patch_openapi.py <input.json> [output.json]")
        sys.exit(1)

    input_path = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) > 2 else input_path

    with open(input_path, "r", encoding="utf-8") as f:
        spec = json.load(f)

    spec, patched, not_found = apply_patches(spec)

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(spec, f, ensure_ascii=False, indent=2)

    print(f"\n✅ {len(patched)} operações documentadas:")
    for p in sorted(patched):
        print(f"   {p}")

    if not_found:
        print(f"\n⚠️  {len(not_found)} paths/métodos não encontrados no JSON (verifique se existem):")
        for p in not_found:
            print(f"   {p}")

    print(f"\n📦 {len(NEW_SCHEMAS)} schemas adicionados em components/schemas")
    print(f"💾 Salvo em: {output_path}")


if __name__ == "__main__":
    main()
