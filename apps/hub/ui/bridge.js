// Ponte entre a UI e o núcleo do Hub (Rust). No navegador não faz nada: a UI fica em modo demonstração,
// com o painel "Demo". Dentro do app (Tauri), troca as simulações pelos dados reais: lobby, teclados,
// Biblioteca ligada ao GitHub, jogos do PC, jogar e atualizações. O visual não muda.
(() => {
  const T = window.__TAURI__;
  if (!T) return;
  const { invoke } = T.core;
  const { listen } = T.event;
  const warn = (e) => toast(String(e && e.message ? e.message : e), null, 'warn');

  document.documentElement.classList.add('in-app');

  // janela sem moldura: os três botões do topo e os cantos retos quando maximizada
  const win = T.window.getCurrentWindow();
  document.getElementById('winMin').addEventListener('click', () => win.minimize());
  document.getElementById('winMax').addEventListener('click', () => win.toggleMaximize());
  document.getElementById('winClose').addEventListener('click', () => win.close());
  const syncMax = () => win.isMaximized().then((m) => {
    document.documentElement.classList.toggle('maximized', m);
    document.getElementById('winMax').title = m ? 'Restaurar' : 'Maximizar';
  }).catch(() => {});
  win.onResized(syncMax);
  syncMax();
  document.getElementById('demo')?.remove();
  document.getElementById('demoToggle')?.remove();
  // o jogo abriu numa janela própria; esta tela só espera a volta
  document.getElementById('grText').innerHTML = 'O jogo abriu. Quando terminar, <b>Esc</b>, <b>B</b> ou <b>Start</b> volta para a biblioteca.';
  document.getElementById('grExit').textContent = 'Voltar para a biblioteca';

  // ------------------------------------------------------------------ lobby
  function applyLobby(v) {
    if (v.joinUrl && v.joinUrl !== JOIN_URL) {
      JOIN_URL = v.joinUrl;
      lobbyArc.querySelectorAll('.card.add').forEach((c) => { c.dataset.sig = ''; });
    }
    const next = new Array(MAX).fill(null);
    for (const s of v.seats) {
      next[s.index] = {
        index: s.index, color: s.color, name: s.name, kind: s.kind, brand: s.brand,
        deviceName: s.deviceName || ({ phone: 'Celular', keyboard: 'Teclado' }[s.kind] || 'Controle'),
        // a carta do teclado diferencia notebook, USB e Bluetooth pelo "via"
        via: s.kind === 'keyboard' ? ({ 'Do notebook': 'Notebook', 'Com fio': 'USB' }[s.via] || s.via) : (s.via || 'Conectado'),
        battery: s.battery, ping: s.kind === 'phone' ? (s.ping ?? '–') : null,
        deviceKey: s.deviceKey.replace(/[^a-z0-9]/gi, '').slice(-24), state: s.state,
      };
    }
    for (let i = 0; i < MAX; i++) {
      const a = seats[i], b = next[i];
      if (!b) continue;
      if (!a || a.deviceKey !== b.deviceKey) { if (b.state === 'active') toast(`P${i + 1} entrou · ${b.deviceName}`, b); }
      else if (a.state === 'reconnecting' && b.state === 'active') toast(`P${i + 1} voltou`, b);
      else if (a.state === 'active' && b.state === 'reconnecting') toast(`P${i + 1} caiu. Guardando a vaga...`, b, 'warn');
    }
    for (let i = 0; i < MAX; i++) seats[i] = next[i];
    renderLobby();
  }
  listen('lobby', (e) => applyLobby(e.payload));
  invoke('lobby_snapshot').then(applyLobby).catch(warn);

  // navegar o Hub com controle ou celular: as ações vêm do núcleo (qualquer jogador); a Gamepad API do
  // navegador fica desligada para o mesmo controle não navegar duas vezes
  window.__padNavExternal = true;
  listen('nav', (e) => navAction(e.payload.action, e.payload.color));

  // controles chegam pelo núcleo (gilrs)
  window.addPlayer = () => null;
  window.removeSeat = (idx) => {
    const old = seats[idx];
    if (!old) return;
    invoke('seat_remove', { index: idx }).catch(warn);
    toast(`P${idx + 1} saiu do lobby`, old, null, { label: 'Desfazer', fn: () => invoke('seat_undo_remove', { index: idx }).catch(warn) });
  };
  // Identificar: a carta pisca e o aviso diz o que aconteceu de verdade no aparelho
  window.identify = (idx) => {
    const s = seats[idx];
    if (!s) return;
    const el = (lobbyArc._order || []).find((c) => c.dataset.key.startsWith(`p${idx}-`));
    if (el) {
      el.classList.remove('identify'); void el.offsetWidth; el.classList.add('identify');
      clearTimeout(el._identifyT); el._identifyT = setTimeout(() => el.classList.remove('identify'), 950);
    }
    invoke('seat_identify', { index: idx }).then((what) => {
      const msg = {
        rumble: `${s.deviceName} de ${s.name} vibrando`,
        phone: `A tela do celular de ${s.name} está piscando na cor dele`,
        flash: s.kind === 'keyboard'
          ? `${s.name}: aperte uma tecla nesse teclado e o card pisca`
          : `${s.deviceName} não vibra pelo Hub ainda: o card de ${s.name} está piscando`,
      }[what];
      if (msg) toast(msg, s);
    }).catch(() => {});
  };
  const renameCard = startRename;
  window.startRename = (nameEl, card) => {
    renameCard(nameEl, card);
    const idx = +card.dataset.key.slice(1).split('-')[0];
    nameEl.addEventListener('blur', () => invoke('seat_rename', { index: idx, name: nameEl.textContent.trim() }).catch(warn), { once: true });
  };

  // ------------------------------------------------------------------ teclados
  window.openKeyboard = () => {
    openModal(`${head('Jogar no teclado', '<b>Cada teclado</b> vira um jogador: o do notebook, um USB ou um Bluetooth.', true)}
      <div class="kb-wait"><span class="kb-key">Enter</span><div><b>Aperte Enter no teclado que vai jogar</b><p>Ele entra no lobby na hora. Mais alguém? Enter no outro teclado.</p></div></div>
      <div class="found" id="kbList"><div class="found-empty">Procurando teclados...</div></div>`, () => {
      const off = backTo(() => openAddPlayer(true));
      const list = document.getElementById('kbList');
      const render = (kbs) => {
        if (!document.getElementById('kbList')) return;
        const full = seats.filter(Boolean).length >= MAX;
        list.innerHTML = kbs.length ? kbs.map((kb) => {
          const st = kb.seat != null ? seats[kb.seat] : null;
          const act = st ? `<span class="seat-tag" style="--c:${st.color}">P${kb.seat + 1}</span>`
            : full ? '<span class="state" style="color:var(--faint)">Lobby cheio</span>'
            : `<button class="btn-ghost sm" data-kb="${escapeHtml(kb.key)}">Conectar</button>`;
          return `<div class="dev${st ? ' ok' : ''}"><span class="ic kb-ic">${icon('keyboard')}</span><div class="t"><b>${escapeHtml(kb.deviceName)}</b><small>${escapeHtml(kb.via)}</small></div><div class="act">${act}</div></div>`;
        }).join('') : '<div class="found-empty">Aperte qualquer tecla no teclado que vai jogar para ele aparecer aqui.</div>';
      };
      const click = (e) => { const b = e.target.closest('[data-kb]'); if (b) invoke('keyboard_add', { key: b.dataset.kb }).catch(warn); };
      list.addEventListener('click', click);
      const unlisten = listen('keyboards', (e) => render(e.payload));
      const relisten = listen('lobby', () => invoke('keyboards_list').then(render));
      invoke('keyboards_listen', { on: true });
      invoke('keyboards_list').then(render).catch(warn);
      return () => { off(); invoke('keyboards_listen', { on: false }); unlisten.then((f) => f()); relisten.then((f) => f()); };
    });
  };

  // ------------------------------------------------------------------ entrar pelo celular
  // os celulares que entram de verdade aparecem na lista; sem o simulador do modo demonstração; "Não conecta?"
  // mostra os endereços reais deste PC
  window.openPhone = () => {
    const full = () => seats.filter(Boolean).length >= MAX;
    const waitingText = () => full()
      ? '<span class="pulse warn"></span>Lobby cheio (10 de 10). Para entrar mais alguém, remova um jogador.'
      : '<span class="pulse"></span>Aguardando celulares. Dá para entrar vários ao mesmo tempo.';
    openModal(`${head('Entrar pelo celular', '<b>Cada pessoa</b> escaneia com a câmera do próprio celular.', true)}
      <div class="phone-wrap">
        <div>
          <div class="qr-big" id="phoneQr">${qrSvg(JOIN_URL)}</div>
          <div class="url" id="phoneUrl">${escapeHtml(JOIN_URL.replace('http://', ''))}</div>
        </div>
        <div>
          <ul class="steps">
            <li><span class="n">1</span><span>Use o <b>mesmo Wi-Fi</b> do PC</span></li>
            <li><span class="n">2</span><span>Aponte a câmera para o <b>QR code</b></span></li>
            <li><span class="n">3</span><span>Pronto: você entra no lobby <b>com a sua cor</b></span></li>
          </ul>
          <div class="waiting" id="phoneWaiting">${waitingText()}</div>
          <div class="joined-list" id="joinedList"></div>
          <div style="margin-top:18px"><button class="linkish" id="altIps">Não conecta?</button></div>
          <div id="altBox"></div>
        </div>
      </div>`, () => {
      const off = backTo(() => openAddPlayer(true));
      // quem já estava no lobby não aparece como "entrou"
      const seen = new Set(seats.filter((x) => x && x.kind === 'phone').map((x) => x.deviceKey + x.index));
      const onLobby = () => {
        const list = document.getElementById('joinedList');
        if (!list) return;
        for (const x of seats) {
          if (!x || x.kind !== 'phone' || x.state !== 'active' || seen.has(x.deviceKey + x.index)) continue;
          seen.add(x.deviceKey + x.index);
          list.insertAdjacentHTML('beforeend', `<span class="joined"><i style="background:${x.color}">${x.index + 1}</i>${escapeHtml(x.name)} entrou como P${x.index + 1}</span>`);
        }
        document.getElementById('phoneWaiting').innerHTML = waitingText();
      };
      // depois do applyLobby (que atualiza os seats), não antes
      const unlisten = listen('lobby', () => setTimeout(onLobby, 0));
      document.getElementById('altIps').addEventListener('click', async () => {
        const box = document.getElementById('altBox');
        let addrs = [];
        try { addrs = await invoke('network_addresses'); } catch (e) { warn(e); }
        box.innerHTML = `<div class="modal-note" style="display:block">
          <b style="color:var(--text)">Confira estas três coisas:</b><br>
          • O celular está no <b>mesmo Wi-Fi</b>? Redes de convidados costumam bloquear celulares de falarem com o PC.<br>
          • O Windows perguntou sobre a rede quando o Hub abriu? Permita em <b>redes privadas</b>.<br>
          • O PC tem mais de uma rede? Tente outro endereço (toque para trocar o QR):
          <div class="alt-ips">${addrs.length ? addrs.map((a) => `<span class="linkish" data-url="${escapeHtml(a.url)}">${escapeHtml(a.url.replace('http://', ''))}</span> (${escapeHtml(a.interface)})${a.current ? ' ✓' : ''}`).join('<br>') : 'Nenhuma rede encontrada: o PC está conectado?'}</div></div>`;
        box.querySelectorAll('[data-url]').forEach((el) => el.addEventListener('click', () => {
          document.getElementById('phoneQr').innerHTML = qrSvg(el.dataset.url);
          document.getElementById('phoneUrl').textContent = el.dataset.url.replace('http://', '');
        }));
      });
      return () => { off(); unlisten.then((f) => f()); };
    });
  };

  // ------------------------------------------------------------------ Controles e Ajustes: dados do núcleo
  Object.assign(hubData, {
    devices: () => invoke('devices_list'),
    layouts: () => invoke('layouts_list'),
    saveLayout: (id, name, controls) => invoke('layout_save', { id, name, controls }),
    deleteLayout: (id) => invoke('layout_delete', { id }),
    settings: () => invoke('settings_get'),
    setJoinPolicy: (on) => invoke('set_join_policy', { onButton: on }),
    addresses: () => invoke('network_addresses'),
    setJoinIp: (ip) => invoke('set_join_ip', { ip }),
    installVirtual: () => invoke('virtual_pads_install'),
    retryVirtual: () => invoke('virtual_pads_retry'),
    checkUpdate: () => invoke('hub_check_update').then((u) => {
      if (!u) toast('Você já está na versão mais nova.', null, 'ok');
    }),
  });

  // ------------------------------------------------------------------ Bluetooth (pareamento próprio vem na fase 3)
  window.openBluetooth = () => {
    openModal(`${head('Controle Bluetooth', 'Pareie uma vez no Windows; depois ele entra sozinho no lobby.', true)}
      <div class="bt-off">
        <div class="core">${icon('bt')}</div>
        <h3>Coloque o controle em modo de pareamento</h3>
        <p>PlayStation: segure <b>Share + PS</b>. Xbox: segure o botão de <b>parear</b>. Switch Pro: segure o botão <b>sync</b>.<br>
        Na janela do Windows, escolha o controle. Ele aparece aqui no lobby assim que conectar.</p>
        <button class="btn-neon" id="btWin">${icon('bt')}Abrir o Bluetooth do Windows</button>
      </div>`, () => {
      const off = backTo(() => openAddPlayer(true));
      document.getElementById('btWin').addEventListener('click', () => invoke('open_bluetooth_settings').catch(warn));
      return off;
    });
  };

  // ------------------------------------------------------------------ Biblioteca
  const known = new Map(); // id → jogo como o núcleo conhece (para mandar só como abrir os novos do PC)
  // arte da Steam vem como referência ("steam:331600"): a capa e a arte larga (hero) são lidas sob demanda,
  // e o library.json guarda só a referência
  const artCache = new Map();
  const art = (ref, hero) => {
    const key = `${ref}|${hero}`;
    if (!artCache.has(key)) artCache.set(key, invoke('game_art', { reference: ref, hero }).catch(() => null));
    return artCache.get(key);
  };
  async function withArt(g) {
    if (!g.image || !g.image.startsWith('steam:')) return g;
    const [cover, hero] = await Promise.all([art(g.image, false), art(g.image, true)]);
    return { ...g, artRef: g.image, image: cover || undefined, artResolved: cover, hero: hero || undefined };
  }
  async function setGames(list, focusId) {
    known.clear();
    list.forEach((g) => known.set(g.id, g));
    const resolved = await Promise.all(list.map(withArt));
    const mapped = resolved.map((g) => ({ ...g, image: g.image || undefined, tags: g.update ? [...g.tags, `Nova versão ${g.update}`] : g.tags }));
    games.splice(0, games.length, ...mapped);
    if (arcade) {
      arcade.setGames(games, coverSvg);
      if (focusId) arcade.focus(Math.max(0, games.findIndex((g) => g.id === focusId)));
    } else if (currentView === 'library') renderLibrary();
  }
  // a UI chama saveLibrary depois de qualquer mudança: o núcleo recebe a lista e aplica a diferença
  window.saveLibrary = () => {
    const payload = games.map((g) => {
      const pc = PC_GAMES.find((p) => p.id === g.id);
      // arte que não foi editada volta como referência (steam:...), não como a imagem inteira
      const ref = g.artRef ?? pc?.artRef;
      const image = ref && (g.image === g.artResolved || g.image === pc?.artResolved) ? ref : (g.image || null);
      return { id: g.id, name: g.name, tagline: g.tagline || '', players: g.players || '', image, store: pc?.store ?? g.store ?? null, launch: pc?.launch ?? null };
    });
    invoke('library_sync', { games: payload }).then((list) => {
      list.forEach((g) => known.set(g.id, g));
      // jogo novo da Steam: recarrega para ganhar a arte larga (hero) na tela do fliperama
      const fresh = list.find((b) => b.image?.startsWith('steam:') && !games.find((g) => g.id === b.id)?.artRef);
      if (fresh) setGames(list, fresh.id);
    }).catch(warn);
  };
  window.forgetLibrary = () => {};
  invoke('library_list').then((list) => setGames(list)).catch(warn);
  window.__arcadeReady.then(() => invoke('library_list').then((list) => setGames(list))).catch(() => {});

  // Jogos deste PC: a varredura real (Steam e Epic) antes de abrir a lista
  const scanList = openScanGames;
  window.openScanGames = async () => {
    try {
      const found = await invoke('pc_scan');
      const withCovers = await Promise.all(found.map(async (g) => {
        if (!g.image || !g.image.startsWith('steam:')) return { ...g, art: palette(g.id) };
        const cover = await art(g.image, false);
        return { ...g, art: palette(g.id), artRef: g.image, image: cover || undefined, artResolved: cover };
      }));
      PC_GAMES.splice(0, PC_GAMES.length, ...withCovers);
    } catch (e) { warn(e); }
    scanList();
  };
  // Escolher um .exe: a janela de arquivo do Windows
  window.openPickGame = async () => {
    const g = await invoke('pick_game_file').catch(warn);
    if (!g) return;
    PC_GAMES.push({ ...g, art: palette(g.id) });
    addToLibrary([toLibraryGame({ ...g, art: palette(g.id) }, 'PC')]);
  };

  // Por link: a leitura de verdade no GitHub
  let lastReport = null;
  window.readManifest = async (url) => {
    const out = document.getElementById('linkResult');
    out.innerHTML = `<div class="progress-lbl" style="margin-top:18px;display:flex;align-items:center;gap:8px">${icon('spinner').replace('<svg', '<svg width="16" height="16"')}Lendo o jogo no GitHub...</div>`;
    let r;
    try { r = await invoke('library_inspect', { url }); } catch (e) {
      out.innerHTML = `<div class="errbox">${icon('warn')}<div>${escapeHtml(String(e))}</div></div>`;
      return;
    }
    lastReport = r;
    const errors = r.issues.filter((i) => i.severity === 'error');
    if (!r.manifest || !r.installable) {
      out.innerHTML = `<div class="errbox">${icon('warn')}<div><b>${escapeHtml(r.summary)}</b><br>${errors.map((i) => escapeHtml(i.message)).join('<br>')}</div></div>`;
      return;
    }
    const m = r.manifest;
    const g = { id: m.id, name: m.name, tagline: m.tagline, players: `${m.players.min === m.players.max ? m.players.max : `${m.players.min} a ${m.players.max}`} jogadores`, art: palette(m.id), image: r.coverUrl, repo: r.repo };
    const mb = Math.max(0.1, r.latest.release.size / 1048576).toFixed(1).replace('.', ',');
    const base = m.requires && m.requires.baseGame;
    out.innerHTML = `<div class="preview">
      <div class="pc">${g.image ? `<img src="${escapeHtml(g.image)}" alt="" style="width:100%;height:100%;object-fit:cover;border-radius:inherit" onerror="this.replaceWith(Object.assign(document.createElement('div'),{innerHTML:coverSvg(${escapeHtml(JSON.stringify({ id: g.id, name: g.name, art: g.art }))})}).firstChild)">` : coverSvg(g)}</div>
      <div>
        <h3>${escapeHtml(m.name)}</h3>
        <div class="by">por ${escapeHtml((m.authors && m.authors[0]) || r.repo.split('/')[0])} · versão ${escapeHtml(r.latest.release.version)}</div>
        <div class="facts">
          <div>${icon('users')}${escapeHtml(g.players)}</div>
          <div>${icon('download')}Download de ${mb} MB, conferido pelo hash</div>
          ${base ? `<div>${icon('check')}Precisa de ${escapeHtml(base.name)} (a gente procura na sua Steam)</div>` : `<div>${icon('check')}Não precisa de outro jogo</div>`}
        </div>
        <div class="warnbox">${icon('warn')}<div>Jogo da comunidade, ainda não verificado pela equipe Party Anywhere. Instale só se confiar no autor.</div></div>
        <div style="margin-top:16px;display:flex;gap:10px"><button class="btn-neon" id="installBtn">${icon('download')}Instalar</button><button class="btn-ghost" data-close>Cancelar</button></div>
        <div id="installProg"></div>
      </div></div>`;
    document.getElementById('installBtn').addEventListener('click', () => install(g));
  };

  window.install = async (g) => {
    const box = document.getElementById('installProg');
    const btn = document.getElementById('installBtn');
    if (btn) btn.disabled = true;
    if (box) box.innerHTML = `<div class="progress"><i id="pbar" style="width:35%"></i></div><div class="progress-lbl" id="plbl">Baixando e conferindo o pacote...</div>`;
    try {
      const list = await invoke('library_install', { url: g.repo || (lastReport && lastReport.repo) });
      closeModal();
      if (currentView !== 'library') await switchView('library');
      setGames(list, g.id);
      toast(`${g.name} está na sua biblioteca`, null, 'ok');
    } catch (e) {
      if (box) box.innerHTML = `<div class="errbox">${icon('warn')}<div>${escapeHtml(String(e))}</div></div>`;
      if (btn) btn.disabled = false;
    }
  };

  // Jogar: abre o jogo de verdade; a animação do fliperama só roda se ele abriu
  const playCinematic = playGame;
  window.playGame = async (g) => {
    if (!g || gameRunning || switching) return;
    try { await invoke('game_play', { id: g.id }); } catch (e) { warn(e); return; }
    return playCinematic(g);
  };
  window.launch = (g) => invoke('game_play', { id: g.id }).then(() => toast(`Abrindo ${g.name}...`, null, 'ok')).catch(warn);

  // ------------------------------------------------------------------ atualizações
  let hubUpdate = null;
  updatePill.addEventListener('click', (e) => {
    e.stopImmediatePropagation();
    if (hubUpdate) {
      toast(`Baixando a v${hubUpdate.version}... O Hub reabre sozinho.`, null, 'ok');
      invoke('hub_apply_update', { update: hubUpdate }).catch(warn);
    } else {
      updateState = 'checking'; renderUpdate();
      invoke('hub_check_update').then((u) => {
        if (!u) { updateState = 'uptodate'; renderUpdate(); toast('Você já está na versão mais nova.', null, 'ok'); }
      }).catch((err) => { updateState = 'uptodate'; renderUpdate(); warn(err); });
    }
  }, true);
  listen('hub-update', (e) => {
    hubUpdate = e.payload;
    updateState = 'ready'; renderUpdate();
    updatePill.title = `Party Anywhere v${hubUpdate.version} disponível. Clique para atualizar.`;
  });
  listen('game-updates', (e) => {
    for (const u of e.payload) {
      const g = known.get(u.id);
      toast(`${u.name}: versão ${u.to} disponível`, null, 'ok', g && g.repo ? { label: 'Atualizar', fn: () => install({ ...g, repo: g.repo }) } : undefined);
    }
    invoke('library_list').then((list) => setGames(list)).catch(() => {});
  });
  listen('library-restored', () => toast('A biblioteca foi recuperada do último backup.', null, 'warn'));
  invoke('hub_info').then((info) => { hubVersion = info.version; updateState = 'uptodate'; renderUpdate(); }).catch(() => {});

  function palette(id) {
    const P = [['#4c1d95', '#ec4899', '#f97316'], ['#082f49', '#06b6d4', '#facc15'], ['#14532d', '#22c55e', '#fde047'], ['#450a0a', '#dc2626', '#f59e0b'], ['#1e1b4b', '#a855f7', '#22c55e'], ['#064e3b', '#10b981', '#a3e635']];
    let h = 7;
    for (const c of id) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return P[h % 6];
  }
})();
