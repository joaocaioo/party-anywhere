//! Os dois arquivos que ligam um jogo ao Party Anywhere (docs/GAME-MANIFEST.md):
//!
//! - `party-anywhere.json`, na raiz do repositório: o que o jogo é (nome, capa, jogadores, como instalar e abrir);
//! - `party-anywhere.release.json`, anexado a cada release: qual pacote baixar e o hash dele.
//!
//! O Hub, a CLI `pa` e o servidor MCP usam as mesmas regras daqui. Sem I/O: quem chama lê os arquivos.

mod cover;
mod game;
mod issues;
mod release;
mod repo;

pub use cover::{check_cover, CoverInfo, COVER_MAX_BYTES, COVER_MIN_HEIGHT, COVER_MIN_WIDTH};
pub use game::{
    Art, BaseGame, GameManifest, Install, InstallFile, InstallSource, InstallTarget, Launch, Players, Requires, Sdk,
    MAX_NAME_CHARS, MAX_TAGLINE_CHARS,
};
pub use issues::{Issue, Severity};
pub use release::{sha256_hex, ReleaseManifest};
pub use repo::RepoRef;

/// Nome do manifesto na raiz do repositório.
pub const MANIFEST_FILE: &str = "party-anywhere.json";
/// Nome do anexo que cada release carrega.
pub const RELEASE_FILE: &str = "party-anywhere.release.json";
/// Única versão de manifesto que este código entende.
pub const MANIFEST_VERSION: u32 = 1;
/// Versão maior do SDK que este Hub fala (docs/SDK.md).
pub const SDK_ABI: u32 = 1;
/// Tópico do GitHub que faz um repositório aparecer em "Descobrir" na Biblioteca.
pub const DISCOVERY_TOPIC: &str = "party-anywhere-game";
/// Esquema JSON publicado (autocompletar no editor).
pub const SCHEMA_URL: &str = "https://raw.githubusercontent.com/joaocaioo/party-anywhere/main/schema/game-manifest.v1.json";

/// Identidade do jogo: `com.autor.jogo`. Minúsculas, números, hífens; ao menos dois segmentos com ponto.
pub fn is_valid_id(id: &str) -> bool {
    let mut parts = id.split('.');
    let Some(first) = parts.next() else { return false };
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        return false;
    }
    let mut rest = 0;
    for p in parts {
        rest += 1;
        let ok = !p.is_empty()
            && !p.starts_with('-')
            && !p.ends_with('-')
            && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return false;
        }
    }
    rest >= 1 && id.len() <= 100
}

/// Caminho relativo seguro para instalar: sem raiz, sem unidade, sem `..`. Protege a pasta do jogo-base
/// contra um pacote que tente escrever fora dela.
pub fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 260 {
        return false;
    }
    let p = path.replace('\\', "/");
    if p.starts_with('/') || p.contains(':') {
        return false;
    }
    p.split('/').all(|seg| !seg.is_empty() && seg != ".." && seg != ".") || p == "."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        for ok in ["com.exemplo.sa-party", "io.github.joao.arena", "a.b", "x9.jogo-2"] {
            assert!(is_valid_id(ok), "{ok}");
        }
        for bad in ["", "jogo", "Com.exemplo.x", "com..x", "com.-x", "com.x-", "com.ex_emplo", "com.exemplo.", ".com.x"] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn safe_paths() {
        for ok in ["MeuJogo.exe", "bin/game.dll", "data\\pack.bin", "."] {
            assert!(is_safe_relative_path(ok), "{ok}");
        }
        for bad in ["", "../x", "a/../../x", "/etc/x", "C:\\Windows\\x", "a//b", "./x"] {
            assert!(!is_safe_relative_path(bad), "{bad}");
        }
    }
}
