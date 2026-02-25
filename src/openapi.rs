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
            "summary": "Send message",
            "tags": ["Message"],
            "parameters": [
              {
                "name": "operation",
                "in": "path",
                "required": true,
                "schema": { "type": "string", "enum": ["sendText"] },
                "description": "Message operation type"
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
                  "schema": { "$ref": "#/components/schemas/SendMessageRequest" }
                }
              }
            },
            "responses": {
              "200": {
                "description": "Message sent",
                "content": {
                  "application/json": {
                    "schema": { "$ref": "#/components/schemas/MessageResponse" }
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
                    "schema": { "$ref": "#/components/schemas/GroupInfo" }
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
                    "schema": {
                      "type": "array",
                      "items": { "$ref": "#/components/schemas/GroupInfo" }
                    }
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
              "requests_total": { "type": "integer", "example": 42 },
              "requests_2xx": { "type": "integer", "example": 38 },
              "requests_4xx": { "type": "integer", "example": 3 },
              "requests_5xx": { "type": "integer", "example": 1 },
              "instances_total": { "type": "integer", "example": 2 }
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
              "state": { "type": "string", "enum": ["disconnected", "connecting", "connected"], "example": "connected" }
            }
          },
          "ConnectResponse": {
            "type": "object",
            "properties": {
              "instance": { "type": "string", "example": "my-instance" },
              "state": { "type": "string", "example": "connecting" },
              "qr": { "type": "string", "nullable": true, "example": "2@abc123..." }
            }
          },
          "SendMessageRequest": {
            "type": "object",
            "required": ["to", "text"],
            "properties": {
              "to": { "type": "string", "example": "5511999999999@s.whatsapp.net" },
              "text": { "type": "string", "example": "Hello World" }
            }
          },
          "MessageResponse": {
            "type": "object",
            "properties": {
              "status": { "type": "string", "example": "sent" },
              "message_id": { "type": "string", "example": "msg-123" }
            }
          },
          "FindMessagesRequest": {
            "type": "object",
            "required": ["chat_id"],
            "properties": {
              "chat_id": { "type": "string", "example": "5511999999999@s.whatsapp.net" },
              "limit": { "type": "integer", "example": 50 }
            }
          },
          "FindMessagesResponse": {
            "type": "object",
            "properties": {
              "messages": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "id": { "type": "string" },
                    "from": { "type": "string" },
                    "text": { "type": "string" },
                    "timestamp": { "type": "integer" }
                  }
                }
              }
            }
          },
          "FindChatsResponse": {
            "type": "object",
            "properties": {
              "chats": {
                "type": "array",
                "items": {
                  "type": "object",
                  "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "last_message_time": { "type": "integer" }
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
              "created_at": { "type": "integer", "example": 1704067200 }
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

/// Returns Swagger UI HTML page bound to `/docs/openapi.json`.
pub fn swagger_ui() -> Html<&'static str> {
    Html(include_str!("swagger_ui.html"))
}
