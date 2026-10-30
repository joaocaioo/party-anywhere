use crate::issues::Issue;

pub const COVER_MIN_WIDTH: usize = 600;
pub const COVER_MIN_HEIGHT: usize = 800;
pub const COVER_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverInfo {
    pub width: usize,
    pub height: usize,
}

/// A capa é a primeira impressão na Biblioteca: retrato 3:4, no mínimo 600×800, até 2 MB.
pub fn check_cover(bytes: &[u8]) -> Result<CoverInfo, Issue> {
    let field = "art.cover";
    if bytes.len() > COVER_MAX_BYTES {
        return Err(Issue::error(field, format!("a capa tem {} KB; o máximo é 2 MB. Exporte em JPG ou WebP.", bytes.len() / 1024)));
    }
    let size = imagesize::blob_size(bytes).map_err(|_| Issue::error(field, "não consegui ler a imagem. Use PNG, JPG ou WebP."))?;
    let (w, h) = (size.width, size.height);
    if w < COVER_MIN_WIDTH || h < COVER_MIN_HEIGHT {
        return Err(Issue::error(field, format!("{w}×{h} é pequena; o mínimo é {COVER_MIN_WIDTH}×{COVER_MIN_HEIGHT} (ex.: 900×1200).")));
    }
    // 3:4 com 2% de tolerância
    let ratio = w as f64 / h as f64;
    if (ratio - 0.75).abs() > 0.015 {
        return Err(Issue::error(field, format!("{w}×{h} não é retrato 3:4. Ex.: 900×1200 ou 600×800.")));
    }
    Ok(CoverInfo { width: w, height: h })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cabeçalho PNG mínimo com as dimensões pedidas (o leitor só olha o IHDR).
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        v
    }

    #[test]
    fn accepts_portrait_three_by_four() {
        assert_eq!(check_cover(&png(900, 1200)), Ok(CoverInfo { width: 900, height: 1200 }));
        assert!(check_cover(&png(600, 800)).is_ok());
    }

    #[test]
    fn rejects_small_wrong_ratio_and_garbage() {
        assert!(check_cover(&png(300, 400)).unwrap_err().message.contains("pequena"));
        assert!(check_cover(&png(1920, 1080)).unwrap_err().message.contains("3:4"));
        assert!(check_cover(b"not an image").is_err());
        assert!(check_cover(&vec![0; COVER_MAX_BYTES + 1]).unwrap_err().message.contains("2 MB"));
    }
}
