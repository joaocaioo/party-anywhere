//! `pa`: a ferramenta de quem cria jogos para o Party Anywhere.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pa_cli::host::{add_to_hub, game_json, install_hub, RealHost};
use pa_cli::{mcp, new_project, pack_dir, resolve_base_game, validate_dir, NewOptions};
use pa_manifest::{Issue, Severity};

const HELP: &str = "pa: jogos co-op para o Party Anywhere

Uso:
  pa hub                      o Party Anywhere está instalado e atualizado?
  pa hub install              baixa e abre o instalador (instala ou atualiza)
  pa games [busca] [--json]   jogos instalados (Steam e Epic) que podem virar co-op
  pa new <pasta> --name \"Meu Jogo\" [--id com.usuario.jogo] [--tagline \"...\"]
  pa new <pasta> --name \"Meu Mod\" --base-game \"<nome ou appid>\"   mod co-op de um jogo instalado
  pa validate [pasta] [--json]
  pa pack [pasta] [--from build] [--tag v0.1.0] [--json]
  pa inspect <link do repositório | dono/repo> [--json]
  pa add <link do repositório | dono/repo | pasta do projeto>   leva o mod para a Biblioteca (abre o Party Anywhere)
  pa mcp                      servidor MCP (stdio) para agentes de IA
  pa --version

Docs: https://github.com/joaocaioo/party-anywhere/blob/main/docs/GAME-MANIFEST.md";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("pa: {msg}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let (cmd, rest) = args.split_first().ok_or_else(|| HELP.to_string())?;
    let opts = Opts::parse(rest)?;
    match cmd.as_str() {
        "new" => {
            let dir = opts.positional.first().ok_or("diga a pasta: pa new <pasta> --name \"Meu Jogo\"")?;
            let name = opts.get("--name").map(str::to_string).unwrap_or_else(|| folder_name(dir));
            let base_game = opts.get("--base-game").map(resolve_base_game).transpose()?;
            if let Some(b) = &base_game {
                println!("Jogo-base: {} ({})", b.name, b.exe);
            }
            let created = new_project(
                Path::new(dir),
                &NewOptions { name, id: opts.get("--id").map(Into::into), tagline: opts.get("--tagline").map(Into::into), base_game },
            )?;
            println!("Projeto criado em {dir} ({} arquivos).", created.len());
            println!("Próximos passos:");
            println!("  1. Troque assets/cover.png pela capa de verdade (retrato 3:4, mínimo 600×800).");
            println!("  2. Leia o SDK: os jogadores chegam pelo party_anywhere.h, sem código de controle.");
            println!("  3. Ajuste o passo Build em .github/workflows/release.yml e crie a tag v0.1.0 para publicar.");
            println!("  4. Adicione o tópico party-anywhere-game ao repositório para aparecer em \"Descobrir\".");
            Ok(ExitCode::SUCCESS)
        }
        "validate" => {
            let dir = opts.dir();
            let v = validate_dir(&dir);
            if opts.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&v).expect("serializa"));
            } else if v.issues.is_empty() {
                println!("Tudo certo: {} {} pode ser publicado.", v.id.unwrap_or_default(), v.version.unwrap_or_default());
            } else {
                print_issues(&v.issues);
            }
            Ok(if v.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        "pack" => {
            let dir = opts.dir();
            let from = opts.get("--from").map(PathBuf::from);
            match pack_dir(&dir, from.as_deref(), opts.get("--tag")) {
                Ok(p) => {
                    if opts.flag("--json") {
                        println!("{}", serde_json::to_string_pretty(&p).expect("serializa"));
                    } else {
                        println!("Pacote: {} ({} arquivos, {} KB)", p.package.display(), p.files, p.release.size / 1024);
                        println!("Anexo:  {}", p.release_manifest.display());
                        println!("sha256: {}", p.release.sha256);
                        println!("Anexe os dois ao release da tag v{}.", p.release.version);
                    }
                    Ok(ExitCode::SUCCESS)
                }
                Err(issues) => {
                    print_issues(&issues);
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        "hub" => {
            if opts.positional.first().map(String::as_str) == Some("install") {
                println!("{}", install_hub()?);
                return Ok(ExitCode::SUCCESS);
            }
            let st = pa_library::hubapp::status(&pa_library::http::Ureq::default());
            if opts.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&st).expect("serializa"));
            } else {
                println!("{}", st.summary);
            }
            Ok(if st.installed.is_some() && !st.update_available { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        "games" => {
            let q = opts.positional.join(" ");
            let games: Vec<_> = pa_library::pcgames::find(&q).iter().map(game_json).collect();
            if opts.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&games).expect("serializa"));
            } else if games.is_empty() {
                println!("Nenhum jogo da Steam ou da Epic encontrado.");
            } else {
                for g in &games {
                    let id = g["steamAppId"].as_u64().map(|i| format!(" · appid {i}")).unwrap_or_default();
                    let ac = g["antiCheat"].as_str().map(|a| format!("  ✗ anti-cheat ({a}): fora do Party Anywhere")).unwrap_or_default();
                    println!("  {} ({}{id}){ac}", g["name"].as_str().unwrap_or(""), g["store"].as_str().unwrap_or(""));
                }
                println!("
Para criar o mod co-op: pa new <pasta> --name \"Meu Mod\" --base-game \"<nome ou appid>\"");
            }
            Ok(ExitCode::SUCCESS)
        }
        "inspect" => {
            let repo = opts.positional.first().ok_or("diga o repositório: pa inspect dono/repo")?;
            let report = pa_library::inspect(&pa_library::http::Ureq::default(), repo)?;
            if opts.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&report).expect("serializa"));
            } else {
                println!("{}", report.summary);
                print_issues(&report.issues);
            }
            Ok(if report.installable { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        "add" => {
            let link = opts.positional.first().ok_or("diga o mod: pa add dono/repo (ou o link do GitHub)")?;
            println!("{}", add_to_hub(link)?);
            Ok(ExitCode::SUCCESS)
        }
        "mcp" => {
            let root = std::env::current_dir().map_err(|e| e.to_string())?;
            let server = mcp::Server::new(root).with_host(Box::new(RealHost));
            let stdin = std::io::stdin();
            server.serve(stdin.lock(), std::io::stdout()).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
        "--version" | "-V" | "version" => {
            println!("pa {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        "--help" | "-h" | "help" => {
            println!("{HELP}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("comando desconhecido: {other}\n\n{HELP}")),
    }
}

fn print_issues(issues: &[Issue]) {
    for i in issues {
        let mark = match i.severity {
            Severity::Error => "✗",
            Severity::Warning => "!",
        };
        println!("  {mark} {}: {}", i.field, i.message);
    }
}

fn folder_name(dir: &str) -> String {
    Path::new(dir).file_name().map(|n| n.to_string_lossy().replace(['-', '_'], " ")).unwrap_or_default()
}

struct Opts {
    positional: Vec<String>,
    named: Vec<(String, Option<String>)>,
}

impl Opts {
    fn parse(args: &[String]) -> Result<Self, String> {
        const WITH_VALUE: [&str; 6] = ["--name", "--id", "--tagline", "--from", "--tag", "--base-game"];
        let mut positional = Vec::new();
        let mut named = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            if WITH_VALUE.contains(&a.as_str()) {
                let v = it.next().ok_or_else(|| format!("{a} precisa de um valor"))?;
                named.push((a.clone(), Some(v.clone())));
            } else if a.starts_with("--") {
                named.push((a.clone(), None));
            } else {
                positional.push(a.clone());
            }
        }
        Ok(Self { positional, named })
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.named.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.as_deref())
    }

    fn flag(&self, key: &str) -> bool {
        self.named.iter().any(|(k, _)| k == key)
    }

    fn dir(&self) -> PathBuf {
        PathBuf::from(self.positional.first().map(String::as_str).unwrap_or("."))
    }
}
