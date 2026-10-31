//! Servidor MCP (Model Context Protocol) por stdio: `pa mcp`. Roda no computador de quem cria, iniciado
//! pelo próprio agente (sem servidor na internet). Dá ao agente as mesmas ferramentas da CLI: conferir e
//! instalar o Hub, achar os jogos instalados, criar o projeto (inclusive o mod co-op de um jogo do PC),
//! validar, empacotar e inspecionar um jogo publicado; e a documentação (a mesma do `pa docs`).
//!
//! É opcional: agentes com terminal usam a `pa` direto (com `--json`) e o `AGENTS.md` do projeto. O MCP serve a
//! quem não tem terminal.
//!
//! JSON-RPC 2.0, uma mensagem por linha. Só o necessário: initialize, ping, tools/*, resources/*.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::docs::{self, DOCS};
use crate::{new_project, pack_dir, validate_dir, BaseGameSpec, NewOptions};

const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// O que depende do mundo real (GitHub, Steam, registro do Windows). Os testes usam um falso.
pub trait Host {
    fn inspect(&self, repo: &str) -> Result<Value, String>;
    fn hub_status(&self) -> Value;
    fn install_hub(&self) -> Result<Value, String>;
    fn find_games(&self, query: &str) -> Vec<Value>;
    fn resolve_base_game(&self, query: &str) -> Result<BaseGameSpec, String>;
    /// Leva um mod (ou jogo) publicado para a Biblioteca: abre o Hub com a prévia; a pessoa confirma lá.
    fn add_to_library(&self, link: &str) -> Result<Value, String>;
}

/// Sem acesso ao mundo real (padrão do `Server::new`).
pub struct Offline;

impl Host for Offline {
    fn inspect(&self, _: &str) -> Result<Value, String> {
        Err("sem acesso à rede".into())
    }
    fn hub_status(&self) -> Value {
        json!({ "summary": "sem acesso ao sistema" })
    }
    fn install_hub(&self) -> Result<Value, String> {
        Err("sem acesso à rede".into())
    }
    fn find_games(&self, _: &str) -> Vec<Value> {
        vec![]
    }
    fn resolve_base_game(&self, q: &str) -> Result<BaseGameSpec, String> {
        Err(format!("não achei \"{q}\""))
    }
    fn add_to_library(&self, _: &str) -> Result<Value, String> {
        Err("sem acesso ao sistema".into())
    }
}

pub struct Server {
    root: PathBuf,
    host: Box<dyn Host>,
}

impl Server {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), host: Box::new(Offline) }
    }

    pub fn with_host(mut self, host: Box<dyn Host>) -> Self {
        self.host = host;
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
            "instructions": "Party Anywhere: transforma jogos do PC em co-op local (até 10 jogadores no mesmo PC, com \
                controles, celulares e teclados). Comece sempre por hub_status: sem o Hub instalado ou com versão nova, \
                ofereça install_hub (peça permissão antes). \
                JOGAR um mod que a pessoa achou (um link de repositório): inspect_published_game e add_to_library. O Hub \
                abre com a prévia, acha o jogo original da pessoa (Steam, Epic ou a pasta que ela escolher) e ela confirma \
                em 'Instalar o mod'. Nunca baixe o jogo original. \
                CRIAR um mod: find_installed_games e create_game com base_game; leia get_docs coop (o caminho é um só: \
                uma cópia do jogo, N jogadores e N visões dentro dela; nunca abrir várias cópias do jogo) e \
                get_docs sdk antes de escrever código \
                de controle (o mod lê os jogadores só pelo SDK); validate_game; publicar é opcional (pack_release e um \
                release no GitHub), mas ajuda outras pessoas a jogar. A capa (retrato 3:4) é obrigatória; peça a arte ao \
                criador. Jogos com anti-cheat ficam de fora."
        })
    }

    fn tools(&self) -> Vec<Value> {
        let dir = json!({ "type": "string", "description": "Pasta do projeto (relativa à pasta atual). Padrão: \".\"" });
        vec![
            json!({
                "name": "hub_status",
                "description": "O Party Anywhere (o Hub) está instalado neste PC? Em que versão, está aberto, e há versão nova? Chame primeiro.",
                "inputSchema": { "type": "object", "properties": {} },
                "annotations": { "readOnlyHint": true, "openWorldHint": true }
            }),
            json!({
                "name": "install_hub",
                "description": "Baixa o instalador mais novo do Hub (conferindo o hash) e abre o instalador na tela, onde a pessoa confirma. Instala ou atualiza. Peça permissão à pessoa antes de chamar.",
                "inputSchema": { "type": "object", "properties": {} },
                "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": true }
            }),
            json!({
                "name": "find_installed_games",
                "description": "Jogos instalados neste PC (Steam e Epic) com pasta, executável e aviso de anti-cheat. Use para escolher o jogo que vira co-op.",
                "inputSchema": { "type": "object", "properties": {
                    "query": { "type": "string", "description": "Parte do nome ou appid da Steam. Vazio lista todos." }
                }},
                "annotations": { "readOnlyHint": true, "openWorldHint": false }
            }),
            json!({
                "name": "create_game",
                "description": "Cria um projeto já válido (party-anywhere.json, capa provisória, SDK com exemplo, workflow de release, AGENTS.md e skill), do jeito que a Biblioteca do Hub instala pelo link do GitHub. Com base_game, cria o mod co-op de um jogo instalado (recusa jogos com anti-cheat). Não sobrescreve arquivos existentes.",
                "inputSchema": { "type": "object", "required": ["dir", "name"], "properties": {
                    "dir": dir,
                    "name": { "type": "string", "description": "Nome do jogo ou do mod (até 40 caracteres)." },
                    "id": { "type": "string", "description": "com.usuario.jogo. Nunca muda depois de publicado." },
                    "tagline": { "type": "string", "description": "Uma frase (até 90 caracteres)." },
                    "base_game": { "type": "string", "description": "Mod co-op de um jogo instalado: nome ou appid da Steam (veja find_installed_games)." }
                }},
                "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false }
            }),
            json!({
                "name": "validate_game",
                "description": "Confere party-anywhere.json e a capa. Devolve a lista de problemas com o que fazer em cada um.",
                "inputSchema": { "type": "object", "properties": { "dir": dir } },
                "annotations": { "readOnlyHint": true, "openWorldHint": false }
            }),
            json!({
                "name": "pack_release",
                "description": "Empacota a pasta de build em release/<pacote>.zip e gera release/party-anywhere.release.json (hash e tamanho). Os dois vão anexados ao release do GitHub.",
                "inputSchema": { "type": "object", "properties": {
                    "dir": dir,
                    "from": { "type": "string", "description": "Pasta do build, relativa ao projeto. Padrão: build" },
                    "tag": { "type": "string", "description": "Tag do release (ex.: v0.2.0); precisa bater com version." }
                }},
                "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
            }),
            json!({
                "name": "get_docs",
                "description": "A documentação para quem cria (a mesma do `pa docs`): sdk (party_anywhere.h: vagas, controles, eventos, celular), coop (como fazer o co-op local: uma cópia do jogo com N jogadores e N visões, nunca várias cópias; reaproveitar o multiplayer do jogo e passar do limite de jogadores; leia antes de propor o caminho de um mod), manifest (party-anywhere.json) ou schema (esquema JSON da ficha).",
                "inputSchema": { "type": "object", "required": ["topic"], "properties": {
                    "topic": { "type": "string", "enum": ["sdk", "coop", "manifest", "schema"] }
                }},
                "annotations": { "readOnlyHint": true, "openWorldHint": false }
            }),
            json!({
                "name": "add_to_library",
                "description": "Leva um mod (ou jogo) publicado para a Biblioteca do Party Anywhere: abre o Hub com a prévia do link; o Hub acha o jogo original da pessoa e ela confirma em 'Instalar o mod'. Antes: hub_status (e install_hub se faltar).",
                "inputSchema": { "type": "object", "required": ["repo"], "properties": {
                    "repo": { "type": "string", "description": "Link do repositório ou dono/repo." }
                }},
                "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": true }
            }),
            json!({
                "name": "inspect_published_game",
                "description": "Lê um jogo publicado no GitHub como o Hub lê: manifesto, último release e anexo, e diz se ele instala pelo link.",
                "inputSchema": { "type": "object", "required": ["repo"], "properties": {
                    "repo": { "type": "string", "description": "Link do repositório ou dono/repo." }
                }},
                "annotations": { "readOnlyHint": true, "openWorldHint": true }
            }),
        ]
    }

    fn call_tool(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        let dir = self.root.join(s("dir").unwrap_or_else(|| ".".into()));
        match name {
            "hub_status" => {
                let st = self.host.hub_status();
                let summary = st.get("summary").and_then(Value::as_str).unwrap_or("").to_string();
                Ok(tool_ok(summary, st))
            }
            "install_hub" => Ok(match self.host.install_hub() {
                Ok(v) => tool_ok(v.get("summary").and_then(Value::as_str).unwrap_or("Instalador aberto.").to_string(), v),
                Err(e) => tool_err(e),
            }),
            "find_installed_games" => {
                let games = self.host.find_games(&s("query").unwrap_or_default());
                let text = if games.is_empty() {
                    "Nenhum jogo da Steam ou da Epic encontrado com esse nome.".to_string()
                } else {
                    games.iter().map(|g| format!("- {} ({})", g["name"].as_str().unwrap_or(""), g["store"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")
                };
                Ok(tool_ok(text, json!({ "games": games })))
            }
            "create_game" => {
                let Some(name) = s("name") else { return Err((-32602, "falta \"name\"".into())) };
                let base_game = match s("base_game") {
                    Some(q) => match self.host.resolve_base_game(&q) {
                        Ok(b) => Some(b),
                        Err(e) => return Ok(tool_err(e)),
                    },
                    None => None,
                };
                let opts = NewOptions { name, id: s("id"), tagline: s("tagline"), base_game };
                Ok(match new_project(&dir, &opts) {
                    Ok(files) => {
                        let list: Vec<String> = files.iter().map(|f| rel(&dir, f)).collect();
                        tool_ok(
                            format!(
                                "Projeto criado em {} ({} arquivos). Próximos passos: trocar assets/cover.png pela capa de verdade, \
                                 ler get_docs sdk, ajustar o passo Build de .github/workflows/release.yml.",
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
            "get_docs" => {
                let topic = s("topic").unwrap_or_default();
                match docs::find(&topic) {
                    Some(d) => Ok(text(d.body)),
                    None => Err((-32602, format!("tema desconhecido: {topic} (sdk, coop, manifest ou schema)"))),
                }
            }
            "inspect_published_game" => {
                let Some(repo) = s("repo") else { return Err((-32602, "falta \"repo\"".into())) };
                Ok(match self.host.inspect(&repo) {
                    Ok(report) => {
                        let summary = report.get("summary").and_then(Value::as_str).unwrap_or("ok").to_string();
                        tool_ok(summary, report)
                    }
                    Err(e) => tool_err(e),
                })
            }
            "add_to_library" => {
                let Some(repo) = s("repo") else { return Err((-32602, "falta \"repo\"".into())) };
                Ok(match self.host.add_to_library(&repo) {
                    Ok(v) => tool_ok(v.get("summary").and_then(Value::as_str).unwrap_or("Hub aberto.").to_string(), v),
                    Err(e) => tool_err(e),
                })
            }
            _ => Err((-32602, format!("ferramenta desconhecida: {name}"))),
        }
    }
}

fn resources() -> Vec<Value> {
    DOCS.iter().map(|d| json!({ "uri": format!("party-anywhere://docs/{}", d.topic), "name": d.title, "mimeType": d.mime })).collect()
}

fn read_resource(params: &Value) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or_default();
    let doc = uri.strip_prefix("party-anywhere://docs/").and_then(docs::find).ok_or((-32602, format!("recurso desconhecido: {uri}")))?;
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": doc.mime, "text": doc.body }] }))
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
        assert_eq!(
            names,
            [
                "hub_status",
                "install_hub",
                "find_installed_games",
                "create_game",
                "validate_game",
                "pack_release",
                "get_docs",
                "add_to_library",
                "inspect_published_game"
            ]
        );

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

    struct FakeHost;
    impl Host for FakeHost {
        fn inspect(&self, _: &str) -> Result<Value, String> {
            Ok(json!({ "summary": "pronto" }))
        }
        fn hub_status(&self) -> Value {
            json!({ "installed": null, "summary": "O Party Anywhere não está instalado." })
        }
        fn install_hub(&self) -> Result<Value, String> {
            Ok(json!({ "summary": "Instalador da versão 0.2.0 aberto." }))
        }
        fn find_games(&self, q: &str) -> Vec<Value> {
            vec![json!({ "name": format!("Jogo {q}"), "store": "Steam" })]
        }
        fn resolve_base_game(&self, q: &str) -> Result<BaseGameSpec, String> {
            if q == "anticheat" {
                return Err("usa anti-cheat (EasyAntiCheat)".into());
            }
            Ok(BaseGameSpec { name: "GTA San Andreas".into(), steam_app_id: Some(12120), epic_app_name: None, exe: "gta_sa.exe".into() })
        }
        fn add_to_library(&self, link: &str) -> Result<Value, String> {
            Ok(json!({ "summary": format!("Party Anywhere aberto com {link}.") }))
        }
    }

    #[test]
    fn agent_takes_a_mod_link_to_the_library() {
        let s = Server::new(".").with_host(Box::new(FakeHost));
        let r = call(&s, 1, "tools/call", json!({ "name": "add_to_library", "arguments": { "repo": "autor/mod-coop" } }));
        assert_eq!(r["result"]["isError"], false);
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("autor/mod-coop"));
        let r = call(&s, 2, "tools/call", json!({ "name": "add_to_library", "arguments": {} }));
        assert!(r["error"].is_object(), "sem repo é erro de parâmetro");
    }

    #[test]
    fn agent_checks_the_hub_and_turns_an_installed_game_into_coop() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Server::new(tmp.path()).with_host(Box::new(FakeHost));
        let r = call(&s, 1, "tools/call", json!({ "name": "hub_status" }));
        assert_eq!(r["result"]["content"][0]["text"], "O Party Anywhere não está instalado.");
        let r = call(&s, 2, "tools/call", json!({ "name": "install_hub" }));
        assert_eq!(r["result"]["isError"], false);
        let r = call(&s, 3, "tools/call", json!({ "name": "find_installed_games", "arguments": { "query": "gta" } }));
        assert_eq!(r["result"]["structuredContent"]["games"][0]["name"], "Jogo gta");

        let r = call(&s, 4, "tools/call", json!({ "name": "create_game", "arguments": { "dir": "sa", "name": "SA Party", "base_game": "12120" } }));
        assert_eq!(r["result"]["isError"], false, "{r}");
        let v = crate::validate_dir(&tmp.path().join("sa"));
        assert!(v.ok, "{:?}", v.issues);
        assert_eq!(v.manifest.unwrap().requires.unwrap().base_game.unwrap().steam_app_id, Some(12120));

        let r = call(&s, 5, "tools/call", json!({ "name": "create_game", "arguments": { "dir": "x", "name": "X", "base_game": "anticheat" } }));
        assert_eq!(r["result"]["isError"], true);
        assert!(!tmp.path().join("x").exists(), "nada criado");
    }

    #[test]
    fn docs_are_served_as_tool_and_resource() {
        let s = Server::new(".");
        let r = call(&s, 1, "tools/call", json!({ "name": "get_docs", "arguments": { "topic": "sdk" } }));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("pa_get_pad"));
        let r = call(&s, 2, "tools/call", json!({ "name": "get_docs", "arguments": { "topic": "coop" } }));
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("limite de jogadores"));
        assert!(call(&s, 3, "tools/call", json!({ "name": "get_docs", "arguments": { "topic": "x" } }))["error"].is_object());
        let r = call(&s, 4, "resources/read", json!({ "uri": "party-anywhere://docs/schema" }));
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
