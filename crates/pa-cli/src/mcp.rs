//! Servidor MCP (Model Context Protocol) por stdio: `pa mcp`. Dá a agentes de IA as mesmas ferramentas da
//! CLI (criar, validar, empacotar, inspecionar um jogo publicado) e a documentação do SDK e do manifesto.
//!
//! JSON-RPC 2.0, uma mensagem por linha. Só o necessário: initialize, ping, tools/*, resources/*.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{new_project, pack_dir, validate_dir, NewOptions};

const SDK_DOCS: &str = include_str!("../../../docs/SDK.md");
const MANIFEST_DOCS: &str = include_str!("../../../docs/GAME-MANIFEST.md");
const MANIFEST_SCHEMA: &str = include_str!("../../../schema/game-manifest.v1.json");
const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Consulta um jogo publicado no GitHub (rede). Fica de fora nos testes.
pub type Inspector = Box<dyn Fn(&str) -> Result<Value, String>>;

pub struct Server {
    root: PathBuf,
    inspector: Option<Inspector>,
}

impl Server {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), inspector: None }
    }

    pub fn with_inspector(mut self, inspector: Inspector) -> Self {
        self.inspector = Some(inspector);
        self
    }

    /// Lê requisições de `input` até acabar e responde em `output`.
    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(msg) => self.handle(&msg),
                Err(e) => Some(error(Value::Null, -32700, &format!("JSON inválido: {e}"))),
            };
            if let Some(reply) = reply {
                writeln!(output, "{reply}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// Uma mensagem → a resposta (notificações não têm resposta).
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
        let id = msg.get("id").cloned()?; // sem id: notificação
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools() })),
            "tools/call" => self.call_tool(&params),
            "resources/list" => Ok(json!({ "resources": resources() })),
            "resources/read" => read_resource(&params),
            _ => Err((-32601, format!("método desconhecido: {method}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => error(id, code, &message),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
        let version = SUPPORTED_PROTOCOLS.iter().find(|v| **v == asked).copied().unwrap_or(SUPPORTED_PROTOCOLS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": {}, "resources": {} },
            "serverInfo": { "name": "party-anywhere", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Ferramentas para criar jogos co-op locais para o Party Anywhere (até 10 jogadores). \
                Leia get_sdk_docs antes de escrever código de controle: o jogo lê os jogadores só pelo SDK. \
                A capa (retrato 3:4) é obrigatória; peça a arte ao criador."
        })
    }

    fn tools(&self) -> Vec<Value> {
        let dir = json!({ "type": "string", "description": "Pasta do projeto (relativa à pasta atual). Padrão: \".\"" });
        let mut tools = vec![
            json!({
                "name": "create_game",
                "description": "Cria um projeto de jogo já válido: party-anywhere.json, capa provisória, workflow de release, MCP e skill. Não sobrescreve arquivos existentes.",
                "inputSchema": { "type": "object", "required": ["dir", "name"], "properties": {
                    "dir": dir,
                    "name": { "type": "string", "description": "Nome do jogo (até 40 caracteres)." },
                    "id": { "type": "string", "description": "com.usuario.jogo. Nunca muda depois de publicado." },
                    "tagline": { "type": "string", "description": "Uma frase (até 90 caracteres)." }
                }}
            }),
            json!({
                "name": "validate_game",
                "description": "Confere party-anywhere.json e a capa. Devolve a lista de problemas com o que fazer em cada um.",
                "inputSchema": { "type": "object", "properties": { "dir": dir } }
            }),
            json!({
                "name": "pack_release",
                "description": "Empacota a pasta de build em release/<pacote>.zip e gera release/party-anywhere.release.json (hash e tamanho). Os dois vão anexados ao release do GitHub.",
                "inputSchema": { "type": "object", "properties": {
                    "dir": dir,
                    "from": { "type": "string", "description": "Pasta do build, relativa ao projeto. Padrão: build" },
                    "tag": { "type": "string", "description": "Tag do release (ex.: v0.2.0); precisa bater com version." }
                }}
            }),
            json!({
                "name": "get_sdk_docs",
                "description": "Contrato do SDK em C (party_anywhere.h): conexão, vagas P1..P10, estado do controle, eventos, vibração e rótulos no celular.",
                "inputSchema": { "type": "object", "properties": {} }
            }),
            json!({
                "name": "get_manifest_spec",
                "description": "Especificação do party-anywhere.json e do party-anywhere.release.json, com o esquema JSON.",
                "inputSchema": { "type": "object", "properties": {} }
            }),
        ];
        if self.inspector.is_some() {
            tools.push(json!({
                "name": "inspect_published_game",
                "description": "Lê um jogo publicado no GitHub como o Hub lê: manifesto, último release e anexo, e diz se ele instala.",
                "inputSchema": { "type": "object", "required": ["repo"], "properties": {
                    "repo": { "type": "string", "description": "Link do repositório ou dono/repo." }
                }}
            }));
        }
        tools
    }

    fn call_tool(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        let dir = self.root.join(s("dir").unwrap_or_else(|| ".".into()));
        match name {
            "create_game" => {
                let Some(name) = s("name") else { return Err((-32602, "falta \"name\"".into())) };
                let opts = NewOptions { name, id: s("id"), tagline: s("tagline") };
                Ok(match new_project(&dir, &opts) {
                    Ok(files) => {
                        let list: Vec<String> = files.iter().map(|f| rel(&dir, f)).collect();
                        tool_ok(
                            format!(
                                "Projeto criado em {} ({} arquivos). Próximos passos: trocar assets/cover.png pela capa de verdade, \
                                 ler get_sdk_docs, ajustar o passo Build de .github/workflows/release.yml.",
                                dir.display(),
                                list.len()
                            ),
                            json!({ "created": list }),
                        )
                    }
                    Err(e) => tool_err(e),
                })
            }
            "validate_game" => {
                let v = validate_dir(&dir);
                let text = if v.issues.is_empty() {
                    "Tudo certo: o Hub aceita este jogo.".to_string()
                } else {
                    v.issues.iter().map(|i| format!("- {i}")).collect::<Vec<_>>().join("\n")
                };
                let structured = serde_json::to_value(&v).expect("Validation serializa");
                Ok(json!({ "content": [{ "type": "text", "text": text }], "structuredContent": structured, "isError": !v.ok }))
            }
            "pack_release" => {
                let from = s("from").map(PathBuf::from);
                Ok(match pack_dir(&dir, from.as_deref(), s("tag").as_deref()) {
                    Ok(p) => tool_ok(
                        format!(
                            "Pacote {} ({} arquivos, sha256 {}). Anexe release/* ao release da tag v{}.",
                            p.package.file_name().unwrap_or_default().to_string_lossy(),
                            p.files,
                            p.release.sha256,
                            p.release.version
                        ),
                        serde_json::to_value(&p).expect("Packed serializa"),
                    ),
                    Err(issues) => tool_err(issues.iter().map(|i| format!("- {i}")).collect::<Vec<_>>().join("\n")),
                })
            }
            "get_sdk_docs" => Ok(text(SDK_DOCS)),
            "get_manifest_spec" => Ok(text(&format!("{MANIFEST_DOCS}\n\n## Esquema JSON\n\n```json\n{MANIFEST_SCHEMA}```\n"))),
            "inspect_published_game" => {
                let (Some(inspector), Some(repo)) = (&self.inspector, s("repo")) else {
                    return Err((-32602, "falta \"repo\"".into()));
                };
                Ok(match inspector(&repo) {
                    Ok(report) => {
                        let summary = report.get("summary").and_then(Value::as_str).unwrap_or("ok").to_string();
                        tool_ok(summary, report)
                    }
                    Err(e) => tool_err(e),
                })
            }
            _ => Err((-32602, format!("ferramenta desconhecida: {name}"))),
        }
    }
}

fn resources() -> Vec<Value> {
    vec![
        json!({ "uri": "party-anywhere://docs/sdk", "name": "SDK (party_anywhere.h)", "mimeType": "text/markdown" }),
        json!({ "uri": "party-anywhere://docs/game-manifest", "name": "Manifesto de jogo", "mimeType": "text/markdown" }),
        json!({ "uri": "party-anywhere://schema/game-manifest.v1.json", "name": "Esquema do manifesto", "mimeType": "application/schema+json" }),
    ]
}

fn read_resource(params: &Value) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
    let (mime, body) = match uri {
        "party-anywhere://docs/sdk" => ("text/markdown", SDK_DOCS),
        "party-anywhere://docs/game-manifest" => ("text/markdown", MANIFEST_DOCS),
        "party-anywhere://schema/game-manifest.v1.json" => ("application/schema+json", MANIFEST_SCHEMA),
        _ => return Err((-32602, format!("recurso desconhecido: {uri}"))),
    };
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": body }] }))
}

fn text(t: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": t }], "isError": false })
}

fn tool_ok(summary: String, structured: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": summary }], "structuredContent": structured, "isError": false })
}

fn tool_err(message: String) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn rel(base: &Path, p: &Path) -> String {
    p.strip_prefix(base).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &Server, id: u64, method: &str, params: Value) -> Value {
        server.handle(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).unwrap()
    }

    #[test]
    fn handshake_and_listing() {
        let s = Server::new(".");
        let init = call(&s, 1, "initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {} }));
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "party-anywhere");
        assert!(s.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());

        let tools = call(&s, 2, "tools/list", json!({}));
        let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["create_game", "validate_game", "pack_release", "get_sdk_docs", "get_manifest_spec"]);

        let unknown = call(&s, 3, "nada", json!({}));
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn agent_can_create_validate_and_pack_a_game() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Server::new(tmp.path());
        let r = call(&s, 1, "tools/call", json!({ "name": "create_game", "arguments": { "dir": "kart", "name": "Kart Caos" } }));
        assert_eq!(r["result"]["isError"], false, "{r}");

        let r = call(&s, 2, "tools/call", json!({ "name": "validate_game", "arguments": { "dir": "kart" } }));
        assert_eq!(r["result"]["structuredContent"]["ok"], true, "{r}");

        let r = call(&s, 3, "tools/call", json!({ "name": "pack_release", "arguments": { "dir": "kart" } }));
        assert_eq!(r["result"]["isError"], true, "sem build ainda");
        std::fs::create_dir_all(tmp.path().join("kart/build")).unwrap();
        std::fs::write(tmp.path().join("kart/build/kart-caos.exe"), b"MZ").unwrap();
        let r = call(&s, 4, "tools/call", json!({ "name": "pack_release", "arguments": { "dir": "kart", "tag": "v0.1.0" } }));
        assert_eq!(r["result"]["isError"], false, "{r}");
        assert_eq!(r["result"]["structuredContent"]["release"]["asset"], "kart-caos-0.1.0.zip");
    }

    #[test]
    fn docs_are_served_as_tool_and_resource() {
        let s = Server::new(".");
        let r = call(&s, 1, "tools/call", json!({ "name": "get_sdk_docs" }));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("pa_get_pad"));
        let r = call(&s, 2, "resources/read", json!({ "uri": "party-anywhere://schema/game-manifest.v1.json" }));
        assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("manifestVersion"));
    }

    #[test]
    fn serve_speaks_line_delimited_json() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\nnot json\n";
        let mut out = Vec::new();
        Server::new(".").serve(&input[..], &mut out).unwrap();
        let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines[0]["result"], json!({}));
        assert_eq!(lines[1]["error"]["code"], -32700);
    }
}
