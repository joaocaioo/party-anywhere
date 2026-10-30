use std::fs;
use std::path::Path;

/// O app empacota uma cópia da UI sem `sample-art/` (capas de jogos reais, só para o modo demonstração no
/// navegador; têm direitos autorais e nunca podem ir no instalador).
fn stage_ui(src: &Path, dst: &Path) {
    let _ = fs::remove_dir_all(dst);
    copy_dir(src, dst);
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("criar a pasta da UI empacotada");
    for entry in fs::read_dir(src).expect("ler apps/hub/ui").flatten() {
        let name = entry.file_name();
        if name == "sample-art" {
            continue;
        }
        let (from, to) = (entry.path(), dst.join(&name));
        if from.is_dir() {
            copy_dir(&from, &to);
        } else {
            fs::copy(&from, &to).expect("copiar a UI");
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=../ui");
    stage_ui(Path::new("../ui"), Path::new("../ui-dist"));
    tauri_build::build()
}
