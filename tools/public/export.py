"""Gera o repositório público a partir deste (privado), só com o produto.

    python tools/public/export.py <pasta do repositório público>

Copia o que está commitado em HEAD (nunca o que está só no disco), tira o que é interno (processo, spikes,
docs internos, instruções de agente deste repositório), ajusta o workspace sem os spikes e confere que nada
pessoal ou interno vazou. A pasta de destino é um repositório git à parte, com o histórico próprio: este
script não faz commit nem push, só deixa a árvore pronta para conferir.

Este arquivo fica só no privado.
"""
import io
import os
import re
import subprocess
import sys
import tarfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# o que é do processo, não do produto (CLAUDE.md, "Repositório e publicação")
EXCLUDE = [
    'CLAUDE.md',
    'docs/VISION.md',
    'docs/ROADMAP.md',
    'docs/UX.md',
    'docs/ARCHITECTURE.md',
    'docs/GUIA-PRIMEIRO-MOD.md',
    'spikes/',
    'medir-latencia.bat',
    '.claude/',          # só o da raiz; o do template de jogo (templates/game/.claude) é produto
    'tools/public/',
]

PATCHES = {
    'Cargo.toml': [('    "spikes/latency",\n', '')],
    '.gitignore': [('/spikes/latency/reports/*.json\n', '')],
}

# nada disto pode aparecer no público
FORBIDDEN = [
    (re.compile(r'voce', re.I), 'nome do usuário do PC'),
    (re.compile(r'gmail\.com', re.I), 'e-mail pessoal'),
    (re.compile(r'[A-Z]:\\Users\\', re.I), 'caminho pessoal do Windows'),
    (re.compile(r'SACoop', re.I), 'referência ao mod de referência do dono'),
    (re.compile(r'(VISION|ROADMAP|UX|ARCHITECTURE|GUIA-PRIMEIRO-MOD)\.md'), 'link para doc interno'),
    (re.compile(r'spikes/'), 'referência aos spikes'),
    (re.compile(r'(ghp_|github_pat_)[A-Za-z0-9_]{20,}'), 'token do GitHub'),
]
SKIP_SCAN = ('apps/hub/ui/vendor/',)
BIG_FILE = 3 * 1024 * 1024


def excluded(path):
    return any(path == e or (e.endswith('/') and path.startswith(e)) for e in EXCLUDE)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    dest = os.path.abspath(sys.argv[1])
    if os.path.commonpath([dest, ROOT]) == ROOT:
        sys.exit('a pasta do público precisa ficar fora deste repositório')
    if not os.path.isdir(os.path.join(dest, '.git')):
        sys.exit(f'{dest} não é um repositório git (crie com git init)')

    tar = subprocess.run(['git', '-C', ROOT, 'archive', '--format=tar', 'HEAD'], check=True, capture_output=True).stdout
    files = {}
    with tarfile.open(fileobj=io.BytesIO(tar)) as t:
        for m in t.getmembers():
            if m.isfile() and not excluded(m.name):
                files[m.name] = t.extractfile(m).read()

    for name, pairs in PATCHES.items():
        text = files[name].decode('utf-8')
        for a, b in pairs:
            if a not in text:
                sys.exit(f'{name}: o ajuste esperado não bate mais ({a.strip()!r}); atualize o export.py')
            text = text.replace(a, b)
        files[name] = text.encode('utf-8')

    problems = []
    for name, data in files.items():
        if len(data) > BIG_FILE:
            problems.append(f'{name}: arquivo grande ({len(data) // 1024} KB)')
        if 'sample-art/' in name:
            problems.append(f'{name}: capa de jogo real (direitos autorais)')
        if name.startswith(SKIP_SCAN) or b'\0' in data[:8192]:
            continue
        text = data.decode('utf-8', errors='replace')
        for rx, why in FORBIDDEN:
            for m in rx.finditer(text):
                line = text.count('\n', 0, m.start()) + 1
                problems.append(f'{name}:{line}: {why} ({m.group(0)})')
    if problems:
        sys.exit('Nada foi copiado. Resolva no privado, faça commit e rode de novo:\n  ' + '\n  '.join(problems))

    # espelha: o que não existe mais no produto sai do público também
    for dirpath, dirnames, filenames in os.walk(dest, topdown=True):
        dirnames[:] = [d for d in dirnames if not (dirpath == dest and d in ('.git', 'target'))]
        for f in filenames:
            rel = os.path.relpath(os.path.join(dirpath, f), dest).replace(os.sep, '/')
            if rel not in files:
                os.remove(os.path.join(dirpath, f))
    for name, data in files.items():
        out = os.path.join(dest, name)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, 'wb') as f:
            f.write(data)
    for dirpath, dirnames, filenames in os.walk(dest, topdown=False):
        if '.git' not in dirpath.split(os.sep) and not os.listdir(dirpath) and dirpath != dest:
            os.rmdir(dirpath)

    # sem os spikes no workspace, o Cargo.lock perde as entradas deles (sem mudar versão de nada)
    subprocess.run(['cargo', 'metadata', '--format-version', '1', '--offline'], cwd=dest, check=True, stdout=subprocess.DEVNULL)
    head = subprocess.run(['git', '-C', ROOT, 'rev-parse', '--short', 'HEAD'], check=True, capture_output=True, text=True).stdout.strip()
    print(f'Pronto: {len(files)} arquivos de {head} em {dest}. Confira com git status e rode os testes lá.')


if __name__ == '__main__':
    main()
