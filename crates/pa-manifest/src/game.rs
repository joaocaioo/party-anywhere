use serde::{Deserialize, Serialize};

use crate::issues::Issue;
use crate::{is_safe_relative_path, is_valid_id, MANIFEST_VERSION, SDK_ABI};

pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_TAGLINE_CHARS: usize = 90;
/// Igual a `pa_core::MAX_SEATS` (P1..P10).
const MAX_PLAYERS: u8 = 10;

/// `party-anywhere.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub tagline: String,
    /// Texto longo, ou o caminho de um .md no repositório (normalmente `README.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    pub version: String,
    pub art: Art,
    pub players: Players,
    pub sdk: Sdk,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    pub install: Install,
    pub launch: Launch,
    /// Hub mínimo para este jogo, se ele usa algo recente do SDK.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_hub_version: Option<String>,
    /// O controle do celular feito para este jogo (opcional): vira o "Padrão do jogo" no Hub.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<crate::controls::Controls>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Art {
    pub cover: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Players {
    pub min: u8,
    pub max: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sdk {
    pub abi: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requires {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_game: Option<BaseGame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseGame {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_app_id: Option<u32>,
    /// Nome do jogo na Epic (o `AppName` dela): o Hub acha a instalação e sabe pedir para a Epic instalar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic_app_name: Option<String>,
    /// Onde comprar, quando não é Steam (GOG, site do jogo). Sem isto, a página da Steam.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_url: Option<String>,
    pub exe: String,
    /// Versões aceitas do executável. Vazio = qualquer uma.
    #[serde(default)]
    pub accepted_sha256: Vec<String>,
    /// O que fazer se a versão não bater (caminho de um .md no repositório ou texto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallSource {
    GithubRelease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallTarget {
    /// Pasta própria do jogo em `%LOCALAPPDATA%\PartyAnywhere\games\<id>\<versão>\`.
    #[serde(rename = "self")]
    SelfDir,
    /// Copia os arquivos para a pasta do jogo-base (mods), com backup.
    BaseGame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub source: InstallSource,
    /// Nome do pacote no release; `{version}` vira a versão.
    pub asset: String,
    pub target: InstallTarget,
    /// Para `baseGame`: o que copiar de dentro do pacote para a pasta do jogo. Para `self`: opcional
    /// (sem a lista, o pacote inteiro é extraído).
    #[serde(default)]
    pub files: Vec<InstallFile>,
    /// Campo da v1 em rascunho: o hash agora vive no `party-anywhere.release.json`. Ignorado.
    #[serde(default, skip_serializing)]
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallFile {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub exe: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "dot")]
    pub working_dir: String,
}

fn dot() -> String {
    ".".into()
}

impl GameManifest {
    /// Lê e valida. `Err` traz as mensagens prontas para mostrar; avisos vêm junto do `Ok`.
    pub fn parse(json: &str) -> Result<(Self, Vec<Issue>), Vec<Issue>> {
        let manifest: GameManifest = serde_json::from_str(json).map_err(|e| {
            vec![Issue::error(
                "(arquivo)",
                format!("o JSON não é um manifesto válido ({e}). Compare com o modelo em templates/game."),
            )]
        })?;
        let issues = manifest.validate();
        if issues.iter().any(Issue::is_error) {
            Err(issues)
        } else {
            Ok((manifest, issues))
        }
    }

    /// Nome do pacote desta versão.
    pub fn asset_name(&self) -> String {
        self.install.asset.replace("{version}", self.version.trim_start_matches('v'))
    }

    /// Versão sem o `v` da tag.
    pub fn semver(&self) -> Option<semver::Version> {
        semver::Version::parse(self.version.trim_start_matches('v')).ok()
    }

    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        if self.manifest_version != MANIFEST_VERSION {
            out.push(Issue::error(
                "manifestVersion",
                format!("este Hub entende a versão {MANIFEST_VERSION}; atualize o Hub ou use \"manifestVersion\": 1."),
            ));
        }
        if !is_valid_id(&self.id) {
            out.push(Issue::error(
                "id",
                "use o formato com.seu-usuario.nome-do-jogo: minúsculas, números e hífens, separados por pontos. O id nunca muda depois de publicado.",
            ));
        }
        check_text(&mut out, "name", &self.name, MAX_NAME_CHARS);
        check_text(&mut out, "tagline", &self.tagline, MAX_TAGLINE_CHARS);
        if self.semver().is_none() {
            out.push(Issue::error("version", "use semver, ex.: 0.1.0 (a tag do release pode ter o v: v0.1.0)."));
        }
        if let Some(min) = &self.min_hub_version {
            if semver::Version::parse(min.trim_start_matches('v')).is_err() {
                out.push(Issue::error("minHubVersion", "use semver, ex.: 0.2.0."));
            }
        }
        if self.authors.is_empty() {
            out.push(Issue::warning("authors", "diga quem fez o jogo: [\"seu-usuario\"]."));
        }

        if let Some(c) = &self.controls {
            if let Err(e) = crate::controls::check_modes(&c.modes) {
                out.push(Issue::error("controls", format!("{e} Monte no editor do Hub (Controles → Criar layout) e copie de lá.")));
            }
        }
        check_image_path(&mut out, "art.cover", Some(&self.art.cover));
        check_image_path(&mut out, "art.hero", self.art.hero.as_ref());
        check_image_path(&mut out, "art.logo", self.art.logo.as_ref());

        let p = self.players;
        if p.min < 1 || p.min > p.max || p.max > MAX_PLAYERS {
            out.push(Issue::error("players", format!("precisa de 1 ≤ min ≤ max ≤ {MAX_PLAYERS}.")));
        } else if let Some(r) = p.recommended {
            if r < p.min || r > p.max {
                out.push(Issue::warning("players.recommended", "fique entre min e max."));
            }
        }
        if self.sdk.abi != SDK_ABI {
            out.push(Issue::error(
                "sdk.abi",
                format!("este Hub fala o SDK {SDK_ABI}. Recompile com o party_anywhere.h atual."),
            ));
        }
        for (i, t) in self.tags.iter().enumerate() {
            if t.is_empty() || t.len() > 24 || t.chars().any(|c| c.is_uppercase() || c.is_whitespace()) {
                out.push(Issue::warning(&format!("tags[{i}]"), "tags curtas, minúsculas, sem espaço: \"acao\", \"corrida\"."));
            }
        }

        if let Some(base) = self.requires.as_ref().and_then(|r| r.base_game.as_ref()) {
            if base.exe.trim().is_empty() || !is_safe_relative_path(&base.exe) {
                out.push(Issue::error("requires.baseGame.exe", "o nome do executável do jogo-base, ex.: gta_sa.exe."));
            }
            for (i, h) in base.accepted_sha256.iter().enumerate() {
                if !is_sha256(h) {
                    out.push(Issue::error(&format!("requires.baseGame.acceptedSha256[{i}]"), "64 caracteres hexadecimais."));
                }
            }
        }

        let install = &self.install;
        if install.asset.trim().is_empty() || install.asset.contains(['/', '\\']) {
            out.push(Issue::error("install.asset", "o nome do arquivo anexado ao release, ex.: meu-jogo-{version}.zip."));
        } else if !install.asset.to_ascii_lowercase().ends_with(".zip") {
            out.push(Issue::error("install.asset", "o pacote precisa ser .zip."));
        }
        if install.target == InstallTarget::BaseGame {
            if self.requires.as_ref().and_then(|r| r.base_game.as_ref()).is_none() {
                out.push(Issue::error("install.target", "\"baseGame\" exige requires.baseGame (qual jogo procurar)."));
            }
            if install.files.is_empty() {
                out.push(Issue::error("install.files", "diga quais arquivos copiar para a pasta do jogo-base."));
            }
        }
        for (i, f) in install.files.iter().enumerate() {
            for (name, path) in [("from", &f.from), ("to", &f.to)] {
                if !is_safe_relative_path(path) || path == "." {
                    out.push(Issue::error(
                        &format!("install.files[{i}].{name}"),
                        "caminho relativo, sem .. e sem C:\\ (o Hub nunca escreve fora da pasta do jogo).",
                    ));
                }
            }
        }

        if !is_safe_relative_path(&self.launch.exe) || self.launch.exe == "." {
            out.push(Issue::error("launch.exe", "caminho relativo do executável, ex.: MeuJogo.exe."));
        }
        if !is_safe_relative_path(&self.launch.working_dir) {
            out.push(Issue::error("launch.workingDir", "caminho relativo; use \".\" para a pasta do jogo."));
        }
        if install.sha256.is_some() {
            out.push(Issue::warning(
                "install.sha256",
                "não é mais usado: o hash vai no party-anywhere.release.json que o `pa pack` gera. Pode apagar.",
            ));
        }
        out
    }
}

fn check_text(out: &mut Vec<Issue>, field: &str, value: &str, max: usize) {
    let n = value.trim().chars().count();
    if n == 0 {
        out.push(Issue::error(field, "não pode ficar vazio."));
    } else if n > max {
        out.push(Issue::error(field, format!("até {max} caracteres (tem {n}).")));
    }
}

fn check_image_path(out: &mut Vec<Issue>, field: &str, path: Option<&String>) {
    let Some(path) = path else { return };
    let lower = path.to_ascii_lowercase();
    if !is_safe_relative_path(path) || path == "." {
        out.push(Issue::error(field, "caminho relativo dentro do repositório, ex.: assets/cover.png."));
    } else if ![".png", ".jpg", ".jpeg", ".webp"].iter().any(|ext| lower.ends_with(ext)) {
        out.push(Issue::error(field, "use PNG, JPG ou WebP."));
    }
}

pub(crate) fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    pub(crate) const TEMPLATE: &str = include_str!("../../../templates/game/party-anywhere.json");

    fn base() -> GameManifest {
        GameManifest::parse(TEMPLATE).expect("o template precisa ser válido").0
    }

    fn errors(m: &GameManifest) -> Vec<String> {
        m.validate().into_iter().filter(Issue::is_error).map(|i| i.field).collect()
    }

    #[test]
    fn template_is_valid_without_warnings() {
        let (m, warnings) = GameManifest::parse(TEMPLATE).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(m.asset_name(), "meu-jogo-0.1.0.zip");
    }

    #[test]
    fn broken_json_explains_itself() {
        let err = GameManifest::parse("{ \"id\": 3 }").unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err[0].message.contains("templates/game"));
    }

    #[test]
    fn catches_each_rule() {
        let mut m = base();
        m.id = "MeuJogo".into();
        m.name = "".into();
        m.tagline = "x".repeat(91);
        m.version = "um".into();
        m.players = Players { min: 3, max: 2, recommended: None };
        m.sdk.abi = 9;
        m.art.cover = "../capa.png".into();
        m.install.asset = "pacote.rar".into();
        m.launch.exe = "C:\\jogo.exe".into();
        let fields = errors(&m);
        for f in ["id", "name", "tagline", "version", "players", "sdk.abi", "art.cover", "install.asset", "launch.exe"] {
            assert!(fields.iter().any(|x| x == f), "faltou {f}: {fields:?}");
        }
    }

    #[test]
    fn base_game_mods_need_the_base_game_and_safe_files() {
        let mut m = base();
        m.install.target = InstallTarget::BaseGame;
        let fields = errors(&m);
        assert!(fields.contains(&"install.target".to_string()));

        m.requires = Some(Requires {
            base_game: Some(BaseGame {
                name: "GTA SA".into(),
                steam_app_id: Some(12120),
                epic_app_name: None,
                store_url: None,
                exe: "gta_sa.exe".into(),
                accepted_sha256: vec!["zz".into()],
                help: None,
            }),
        });
        m.install.files = vec![InstallFile { from: "SAParty.asi".into(), to: "../../Windows/x.dll".into() }];
        let fields = errors(&m);
        assert!(fields.contains(&"install.files[0].to".to_string()));
        assert!(fields.contains(&"requires.baseGame.acceptedSha256[0]".to_string()));
    }

    #[test]
    fn old_sha256_field_is_only_a_warning() {
        let json = TEMPLATE.replace("\"target\": \"self\"", "\"sha256\": \"abc\", \"target\": \"self\"");
        let (_, issues) = GameManifest::parse(&json).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!((issues[0].severity, issues[0].field.as_str()), (Severity::Warning, "install.sha256"));
    }

    #[test]
    fn version_with_v_prefix_works_like_the_tag() {
        let mut m = base();
        m.version = "v1.2.3".into();
        assert!(errors(&m).is_empty());
        assert_eq!(m.semver().unwrap(), semver::Version::new(1, 2, 3));
        assert_eq!(m.asset_name(), "meu-jogo-1.2.3.zip");
    }

    #[test]
    fn round_trips_through_json() {
        let m = base();
        let again: GameManifest = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, again);
    }
}
