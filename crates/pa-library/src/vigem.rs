//! O componente dos controles virtuais (ViGEmBus, BSD-3, da Nefarius): a versão que o Hub instala, fixa e
//! conferida pelo hash. O instalador do Hub leva este mesmo arquivo (apps/hub/tools/fetch_vigem.py) e o
//! botão "Instalar" do app baixa este mesmo endereço.

use pa_manifest::sha256_hex;

pub const VERSION: &str = "1.22.0";
pub const URL: &str = "https://github.com/nefarius/ViGEmBus/releases/download/v1.22.0/ViGEmBus_1.22.0_x64_x86_arm64.exe";
pub const SHA256: &str = "89220a7865076b342892f98865f3499fb7c4cfd673159e89d352c360fd014c6a";
pub const FILE: &str = "ViGEmBus_1.22.0_x64_x86_arm64.exe";

/// O arquivo baixado é exatamente o esperado?
pub fn verify(bytes: &[u8]) -> Result<(), String> {
    if sha256_hex(bytes) == SHA256 {
        Ok(())
    } else {
        Err("o instalador baixado não confere com o esperado (nada foi aberto).".into())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_anything_else() {
        assert!(super::verify(b"MZ outro instalador").is_err());
    }
}
