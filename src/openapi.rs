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
            "summary": "Root",
            "responses": { "200": { "description": "OK" } }
          }
        },
        "/healthz": {
          "get": {
            "summary": "Health probe",
            "responses": { "200": { "description": "Healthy" } }
          }
        },
        "/readyz": {
          "get": {
            "summary": "Readiness probe",
            "responses": {
              "200": { "description": "Ready" },
              "503": { "description": "Not ready" }
            }
          }
        },
        "/metrics": {
          "get": {
            "summary": "Request metrics snapshot",
            "responses": { "200": { "description": "Metrics payload" } }
          }
        },
        "/instance/create": {
          "post": {
            "summary": "Create instance",
            "requestBody": { "required": true },
            "responses": {
              "201": { "description": "Created" },
              "409": { "description": "Already exists" }
            }
          }
        },
        "/instance/delete/{name}": {
          "delete": {
            "summary": "Delete instance",
            "parameters": [
              { "name": "name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": { "description": "Deleted" },
              "404": { "description": "Not found" }
            }
          }
        },
        "/instance/connectionState/{name}": {
          "get": {
            "summary": "Get instance connection state",
            "parameters": [
              { "name": "name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": { "description": "Current state" },
              "404": { "description": "Not found" }
            }
          }
        },
        "/instance/connect/{name}": {
          "get": {
            "summary": "Start/connect instance",
            "parameters": [
              { "name": "name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": { "description": "Connect flow result" },
              "404": { "description": "Not found" },
              "409": { "description": "Already connected" }
            }
          }
        },
        "/message/{operation}/{instance_name}": {
          "post": {
            "summary": "Send outbound message",
            "parameters": [
              { "name": "operation", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "instance_name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": { "required": true },
            "responses": {
              "200": { "description": "Message accepted" },
              "400": { "description": "Invalid operation/content" },
              "404": { "description": "Instance not found" }
            }
          }
        },
        "/chat/findMessages/{instance_name}": {
          "post": {
            "summary": "Find chat messages (synthetic)",
            "parameters": [
              { "name": "instance_name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": { "required": true },
            "responses": {
              "200": { "description": "Message list" },
              "404": { "description": "Instance not found" }
            }
          }
        },
        "/chat/findChats/{instance_name}": {
          "get": {
            "summary": "Find chats (synthetic)",
            "parameters": [
              { "name": "instance_name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": { "description": "Chats list" },
              "404": { "description": "Instance not found" }
            }
          }
        },
        "/group/create/{instance_name}": {
          "post": {
            "summary": "Create group (synthetic)",
            "parameters": [
              { "name": "instance_name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": { "required": true },
            "responses": {
              "201": { "description": "Group created" },
              "404": { "description": "Instance not found" }
            }
          }
        },
        "/group/fetchAllGroups/{instance_name}": {
          "get": {
            "summary": "List groups (synthetic)",
            "parameters": [
              { "name": "instance_name", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": {
              "200": { "description": "Groups list" },
              "404": { "description": "Instance not found" }
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
