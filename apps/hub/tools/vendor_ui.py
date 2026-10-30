"""Copia para apps/hub/ui/vendor/ o que a UI usaria da internet: Three.js (só os módulos usados), o gerador
de QR e as fontes. O Hub funciona sem internet e sem CDN.

Uso: python apps/hub/tools/vendor_ui.py   (precisa do npm; baixa as versões fixadas abaixo num temporário)
"""
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

THREE = "three@0.160.0"
QRCODE = "qrcode-generator@1.4.4"
# família -> pesos usados pela UI
FONTS = {
    "anton": [400],
    "bungee": [400],
    "caveat": [700],
    "doto": [900],
    "dotgothic16": [400],
    "inter": [400, 500, 600, 700, 800],
}
SUBSETS = ("latin", "latin-ext")
# módulos de three/addons importados pela UI (o resto vem pelas dependências)
ADDONS = [
    "postprocessing/EffectComposer.js",
    "postprocessing/RenderPass.js",
    "postprocessing/UnrealBloomPass.js",
    "postprocessing/OutputPass.js",
    "postprocessing/ShaderPass.js",
    "geometries/RoundedBoxGeometry.js",
    "lights/RectAreaLightUniformsLib.js",
]

UI = pathlib.Path(__file__).resolve().parents[1] / "ui"
OUT = UI / "vendor"


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        pkgs = [THREE, QRCODE] + [f"@fontsource/{f}@5" for f in FONTS]
        subprocess.run(
            ["npm", "install", "--ignore-scripts", "--no-audit", "--no-fund", "--prefix", tmp, *pkgs],
            check=True,
            shell=sys.platform == "win32",
        )
        nm = pathlib.Path(tmp) / "node_modules"
        if OUT.exists():
            shutil.rmtree(OUT)
        (OUT / "three/addons").mkdir(parents=True)
        (OUT / "fonts").mkdir()

        three = nm / "three"
        shutil.copy(three / "build/three.module.js", OUT / "three/three.module.js")
        shutil.copy(three / "LICENSE", OUT / "three/LICENSE")
        jsm = three / "examples/jsm"
        todo, seen = list(ADDONS), set()
        while todo:
            rel = todo.pop()
            if rel in seen:
                continue
            seen.add(rel)
            src = jsm / rel
            dst = OUT / "three/addons" / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            text = src.read_text(encoding="utf-8")
            dst.write_text(text, encoding="utf-8")
            for dep in re.findall(r"from\s+'(\.[^']+)'", text):
                todo.append((pathlib.PurePosixPath(rel).parent / dep).as_posix().replace("/./", "/"))
                todo[-1] = str(pathlib.PurePosixPath(*_normalize(todo[-1])))

        shutil.copy(nm / "qrcode-generator/qrcode.js", OUT / "qrcode.js")
        shutil.copy(nm / "qrcode-generator/LICENSE", OUT / "qrcode.LICENSE") if (nm / "qrcode-generator/LICENSE").exists() else None

        css = ["/* Fontes da UI (OFL), copiadas do @fontsource por tools/vendor_ui.py. */"]
        for family, weights in FONTS.items():
            pkg = nm / "@fontsource" / family
            for w in weights:
                block_css = (pkg / f"{w}.css").read_text(encoding="utf-8")
                for block in re.findall(r"/\* ([a-z0-9-]+) \*/\s*(@font-face\s*{[^}]*})", block_css):
                    name, face = block
                    if not any(name.endswith(f"-{s}-{w}-normal") for s in SUBSETS):
                        continue
                    woff2 = re.search(r"url\(\./files/([^)]+\.woff2)\)", face).group(1)
                    shutil.copy(pkg / "files" / woff2, OUT / "fonts" / woff2)
                    face = re.sub(r"src:[^;]*;", f"src: url(./fonts/{woff2}) format('woff2');", face)
                    css.append(f"/* {name} */\n{face}")
        (OUT / "fonts.css").write_text("\n".join(css) + "\n", encoding="utf-8")
        (OUT / "VERSIONS.json").write_text(
            json.dumps({"three": THREE, "qrcode": QRCODE, "fonts": FONTS}, indent=2) + "\n", encoding="utf-8"
        )
    print(f"ok: {sum(1 for _ in OUT.rglob('*') if _.is_file())} arquivos em {OUT}")


def _normalize(path: str) -> list:
    parts = []
    for p in path.split("/"):
        if p == "..":
            parts.pop()
        elif p not in ("", "."):
            parts.append(p)
    return parts


if __name__ == "__main__":
    main()
