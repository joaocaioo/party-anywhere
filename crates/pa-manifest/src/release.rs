use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::game::is_sha256;
use crate::issues::Issue;
use crate::{is_valid_id, GameManifest, MANIFEST_VERSION};

/// `party-anywhere.release.json`: anexado a cada release pelo `pa pack` (e pelo workflow do template).
/// É o que o Hub lê para instalar e para avisar que há versão nova.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseManifest {
    pub manifest_version: u32,
    pub id: String,
    pub version: String,
    /// Nome do pacote anexado ao mesmo release.
    pub asset: String,
    pub sha256: String,
    pub size: u64,
    pub sdk_abi: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_hub_version: Option<String>,
}

impl ReleaseManifest {
    /// Monta a partir do manifesto do jogo e dos bytes do pacote gerado.
    pub fn for_package(game: &GameManifest, package: &[u8]) -> Self {
        Self {
            manifest_version: MANIFEST_VERSION,
            id: game.id.clone(),
            version: game.version.trim_start_matches('v').to_string(),
            asset: game.asset_name(),
            sha256: sha256_hex(package),
            size: package.len() as u64,
            sdk_abi: game.sdk.abi,
            min_hub_version: game.min_hub_version.clone(),
        }
    }

    pub fn semver(&self) -> Option<semver::Version> {
        semver::Version::parse(self.version.trim_start_matches('v')).ok()
    }

    /// Confere este anexo contra o manifesto do repositório (mesmo jogo) e a tag do release.
    pub fn validate(&self, game: &GameManifest, tag: Option<&str>) -> Vec<Issue> {
        let mut out = Vec::new();
        if self.manifest_version != MANIFEST_VERSION {
            out.push(Issue::error("manifestVersion", "versão do anexo desconhecida para este Hub."));
        }
        if !is_valid_id(&self.id) || self.id != game.id {
            out.push(Issue::error("id", format!("o release diz \"{}\", mas o repositório é \"{}\".", self.id, game.id)));
        }
        if self.semver().is_none() {
            out.push(Issue::error("version", "versão fora do semver."));
        }
        if let Some(tag) = tag {
            if tag.trim_start_matches('v') != self.version.trim_start_matches('v') {
                out.push(Issue::error("version", format!("a tag é {tag}, mas o anexo diz {}.", self.version)));
            }
        }
        if !is_sha256(&self.sha256) {
            out.push(Issue::error("sha256", "64 caracteres hexadecimais."));
        }
        if self.size == 0 {
            out.push(Issue::error("size", "pacote vazio."));
        }
        out
    }

    /// O pacote baixado é exatamente o publicado?
    pub fn verify(&self, package: &[u8]) -> bool {
        package.len() as u64 == self.size && sha256_hex(package).eq_ignore_ascii_case(&self.sha256)
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> GameManifest {
        GameManifest::parse(include_str!("../../../templates/game/party-anywhere.json")).unwrap().0
    }

    #[test]
    fn known_hash() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn release_for_a_package_verifies_and_rejects_tampering() {
        let g = game();
        let r = ReleaseManifest::for_package(&g, b"zip bytes");
        assert_eq!((r.asset.as_str(), r.size), ("meu-jogo-0.1.0.zip", 9));
        assert!(r.validate(&g, Some("v0.1.0")).is_empty());
        assert!(r.verify(b"zip bytes"));
        assert!(!r.verify(b"zip bytez"));
    }

    #[test]
    fn mismatched_tag_or_game_is_an_error() {
        let g = game();
        let mut r = ReleaseManifest::for_package(&g, b"x");
        assert_eq!(r.validate(&g, Some("v0.2.0")).len(), 1);
        r.id = "com.outro.jogo".into();
        assert!(r.validate(&g, None).iter().any(|i| i.field == "id"));
    }
}
