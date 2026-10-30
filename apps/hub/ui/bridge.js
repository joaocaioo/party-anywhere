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
  document.getElementById('demo')?.remove();
  document.getElementById('demoToggle')?.remove();

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

  // controles chegam pelo núcleo (gilrs); a Gamepad API do navegador fica só para navegar
  window.addPlayer = () => null;
  window.removeSeat = (idx) => {
    const old = seats[idx];
    if (!old) return;
    invoke('seat_remove', { index: idx }).catch(warn);
    toast(`P${idx + 1} saiu do lobby`, old, null, { label: 'Desfazer', fn: () => invoke('seat_undo_remove', { index: idx }).catch(warn) });
  };
  const identifyCard = identify;
  window.identify = (idx) => { identifyCard(idx); invoke('seat_identify', { index: idx }).catch(() => {}); };
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
  function setGames(list, focusId) {
    known.clear();
    list.forEach((g) => known.set(g.id, g));
    const mapped = list.map((g) => ({ ...g, image: g.image || undefined, tags: g.update ? [...g.tags, `Nova versão ${g.update}`] : g.tags }));
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
      return { id: g.id, name: g.name, tagline: g.tagline || '', players: g.players || '', image: g.image || null, store: pc?.store ?? g.store ?? null, launch: pc?.launch ?? null };
    });
    invoke('library_sync', { games: payload }).then((list) => list.forEach((g) => known.set(g.id, g))).catch(warn);
  };
  window.forgetLibrary = () => {};
  invoke('library_list').then((list) => setGames(list)).catch(warn);
  window.__arcadeReady.then(() => invoke('library_list').then((list) => setGames(list))).catch(() => {});

  // Jogos deste PC: a varredura real (Steam e Epic) antes de abrir a lista
  const scanList = openScanGames;
  window.openScanGames = async () => {
    try {
      const found = await invoke('pc_scan');
      PC_GAMES.splice(0, PC_GAMES.length, ...found.map((g) => ({ ...g, art: palette(g.id) })));
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
      invoke('hub_apply_update', { installerUrl: hubUpdate.installerUrl }).catch(warn);
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
