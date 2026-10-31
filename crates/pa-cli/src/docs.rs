//! A documentação para quem cria, embutida na `pa` (sempre a da versão instalada): `pa docs <tema>` para
//! qualquer agente com terminal, e `get_docs` / recursos no MCP para quem usa o MCP.

pub struct Doc {
    pub topic: &'static str,
    pub title: &'static str,
    pub mime: &'static str,
    pub body: &'static str,
}

pub const DOCS: [Doc; 4] = [
    Doc {
        topic: "sdk",
        title: "SDK em C (party_anywhere.h): vagas P1..P10, controles, eventos, celular e tela do jogo no celular",
        mime: "text/markdown",
        body: include_str!("../../../docs/SDK.md"),
    },
    Doc {
        topic: "coop",
        title: "Multiplayer online → co-op local sem perder jogabilidade, e como passar do limite de jogadores",
        mime: "text/markdown",
        body: include_str!("../../../docs/MOD-COOP-LOCAL.md"),
    },
    Doc {
        topic: "manifest",
        title: "A ficha party-anywhere.json e o party-anywhere.release.json",
        mime: "text/markdown",
        body: include_str!("../../../docs/GAME-MANIFEST.md"),
    },
    Doc {
        topic: "schema",
        title: "Esquema JSON da ficha",
        mime: "application/schema+json",
        body: include_str!("../../../schema/game-manifest.v1.json"),
    },
];

pub fn find(topic: &str) -> Option<&'static Doc> {
    DOCS.iter().find(|d| d.topic == topic)
}

/// A lista de temas, para `pa docs` sem tema e para a mensagem de tema desconhecido.
pub fn topics() -> String {
    DOCS.iter().map(|d| format!("  pa docs {:<9} {}", d.topic, d.title)).collect::<Vec<_>>().join("\n")
}
