use std::process::{Command, Stdio};
use std::io::Write;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct BaileysManager {
    processes: Arc<Mutex<HashMap<String, std::process::Child>>>,
}

impl BaileysManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn connect(&self, instance_name: &str) -> Result<String, String> {
        let script = format!(
            r#"
import {{ default as makeWASocket, useMultiFileAuthState }} from '@whiskeysockets/baileys';
import path from 'path';
import {{ fileURLToPath }} from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const authDir = path.join(__dirname, 'auth_info', '{}');

const {{ state, saveCreds }} = await useMultiFileAuthState(authDir);
const socket = makeWASocket({{ auth: state, printQRInTerminal: false }});

socket.ev.on('connection.update', (update) => {{
  const {{ qr, pairingCode }} = update;
  if (qr) console.log(JSON.stringify({{ type: 'qr', data: qr }}));
  if (pairingCode) console.log(JSON.stringify({{ type: 'pairing', data: pairingCode }}));
}});

socket.ev.on('creds.update', saveCreds);

await new Promise(r => setTimeout(r, 5000));
"#,
            instance_name
        );

        let output = Command::new("node")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| e.to_string())?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Ok(json) = serde_json::from_str::<Value>(line) {
                if let Some(data) = json.get("data").and_then(|v| v.as_str()) {
                    return Ok(data.to_string());
                }
            }
        }

        Err("No QR or pairing code generated".to_string())
    }
}
