use axum::response::Html;
use serde_json::{Value, json};

/// Returns the static OpenAPI 3.0 document for the current HTTP surface.
pub fn openapi_document() -> Value {
    json!({
      "openapi": "3.0.3",
      "info": {
        "title": "chatwarp-api",
        "version": "0.1.0",
        "description": "HTTP API para runtime Direct WA Client (escopo sintético M0-M10)."
      },
      "servers": [
        { "url": "http://localhost:8080" }
      ],
      "paths": {
        "/": {
          "get": {
            "summary": "Root endpoint",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "API status",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/RootResponse" }
                  }
                }
              }
            }
          }
        },
        "/swagger": {
          "get": {
            "summary": "Swagger UI",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "Swagger HTML page",
                "content": {
                  "text/html": {
                    "schema": { "type": "string" }
                  }
                }
              }
            }
          }
        },
        "/docs/swagger": {
          "get": {
            "summary": "Swagger UI (alias)",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "Swagger HTML page",
                "content": {
                  "text/html": {
                    "schema": { "type": "string" }
                  }
                }
              }
            }
          }
        },
        "/openapi.json": {
          "get": {
            "summary": "OpenAPI JSON document",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "OpenAPI document",
                "content": {
                  "application/json": {
                    "schema": { "type": "object" }
                  }
                }
              }
            }
          }
        },
        "/docs/openapi.json": {
          "get": {
            "summary": "OpenAPI JSON document (alias)",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "OpenAPI document",
                "content": {
                  "application/json": {
                    "schema": { "type": "object" }
                  }
                }
              }
            }
          }
        },
        "/healthz": {
          "get": {
            "summary": "Health probe",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "Service is healthy",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                  }
                }
              }
            }
          }
        },
        "/readyz": {
          "get": {
            "summary": "Readiness probe",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "Service is ready",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                  }
                }
              },
              "503": {
                "description": "Service not ready",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                  }
                }
              }
            }
          }
        },
        "/metrics": {
          "get": {
            "summary": "Request metrics snapshot",
            "tags": ["System"],
            "responses": {
              "200": {
                "description": "Current metrics",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/MetricsSnapshot" }
                  }
                }
              }
            }
          }
        },
        "/instance/create": {
          "post": {
            "summary": "Create new instance",
            "tags": ["Instance"],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateInstanceRequest" }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Instance created",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/InstanceOkResponse" }
                  }
                }
              },
              "400": {
                "description": "Invalid instance name",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "409": {
                "description": "Instance already exists",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/instance/delete/{name}": {
          "delete": {
            "summary": "Delete instance",
            "tags": ["Instance"],
            "parameters": [
              {
                "name": "name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "responses": {
              "200": {
                "description": "Instance deleted",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/InstanceOkResponse" }
                  }
                }
              },
              "400": {
                "description": "Invalid instance name",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/instance/connectionState/{name}": {
          "get": {
            "summary": "Get instance connection state",
            "tags": ["Instance"],
            "parameters": [
              {
                "name": "name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "responses": {
              "200": {
                "description": "Current connection state",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ConnectionStateResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/instance/connect/{name}": {
          "get": {
            "summary": "Connect instance and get QR code",
            "tags": ["Instance"],
            "parameters": [
              {
                "name": "name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "responses": {
              "200": {
                "description": "Connection initiated",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ConnectResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "409": {
                "description": "Already connected",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/message/{operation}/{instance_name}": {
          "post": {
            "summary": "Send message (sendText real; others 501)",
            "tags": ["Message"],
            "parameters": [
              {
                "name": "operation",
                "in": "path",
                "required": true,
                "description": "Message operation type",
                "schema": {
                  "type": "string",
                  "enum": [
                    "sendTemplate",
                    "sendText",
                    "sendMedia",
                    "sendPtv",
                    "sendWhatsAppAudio",
                    "sendStatus",
                    "sendSticker",
                    "sendLocation",
                    "sendContact",
                    "sendReaction",
                    "sendPoll",
                    "sendList",
                    "sendButtons"
                  ]
                }
              },
              {
                "name": "instance_name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/OutgoingMessage" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Message sent",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/MessagePostResponse" }
                  }
                }
              },
              "400": {
                "description": "Invalid operation or content",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "409": {
                "description": "Instance not connected",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "501": {
                "description": "Operation not implemented in this release",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "500": {
                "description": "Binary node encoding error",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "503": {
                "description": "Instance unavailable",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/chat/findMessages/{instance_name}": {
          "post": {
            "summary": "Find messages in chat",
            "tags": ["Chat"],
            "parameters": [
              {
                "name": "instance_name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/FindMessagesRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Messages found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/FindMessagesResponse" }
                  }
                }
              },
              "400": {
                "description": "Invalid request",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/chat/findChats/{instance_name}": {
          "get": {
            "summary": "List all chats",
            "tags": ["Chat"],
            "parameters": [
              {
                "name": "instance_name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "responses": {
              "200": {
                "description": "Chats list",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/FindChatsResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/group/create/{instance_name}": {
          "post": {
            "summary": "Create group",
            "tags": ["Group"],
            "parameters": [
              {
                "name": "instance_name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "requestBody": {
              "required": true,
              "content": {
                "application/json": {
                  "schema": { "$ref": "#/components/schemas/CreateGroupRequest" }
                }
              }
            },
            "responses": {
              "201": {
                "description": "Group created",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/GroupCreateResponse" }
                  }
                }
              },
              "400": {
                "description": "Invalid group payload",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        },
        "/group/fetchAllGroups/{instance_name}": {
          "get": {
            "summary": "List all groups",
            "tags": ["Group"],
            "parameters": [
              {
                "name": "instance_name",
                "in": "path",
                "required": true,
                "schema": { "type": "string" },
                "description": "Instance name"
              }
            ],
            "responses": {
              "200": {
                "description": "Groups list",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/GroupListResponse" }
                  }
                }
              },
              "404": {
                "description": "Instance not found",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorResponse" }
                  }
                }
              }
            }
          }
        }
      },
      "components": {
        "schemas": {
          "RootResponse": {
            "type": "object",
            "properties": {
              "name": { "type": "string", "example": "chatwarp-api" },
              "status": { "type": "string", "example": "ok" }
            }
          },
          "HealthResponse": {
            "type": "object",
            "properties": {
              "ok": { "type": "boolean", "example": true }
            }
          },
          "MetricsSnapshot": {
            "type": "object",
            "properties": {
              "uptime_seconds": { "type": "integer", "example": 42 },
              "instances_total": { "type": "integer", "example": 2 },
              "requests_total": { "type": "integer", "example": 100 },
              "inflight_requests": { "type": "integer", "example": 1 },
              "responses_2xx": { "type": "integer", "example": 95 },
              "responses_4xx": { "type": "integer", "example": 3 },
              "responses_5xx": { "type": "integer", "example": 2 },
              "responses_other": { "type": "integer", "example": 0 }
            }
          },
          "CreateInstanceRequest": {
            "type": "object",
            "required": ["name"],
            "properties": {
              "name": { "type": "string", "example": "my-instance" },
              "auto_connect": { "type": "boolean", "example": false }
            }
          },
          "InstanceOkResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "status": { "type": "string", "example": "created" }
            }
          },
          "ConnectionStateResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "state": {
                "type": "string",
                "enum": ["Connecting", "QrPending", "Connected", "Disconnected"],
                "example": "Connected"
              }
            }
          },
          "ConnectResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "state": {
                "type": "string",
                "enum": ["Connecting", "QrPending", "Connected", "Disconnected"],
                "example": "Connected"
              },
              "qr": { "type": "string", "nullable": true, "example": "2@ref,BASE64_NOISE_PUB,BASE64_IDENTITY_PUB,BASE64_ADV_KEY" }
            }
          },
          "OutgoingMessage": {
            "type": "object",
            "required": ["to", "content"],
            "properties": {
              "to": { "type": "string", "example": "5511999999999@s.whatsapp.net" },
              "content": {
                "type": "object",
                "description": "Payload tipado de mensagem. Deve combinar com :operation.",
                "additionalProperties": true,
                "example": { "type": "text", "text": "Hello World" }
              }
            }
          },
          "MessagePostResponse": {
            "type": "object",
            "properties": {
              "key": {
                "type": "object",
                "properties": {
                  "id": { "type": "string", "example": "msg-1772217584011-f3087c0c" }
                }
              }
            }
          },
          "FindMessagesRequest": {
            "type": "object",
            "required": ["remote_jid"],
            "properties": {
              "remote_jid": { "type": "string", "example": "5511999999999@s.whatsapp.net" },
              "limit": { "type": "integer", "minimum": 1, "maximum": 100, "example": 20 }
            }
          },
          "FindMessagesResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "remote_jid": { "type": "string", "example": "5511999999999@s.whatsapp.net" },
              "count": { "type": "integer", "example": 2 },
              "messages": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "id": { "type": "string", "example": "my-instance-0000" },
                    "from_me": { "type": "boolean", "example": true },
                    "body": { "type": "string", "example": "synthetic message #0 for 5511999999999@s.whatsapp.net" },
                    "timestamp": { "type": "integer", "example": 1772217584 }
                  }
                }
              }
            }
          },
          "FindChatsResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "chats": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "jid": { "type": "string", "example": "111@s.whatsapp.net" },
                    "name": { "type": "string", "example": "Synthetic Contact" },
                    "unread": { "type": "integer", "example": 0 }
                  }
                }
              }
            }
          },
          "CreateGroupRequest": {
            "type": "object",
            "required": ["subject", "participants"],
            "properties": {
              "subject": { "type": "string", "example": "My Group" },
              "participants": {
                "type": "array",
                "items": { "type": "string" },
                "example": ["5511999999999@s.whatsapp.net", "5511888888888@s.whatsapp.net"]
              }
            }
          },
          "GroupInfo": {
            "type": "object",
            "properties": {
              "id": { "type": "string", "example": "1-my-instance@g.us" },
              "subject": { "type": "string", "example": "My Group" },
              "participants": {
                "type": "array",
                "items": { "type": "string" },
                "example": ["5511999999999@s.whatsapp.net"]
              },
              "created_at": { "type": "integer", "example": 1772217584 }
            }
          },
          "GroupCreateResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "group": { "$ref": "#/components/schemas/GroupInfo" },
              "status": { "type": "string", "example": "created" }
            }
          },
          "GroupListResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "groups": {
                "type": "array",
                "items": { "$ref": "#/components/schemas/GroupInfo" }
              }
            }
          },
          "ApiErrorResponse": {
            "type": "object",
            "properties": {
              "error": { "type": "string", "example": "instance_not_found" },
              "message": { "type": "string", "example": "instance not found" }
            }
          }
        }
      }
    })
}

/// Returns Swagger UI HTML page bound to `/openapi.json`.
pub fn swagger_ui() -> Html<&'static str> {
    Html(include_str!("swagger_ui.html"))
}
