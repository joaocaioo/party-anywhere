// Idiomas da UI: português do Brasil (o texto-fonte, escrito direto no index.html e no bridge.js) e inglês.
// O inglês vem do dicionário em i18n-en.js: frases exatas e moldes com partes variáveis ("3 jogadores").
// Em inglês, um MutationObserver troca cada texto (e placeholder, title, aria-label, alt) assim que ele
// entra na tela e guarda o original: voltar ao português não recarrega nada. Em português o observador fica
// desligado e nada muda. O que pinta em canvas chama `tr()` direto e redesenha no evento `pa:lang`.
(() => {
  const KEY = 'pa.lang';
  const ATTRS = ['placeholder', 'title', 'aria-label', 'alt'];
  const SKIP = new Set(['SCRIPT', 'STYLE', 'TEXTAREA', 'NOSCRIPT']);
  const dict = (window.PA_I18N_EN = window.PA_I18N_EN || { exact: {}, patterns: [] });
  const norm = (s) => s.replace(/\s+/g, ' ').trim();
  const texts = new WeakMap(); // nó de texto → { pt, en }
  const attrs = new WeakMap(); // elemento → { [atributo]: { pt, en } }
  const cache = new Map();
  const missing = new Set(); // para conferir a tradução no navegador: PA_I18N.missing()
  let lang = 'pt-BR';
  let observer = null;

  function translate(key) {
    const hit = cache.get(key);
    if (hit !== undefined) return hit;
    let out = Object.prototype.hasOwnProperty.call(dict.exact, key) ? dict.exact[key] : null;
    if (out == null) {
      for (const [re, rep] of dict.patterns) {
        if (re.test(key)) { out = key.replace(re, rep); break; }
      }
    }
    if (out == null) missing.add(key);
    if (cache.size > 4000) cache.clear();
    cache.set(key, out);
    return out;
  }

  // traduz uma frase solta; mantém os espaços das pontas (o texto entre duas tags costuma ter)
  function tr(s) {
    if (lang !== 'en' || typeof s !== 'string') return s;
    const key = norm(s);
    if (!key || !/\p{L}/u.test(key)) return s;
    const out = translate(key);
    if (out == null) return s;
    return s.match(/^\s*/)[0] + out + s.match(/\s*$/)[0];
  }

  const skipped = (el) => !el || SKIP.has(el.nodeName) || !!el.closest('[contenteditable="true"],[data-no-i18n]');

  function fixText(node) {
    if (skipped(node.parentElement)) return;
    const rec = texts.get(node);
    if (rec && node.data === rec.en) return; // a troca foi nossa
    const pt = node.data;
    const en = tr(pt);
    texts.set(node, { pt, en });
    if (en !== pt) node.data = en;
  }

  function fixAttr(el, name) {
    const val = el.getAttribute(name);
    if (val == null || skipped(el)) return;
    let recs = attrs.get(el);
    const rec = recs && recs[name];
    if (rec && val === rec.en) return;
    const en = tr(val);
    if (!recs) attrs.set(el, (recs = {}));
    recs[name] = { pt: val, en };
    if (en !== val) el.setAttribute(name, en);
  }

  function walk(root) {
    if (root.nodeType === 3) { fixText(root); return; }
    if (root.nodeType !== 1 && root.nodeType !== 9 && root.nodeType !== 11) return;
    if (root.nodeType === 1) {
      if (SKIP.has(root.nodeName)) return;
      for (const a of ATTRS) if (root.hasAttribute(a)) fixAttr(root, a);
    }
    const it = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
    for (let n = it.nextNode(); n; n = it.nextNode()) {
      if (n.nodeType === 3) fixText(n);
      else for (const a of ATTRS) if (n.hasAttribute(a)) fixAttr(n, a);
    }
  }

  // volta ao português: só o que nós trocamos (o resto a UI já desenhou em português)
  function restore(root) {
    const it = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
    for (let n = it.nextNode(); n; n = it.nextNode()) {
      if (n.nodeType === 3) {
        const rec = texts.get(n);
        if (rec && n.data === rec.en && rec.en !== rec.pt) n.data = rec.pt;
        texts.delete(n);
      } else {
        const recs = attrs.get(n);
        if (!recs) continue;
        for (const [name, rec] of Object.entries(recs)) {
          if (n.getAttribute(name) === rec.en && rec.en !== rec.pt) n.setAttribute(name, rec.pt);
        }
        attrs.delete(n);
      }
    }
    if (document.title !== titlePt) document.title = titlePt;
  }

  function onMutations(list) {
    for (const m of list) {
      if (m.type === 'characterData') fixText(m.target);
      else if (m.type === 'attributes') fixAttr(m.target, m.attributeName);
      else for (const n of m.addedNodes) walk(n);
    }
  }

  let titlePt = document.title;
  function apply() {
    document.documentElement.lang = lang === 'en' ? 'en' : 'pt-BR';
    if (lang === 'en') {
      if (!observer) {
        observer = new MutationObserver(onMutations);
        observer.observe(document.documentElement, {
          subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS,
        });
      }
      titlePt = document.title;
      if (document.body) walk(document.body);
      document.title = tr(titlePt);
    } else {
      if (observer) { observer.disconnect(); observer = null; }
      if (document.body) restore(document.body);
    }
  }

  function setLang(next, opts = {}) {
    next = next === 'en' ? 'en' : 'pt-BR';
    try { localStorage.setItem(KEY, next); } catch (e) { /* sem armazenamento: vale nesta sessão */ }
    if (next === lang) return;
    lang = next;
    apply();
    window.dispatchEvent(new CustomEvent('pa:lang', { detail: { lang, fromApp: !!opts.fromApp } }));
  }

  // primeira escolha: a salva; senão, o idioma do sistema (português → pt-BR, qualquer outro → inglês)
  let saved = null;
  try { saved = localStorage.getItem(KEY); } catch (e) { /* idem */ }
  lang = saved || (/^pt\b/i.test(navigator.language || 'pt') ? 'pt-BR' : 'en');
  apply(); // antes do <body>: o observador já pega cada nó que o navegador monta
  document.addEventListener('DOMContentLoaded', () => { titlePt = document.title; apply(); });

  window.tr = tr;
  window.PA_I18N = {
    get lang() { return lang; },
    set: setLang,
    missing: () => [...missing].sort(),
  };
})();
