"""Grava os clipes do README (docs/media/*.webp) a partir do modo demonstração do Hub, num Edge invisível
(CDP, screencast). Precisa da UI servida em http://localhost:5173 e do pacote websocket-client.

    python apps/hub/tools/record_readme.py docs/media/lobby.webp lobby
    python apps/hub/tools/record_readme.py docs/media/library.webp library

Só jogos de exemplo originais: as capas de exemplo (sample-art, com direitos autorais) ficam de fora.
"""
import base64, io, json, os, subprocess, sys, time, urllib.request
import websocket
from PIL import Image

OUT = sys.argv[1]
WHICH = sys.argv[2]  # lobby | library
SCR = os.path.join(os.environ.get('TEMP', '.'), 'pa-record')
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
PORT = 9333
W, H = 1280, 760

prof = os.path.join(SCR, 'edge-rec-' + WHICH)
proc = subprocess.Popen([EDGE, '--headless=new', f'--remote-debugging-port={PORT}', f'--user-data-dir={prof}',
                         f'--window-size={W},{H}', '--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--enable-gpu-rasterization',
                         '--hide-scrollbars', '--mute-audio', 'about:blank'])
try:
    for _ in range(50):
        try:
            tabs = json.load(urllib.request.urlopen(f'http://127.0.0.1:{PORT}/json'))
            page = next(t for t in tabs if t['type'] == 'page')
            break
        except Exception:
            time.sleep(0.2)
    ws = websocket.create_connection(page['webSocketDebuggerUrl'], timeout=60, suppress_origin=True)
    n = [0]
    frames = []
    def handle(m):
        # quadro do screencast: guarda e confirma (sem a confirmação o navegador para de mandar)
        if m.get('method') == 'Page.screencastFrame':
            p = m['params']
            frames.append((p['metadata']['timestamp'], p['data']))
            n[0] += 1
            ws.send(json.dumps({'id': n[0], 'method': 'Page.screencastFrameAck', 'params': {'sessionId': p['sessionId']}}))
    def cdp(method, **params):
        n[0] += 1
        my = n[0]
        ws.send(json.dumps({'id': my, 'method': method, 'params': params}))
        while True:
            m = json.loads(ws.recv())
            if m.get('id') == my:
                if 'error' in m:
                    raise RuntimeError(m['error'])
                return m.get('result', {})
            handle(m)
    def pump(seconds):
        end = time.time() + seconds
        ws.settimeout(0.05)
        while time.time() < end:
            try:
                handle(json.loads(ws.recv()))
            except websocket.WebSocketTimeoutException:
                pass
        ws.settimeout(60)
    def js(code):
        r = cdp('Runtime.evaluate', expression=code, awaitPromise=True, returnByValue=True)
        if 'exceptionDetails' in r:
            raise RuntimeError(r['exceptionDetails'])
        return r.get('result', {}).get('value')
    cdp('Emulation.setDeviceMetricsOverride', width=W, height=H, deviceScaleFactor=1, mobile=False)
    cdp('Page.enable')
    cdp('Page.navigate', url='http://localhost:5173/')
    time.sleep(3)
    # sem o painel Demo e sem as capas de exemplo (arte com direitos autorais)
    js("""(async () => {
          document.getElementById('demoToggle')?.style.setProperty('display','none');
          document.getElementById('demo')?.style.setProperty('display','none');
          if (window.__arcadeReady) await window.__arcadeReady;
          const ok = (g) => !/^(sa|op4|pc-gta-sa|pc-op4)$/.test(g.id) && !/san andreas|one piece|grand theft/i.test(g.name) && !String(g.image || '').includes('sample-art');
          games.splice(0, games.length, ...games.filter(ok));
          PC_GAMES.splice(0, PC_GAMES.length, ...PC_GAMES.filter(ok));
          if (arcade) { arcade.setGames(games, coverSvg); arcade.focus(0); }
          return games.map(g => g.name).join(', ');
        })()""")
    started = [False]
    def grab(seconds):
        if not started[0]:
            cdp('Page.startScreencast', format='jpeg', quality=85, everyNthFrame=1)
            started[0] = True
        pump(seconds)
    def demo(a):
        js(f"document.querySelector('[data-demo={a}]').click(); 1")

    if WHICH == 'lobby':
        grab(1.2)
        for a in ['usb', 'phone', 'keyboard', 'phone', 'usb', 'phone', 'keyboard', 'phone', 'usb', 'phone']:
            demo(a)
            grab(1.0)
        grab(2.0)
    else:
        for a in ['usb', 'phone', 'keyboard', 'phone', 'usb', 'phone', 'keyboard', 'phone', 'usb']:
            demo(a)
        time.sleep(2.5)
        js("document.querySelector('[data-view=library]').click(); 1")
        grab(3.0)
        for d in ['right', 'right', 'left']:
            js(f"navAction('{d}', seats[0].color, 0); 1")
            grab(1.5)
        js("navAction('a', seats[0].color, 0); 1")  # jogar o Kart Caos (até 8): 9 no lobby
        grab(3.4)
        for s in [0, 1, 3, 5, 6]:
            js(f"navAction('a', seats[{s}].color, {s}); 1")
            grab(0.8)
        grab(2.5)
    ws.close()
finally:
    subprocess.run(['taskkill', '/PID', str(proc.pid), '/T', '/F'], capture_output=True)

# GIF: 800 px, no máximo 12 quadros por segundo, tempo real entre eles
WIDTH, STEP = 800, 1 / 12
picked = []
for t, data in frames:
    if not picked or t - picked[-1][0] >= STEP:
        picked.append((t, data))
out = []
for i, (t, data) in enumerate(picked):
    img = Image.open(io.BytesIO(base64.b64decode(data))).convert('RGB')
    dur = int(((picked[i + 1][0] if i + 1 < len(picked) else t + 2.0) - t) * 1000)
    out.append((img.resize((WIDTH, int(WIDTH * img.height / img.width)), Image.LANCZOS), max(40, dur)))
if OUT.endswith('.webp'):
    # WebP animado: bem menor que GIF com o 3D e os degradês, e sem faixas de cor
    out[0][0].save(OUT, save_all=True, append_images=[im for im, _ in out[1:]], duration=[d for _, d in out], loop=0, quality=72, method=6)
else:
    pal = [im.quantize(colors=128, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE) for im, _ in out]
    pal[0].save(OUT, save_all=True, append_images=pal[1:], duration=[d for _, d in out], loop=0, optimize=True)
print(len(frames), 'recebidos,', len(out), 'quadros', os.path.getsize(OUT) // 1024, 'KB')
