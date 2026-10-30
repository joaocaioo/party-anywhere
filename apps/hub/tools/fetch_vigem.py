"""Baixa o instalador do ViGEmBus que o instalador do Hub leva junto (.downloads/vigem/), conferindo o hash.

A versão e o hash são os de crates/pa-library/src/vigem.rs. Rodado pelo workflow de release antes do
`npx tauri build --config tauri.release.conf.json`.
"""
import hashlib
import pathlib
import re
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[3]
src = (ROOT / "crates/pa-library/src/vigem.rs").read_text(encoding="utf-8")
url = re.search(r'pub const URL: &str = "([^"]+)"', src).group(1)
sha = re.search(r'pub const SHA256: &str = "([0-9a-f]{64})"', src).group(1)
name = re.search(r'pub const FILE: &str = "([^"]+)"', src).group(1)

out = ROOT / ".downloads/vigem" / name
if not (out.exists() and hashlib.sha256(out.read_bytes()).hexdigest() == sha):
    out.parent.mkdir(parents=True, exist_ok=True)
    data = urllib.request.urlopen(url, timeout=120).read()
    got = hashlib.sha256(data).hexdigest()
    if got != sha:
        raise SystemExit(f"hash do ViGEmBus não confere: {got}")
    out.write_bytes(data)
print(f"ok: {out}")
