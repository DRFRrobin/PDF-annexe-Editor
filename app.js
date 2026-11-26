'use strict';
/* Annexio (version web) — chaîne d'étapes : Fichier → ReeMaker → Annexio → Export.
 * Les tampons s'empilent sur le même document ; rien n'est écrit avant l'export. Tout reste dans le navigateur. */
const APP_VERSION = '1.1.19'; // +0.0.1 à chaque mise à jour (voir CHANGELOG.md)
const { PDFDocument, StandardFonts } = PDFLib;
const St = Stamps;
pdfjsLib.GlobalWorkerOptions.workerSrc = 'vendor/pdf.worker.min.js';
const $ = id => document.getElementById(id);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const KEY = 'annexio.web.v2';
const BASE_K = 96 / 72; // pixels écran par point à 100 %

// ───────────────────────── état ─────────────────────────
const DEF_RM = () => ({ apply: true, site: '', ree: '', indice: '', start: 1, arret: '', cycle: '', showCycle: false,
  mode: 'all', from: 1, to: 1, tranches: [0], zoom: 1.2, orient: 'auto' });
const DEF_AN = () => ({ apply: true, annexe: '1', template: 'Annexe {annexe} Folio {n}/{total}', start: 1, total: '', size: 14,
  pages: '', fx: 0.04, fy: 0.93, margin: 20 });
const DEF_SET = () => ({ theme: 'system', baseSize: 9, fill: true, keepPos: true });

const S = {
  name: '', bytes: null, doc: null, n: 0, cur: 0, vis: [],
  step: 0, visited: [false, false, false, false], seen: [false, false, false, false], exported: false, outName: '',
  rm: DEF_RM(), an: DEF_AN(), set: DEF_SET(), arrets: ['TEM'],
  place: {}, skipped: {}, rmDone: false, last: null, run: null, orientCache: {},
  zoom: { mode: 'fit', k: BASE_K }, fonts: null, wheel: 0, thumbToken: 0,
};

// ───────────────────────── mémorisation locale ─────────────────────────
function persist() {
  clearTimeout(persist.t);
  persist.t = setTimeout(() => {
    try { localStorage.setItem(KEY, JSON.stringify({ rm: S.rm, an: S.an, set: S.set, arrets: S.arrets })); } catch (_) { /* navigation privée */ }
  }, 300);
}
function restore() {
  try {
    const o = JSON.parse(localStorage.getItem(KEY) || 'null');
    if (!o) return;
    Object.assign(S.rm, o.rm || {}); Object.assign(S.an, o.an || {}); Object.assign(S.set, o.set || {});
    if (Array.isArray(o.arrets) && o.arrets.length) S.arrets = o.arrets;
    S.rm.mode = S.rm.mode === 'range' ? 'range' : 'all';
  } catch (_) { /* valeurs par défaut */ }
}

// ───────────────────────── outils ─────────────────────────
const num = (v, d) => { const x = parseFloat(String(v).replace(',', '.')); return Number.isFinite(x) ? x : d; };
function say(text, err) { const el = $('status'); el.textContent = text || ''; el.classList.toggle('err', !!err); }
function mk(tag, css, text) { const el = document.createElement(tag); if (css) el.style.cssText = css; if (text != null) el.textContent = text; return el; }
const baseName = () => S.name.replace(/\.pdf$/i, '');

function rmPages() {
  if (!S.n) return [];
  if (S.rm.mode === 'all') return Array.from({ length: S.n }, (_, i) => i);
  let a = clampInt(S.rm.from, 1, S.n), b = clampInt(S.rm.to, 1, S.n);
  if (a > b) [a, b] = [b, a];
  const out = []; for (let i = a - 1; i < b; i++) out.push(i);
  return out;
}
const clampInt = (v, a, b) => Math.min(Math.max(parseInt(v) || a, a), b);
function folioOf(i) {
  let c = 0;
  for (const j of rmPages()) { if (j === i) break; if (!S.skipped[j]) c++; }
  return (parseInt(S.rm.start) || 1) + c;
}
const rmView = () => ({ ...S.rm, baseSize: S.set.baseSize });
const anActive = () => S.an.apply && S.seen[2];
function anSet() { try { return St.parseRange(S.an.pages, S.n); } catch (_) { return new Set(); } }

// ───────────────────────── chargement du PDF ─────────────────────────
async function openFile(file) {
  if (!file) return;
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const doc = await pdfjsLib.getDocument({ data: bytes.slice() }).promise;
    resetDocument();
    S.bytes = bytes; S.doc = doc; S.n = doc.numPages; S.name = file.name; S.cur = 0;
    S.rm.from = 1; S.rm.to = S.n;
    S.outName = baseName() + '_folioté.pdf';
    syncForm();
    say('« ' + S.name + ' » chargé (' + S.n + ' page(s)).');
    $('wrap').hidden = false; $('empty').hidden = true;
    await renderPage();
    buildThumbs();
    updateUI();
  } catch (e) {
    say('Ce fichier n\'est pas un PDF lisible' + (/password/i.test(e.message) ? ' (protégé par un mot de passe)' : '') + '.', true);
  }
}
function resetDocument() {
  S.bytes = null; S.doc = null; S.n = 0; S.vis = []; S.place = {}; S.skipped = {}; S.rmDone = false; S.last = null; S.run = null;
  S.orientCache = {}; S.exported = false; S.visited = [false, false, false, false]; S.seen = [false, false, false, false]; S.thumbToken++;
}
function closeFile() {
  resetDocument(); S.name = ''; S.step = 0;
  $('wrap').hidden = true; $('empty').hidden = false; $('thumbs').innerHTML = '';
  $('file').value = '';
  say(''); updateUI(); showStepPanel();
}

// ───────────────────────── aperçu ─────────────────────────
let renderTask = null, renderToken = 0;
function currentK(w, h) {
  const sc = $('scroller'), aw = Math.max(200, sc.clientWidth - 30), ah = Math.max(200, sc.clientHeight - 30);
  if (S.zoom.mode === 'fit') return Math.min(aw / w, ah / h);
  if (S.zoom.mode === 'width') return Math.max(0.1, (aw - 8) / w);
  return S.zoom.k;
}
async function renderPage() {
  if (!S.doc) return;
  const my = ++renderToken;
  const page = await S.doc.getPage(S.cur + 1);
  const vp1 = page.getViewport({ scale: 1 });
  S.vis[S.cur] = { w: vp1.width, h: vp1.height };
  const k = S.zoom.k = currentK(vp1.width, vp1.height), dpr = window.devicePixelRatio || 1;
  const cv = $('cv'), vp = page.getViewport({ scale: k * dpr });
  cv.width = Math.round(vp.width); cv.height = Math.round(vp.height);
  const cw = vp1.width * k + 'px', ch = vp1.height * k + 'px';
  cv.style.width = cw; cv.style.height = ch; $('wrap').style.width = cw; $('wrap').style.height = ch;
  $('zVal').textContent = Math.round(k / BASE_K * 100) + ' %';
  renderOverlays();
  if (renderTask) { try { renderTask.cancel(); } catch (_) { /* déjà fini */ } }
  renderTask = page.render({ canvasContext: cv.getContext('2d'), viewport: vp });
  try { await renderTask.promise; } catch (e) { if (e && e.name !== 'RenderingCancelledException') throw e; }
  if (my === renderToken) refreshThumbs();
}

function setZoom(mode, k) {
  S.zoom.mode = mode; if (k) S.zoom.k = Math.min(Math.max(k, 0.15), 8);
  clearTimeout(setZoom.t); setZoom.t = setTimeout(renderPage, 60);
}

// ───────────────────────── calques : cartouche et tampon Annexe ─────────────────────────
function cartPlacement(i) {
  if (!S.rm.apply) return null;
  if (S.run && S.run.list[S.run.pos] === i) return { pl: S.run.pending, mode: 'live' };
  if (S.place[i]) return { pl: S.place[i], mode: 'fixed' };
  if (!S.run && !S.rmDone && S.step === 1 && rmPages().includes(i)) return { pl: { fx: 0.6, fy: 0.03, orient: orientFixed() }, mode: 'ghost' };
  return null;
}
const orientFixed = () => (S.rm.orient === 'auto' ? 0 : parseInt(S.rm.orient) || 0);

function drawCartouche(el, pl, folio, k, v, mode) {
  const ly = St.cartLayout(S.fonts.reg, rmView(), folio), o = pl.orient || 0, bb = St.bbox(ly, o);
  const { vx, vy } = St.place(pl, { VW: v.w, VH: v.h }, bb);
  el.className = 'ov' + (mode === 'live' ? ' live' : mode === 'ghost' ? ' ghost' : '');
  el.style.left = vx * k + 'px'; el.style.top = vy * k + 'px'; el.style.width = bb.w * k + 'px'; el.style.height = bb.h * k + 'px';
  el.replaceChildren();
  const tr = { 0: '', 90: `translate(${ly.H * k}px,0) rotate(90deg)`, 180: `translate(${ly.W * k}px,${ly.H * k}px) rotate(180deg)`, 270: `translate(0,${ly.W * k}px) rotate(270deg)` }[o];
  const ci = mk('div', `width:${ly.W * k}px;height:${ly.H * k}px;transform:${tr};background:${S.set.fill ? '#fff' : 'transparent'}`); ci.className = 'ci';
  const line = (x, y, w, h) => ci.append(mk('div', `left:${x * k}px;top:${y * k}px;width:${w * k}px;height:${h * k}px`));
  const b = ly.bw;
  line(-b / 2, -b / 2, ly.W + b, b); line(-b / 2, ly.H - b / 2, ly.W + b, b);
  line(-b / 2, -b / 2, b, ly.H + b); line(ly.W - b / 2, -b / 2, b, ly.H + b);
  line(0, ly.rowH - b / 2, ly.W, b); line(0, 2 * ly.rowH - b / 2, ly.W, b); line(ly.leftW - b / 2, ly.rowH, b, 2 * ly.rowH);
  const t = ly.t, text = (str, x, row) => ci.append(mk('span', `left:${x * k}px;top:${row * ly.rowH * k}px;height:${ly.rowH * k}px;line-height:${ly.rowH * k}px;font-size:${ly.fs * k}px`, str));
  const cx = str => ly.leftW + (ly.rightW - St.width(S.fonts.reg, str, ly.fs)) / 2;
  text(t.l1, ly.padx, 0); text(t.l2a, ly.padx, 1); text(t.l2b, cx(t.l2b), 1); text(t.l3a, ly.padx, 2); text(t.l3b, cx(t.l3b), 2);
  el.append(ci);
}

function annexData(i) {
  if (!anActive() || !anSet().has(i + 1)) return null;
  const text = St.annexText(S.an, i + 1, S.n), size = clampNum(S.an.size, 6, 72, 14);
  return { text, size, ly: St.annexLayout(S.fonts.bold, text, size) };
}
const clampNum = (v, a, b, d) => Math.min(Math.max(num(v, d), a), b);

function renderOverlays() {
  const v = S.vis[S.cur];
  if (!S.doc || !v || !S.fonts) return;
  const k = S.zoom.k, cp = cartPlacement(S.cur), ovC = $('ovC'), ovA = $('ovA');
  if (cp) { ovC.hidden = false; drawCartouche(ovC, cp.pl, folioOf(S.cur), k, v, cp.mode); } else { ovC.hidden = true; }
  const ad = annexData(S.cur);
  if (ad) {
    const { vx, vy } = St.place(S.an, { VW: v.w, VH: v.h }, { w: ad.ly.w, h: ad.ly.h });
    Object.assign(ovA.style, { left: vx * k + 'px', top: vy * k + 'px', width: ad.ly.w * k + 'px', height: ad.ly.h * k + 'px',
      fontSize: ad.size * k + 'px', borderWidth: St.A_BORDER * k + 'px', background: S.set.fill ? '#fff' : 'transparent' });
    ovA.textContent = ad.text; ovA.hidden = false;
    ovA.className = 'ov ann' + (S.step === 2 ? ' live' : '');
  } else { ovA.hidden = true; }
}

// couche déplaçable à l'écran (selon l'étape)
function liveLayer() {
  const v = S.vis[S.cur];
  if (!S.doc || !v) return null;
  if (S.step === 1 && S.run && S.run.list[S.run.pos] === S.cur) {
    const pl = S.run.pending, ly = St.cartLayout(S.fonts.reg, rmView(), folioOf(S.cur)), bb = St.bbox(ly, pl.orient || 0);
    return { bb, v, pl, pos: () => St.place(pl, { VW: v.w, VH: v.h }, bb) };
  }
  const ad = S.step === 2 ? annexData(S.cur) : null;
  if (ad) { const bb = { w: ad.ly.w, h: ad.ly.h }; return { bb, v, pl: S.an, pos: () => St.place(S.an, { VW: v.w, VH: v.h }, bb) }; }
  return null;
}
function moveLive(L, x, y) {
  L.pl.fx = St.clamp(x, 0, Math.max(0, L.v.w - L.bb.w)) / L.v.w;
  L.pl.fy = St.clamp(y, 0, Math.max(0, L.v.h - L.bb.h)) / L.v.h;
}
function onPointerDown(e) {
  if (e.button !== 0) return;
  const L = liveLayer(); if (!L) return;
  e.preventDefault();
  const r = $('wrap').getBoundingClientRect(), k = S.zoom.k, p = L.pos();
  const onBox = e.target.closest('.ov.live');
  const px = (e.clientX - r.left) / k, py = (e.clientY - r.top) / k;
  const offx = onBox ? px - p.vx : L.bb.w / 2, offy = onBox ? py - p.vy : L.bb.h / 2;
  const move = ev => { moveLive(L, (ev.clientX - r.left) / k - offx, (ev.clientY - r.top) / k - offy); renderOverlays(); };
  const up = () => { removeEventListener('pointermove', move); removeEventListener('pointerup', up); persist(); updateUI(); };
  move(e); addEventListener('pointermove', move); addEventListener('pointerup', up);
}

// ───────────────────────── miniatures ─────────────────────────
async function buildThumbs() {
  const box = $('thumbs'), my = ++S.thumbToken; box.replaceChildren();
  const count = Math.min(S.n, 300), dpr = window.devicePixelRatio || 1, btns = [];
  for (let i = 0; i < count; i++) {
    const b = mk('button'); b.type = 'button'; b.title = 'Page ' + (i + 1); b.setAttribute('aria-label', 'Page ' + (i + 1));
    const c = document.createElement('canvas'); c.width = 70; c.height = 99; c.style.height = '99px';
    b.append(c, mk('span', '', String(i + 1)), mk('i')); b.onclick = () => gotoPage(i); box.append(b); btns.push(b);
  }
  refreshThumbs();
  for (let i = 0; i < count; i++) {
    if (my !== S.thumbToken || !S.doc) return;
    try {
      const page = await S.doc.getPage(i + 1), v1 = page.getViewport({ scale: 1 }), sc = 70 * dpr / v1.width, vp = page.getViewport({ scale: sc });
      const c = btns[i].firstChild; c.width = Math.round(vp.width); c.height = Math.round(vp.height); c.style.height = (vp.height / dpr) + 'px';
      await page.render({ canvasContext: c.getContext('2d'), viewport: vp }).promise;
    } catch (_) { /* miniature facultative */ }
  }
}
function refreshThumbs() {
  const set = anSet();
  $$('#thumbs button').forEach((b, i) => {
    b.classList.toggle('on', i === S.cur);
    b.classList.toggle('stamped', !!(S.rm.apply && S.place[i]) || !!(anActive() && set.has(i + 1)));
  });
  const on = $('thumbs').querySelector('button.on'); if (on) on.scrollIntoView({ block: 'nearest' });
}

// ───────────────────────── navigation entre pages ─────────────────────────
function gotoPage(i) {
  if (!S.doc) return;
  i = St.clamp(i, 0, S.n - 1);
  if (S.run) {
    const pos = S.run.list.indexOf(i);
    if (pos < 0) return say('Cette page n\'est pas dans la plage à folioter.', true);
    return setRunPos(pos);
  }
  if (i === S.cur) return;
  S.cur = i; renderPage(); updateUI();
}

// ───────────────────────── étape ReeMaker : foliotage page par page ─────────────────────────
async function resolveOrient(i) {
  if (S.rm.orient !== 'auto') return parseInt(S.rm.orient) || 0;
  if (S.orientCache[i] == null) S.orientCache[i] = await St.detectOrientation(await S.doc.getPage(i + 1));
  return S.orientCache[i];
}
async function pendingFor(i) {
  const orient = await resolveOrient(i);
  if (S.place[i]) return { ...S.place[i] };
  if (S.last && S.set.keepPos) return { fx: S.last.fx, fy: S.last.fy, orient };
  return { fx: 0.6, fy: 0.03, orient };
}
async function startRun() {
  const list = rmPages();
  if (!S.doc) return say('Ouvrez d\'abord un PDF.', true);
  if (!list.length) return say('Aucune page à folioter.', true);
  S.run = { list, pos: 0, pending: null, snap: JSON.stringify({ place: S.place, skipped: S.skipped, rmDone: S.rmDone, last: S.last }) };
  S.rmDone = false;
  await setRunPos(0);
}
async function setRunPos(pos) {
  const r = S.run; if (!r) return;
  r.pos = pos; const i = r.list[pos];
  r.pending = await pendingFor(i);
  S.cur = i; await renderPage(); updateUI();
}
async function validateRun() {
  const r = S.run; if (!r) return;
  const i = r.list[r.pos];
  S.place[i] = { ...r.pending }; delete S.skipped[i]; S.last = { ...r.pending };
  await advanceRun();
}
async function skipRun() {
  const r = S.run; if (!r) return;
  const i = r.list[r.pos];
  S.skipped[i] = true; delete S.place[i];
  await advanceRun();
}
async function advanceRun() {
  const r = S.run;
  if (r.pos + 1 >= r.list.length) {
    S.run = null; S.rmDone = true;
    const n = Object.keys(S.place).length;
    say('Foliotage terminé : ' + n + ' page(s) · folios ' + folioRange() + '.');
    await renderPage(); updateUI(); return;
  }
  await setRunPos(r.pos + 1);
}
async function prevRun() { if (S.run && S.run.pos > 0) await setRunPos(S.run.pos - 1); }
async function cancelRun() {
  if (!S.run) return;
  const o = JSON.parse(S.run.snap); S.place = o.place; S.skipped = o.skipped; S.rmDone = o.rmDone; S.last = o.last; S.run = null;
  say('Foliotage annulé.'); await renderPage(); updateUI();
}
function folioRange() {
  const idx = Object.keys(S.place).map(Number).sort((a, b) => a - b);
  return idx.length ? folioOf(idx[0]) + '–' + folioOf(idx[idx.length - 1]) : '—';
}
function nudge(dx, dy) {
  const L = liveLayer(); if (!L) return false;
  const p = L.pos(); moveLive(L, p.vx + dx, p.vy + dy); renderOverlays(); persist(); return true;
}

// ───────────────────────── étapes et interface ─────────────────────────
const STEP_NAMES = ['Fichier', 'ReeMaker', 'Annexio', 'Export'];
async function setStep(i) {
  if (i > 0 && !S.doc) return say('Ouvrez d\'abord un PDF.', true);
  if (S.run && i !== 1 && !confirm('Le foliotage en cours sera annulé. Continuer ?')) return;
  if (S.run && i !== 1) await cancelRun();
  for (let j = 0; j < i; j++) S.visited[j] = true;
  S.step = i; S.visited[i] = true; S.seen[i] = true;
  if (i === 3 && !S.outName && S.name) S.outName = baseName() + '_folioté.pdf';
  showStepPanel(); renderOverlays(); updateUI();
}
function showStepPanel() { $$('.step').forEach(p => { p.hidden = +p.dataset.step !== S.step; }); }

function chevState(i) {
  if (i === 0) return S.doc ? ['done', S.name + ' · ' + S.n + ' p.'] : ['', ''];
  if (i === 1) {
    if (!S.rm.apply) return S.visited[1] ? ['skip', 'ignoré'] : ['', ''];
    if (S.run) return ['', 'page ' + (S.run.pos + 1) + '/' + S.run.list.length];
    return S.rmDone ? ['done', 'folios ' + folioRange()] : ['', ''];
  }
  if (i === 2) {
    if (!S.an.apply) return S.visited[2] && S.step !== 2 ? ['skip', 'ignoré'] : ['', ''];
    return S.visited[2] && S.step !== 2 ? ['done', 'Annexe ' + S.an.annexe] : ['', ''];
  }
  return S.exported ? ['done', 'exporté'] : ['', ''];
}
function updateUI() {
  $$('.chev').forEach((b, i) => {
    const [cls, sm] = chevState(i);
    b.className = 'chev' + (i === S.step ? ' on' : cls ? ' ' + cls : '');
    b.querySelector('small').textContent = sm;
    b.disabled = i > 0 && !S.doc;
    if (i === S.step) b.setAttribute('aria-current', 'step'); else b.removeAttribute('aria-current');
    const dot = b.querySelector('i'); dot.textContent = (cls === 'done' && i !== S.step) ? '✓' : String(i + 1);
  });
  // bande fichier
  $('fileInfo').textContent = S.doc ? '📄 ' + S.name + ' · ' + S.n + ' page(s)' : 'Aucun PDF — glissez-le dans la fenêtre ou cliquez sur « Choisir un PDF ».';
  $('pageInd').textContent = S.doc ? 'Page ' + (S.cur + 1) + '/' + S.n : '';
  $('btnPrev').disabled = !S.doc || S.cur === 0 || !!S.run; $('btnNext').disabled = !S.doc || S.cur >= S.n - 1 || !!S.run; $('btnClose').hidden = !S.doc;
  $('dropText').innerHTML = S.doc ? '📄 ' + escapeHtml(S.name) + '<br><small>' + S.n + ' page(s) · cliquer pour changer</small>' : 'Choisir un PDF<br><small>ou le glisser ici · Ctrl+O</small>';
  // étape ReeMaker : formulaire / suivi
  $('rmForm').hidden = !!S.run; $('rmRun').hidden = !S.run;
  $('rmFields').disabled = !S.rm.apply;
  $('rmStartBtn').hidden = !S.rm.apply; $('rmStartBtn').textContent = S.rmDone ? '↺ Modifier le foliotage' : '▶ Démarrer le foliotage';
  $('rmStartBtn').disabled = !S.doc;
  $('rmDoneInfo').textContent = !S.rm.apply ? 'Étape ignorée : aucun cartouche ne sera ajouté.'
    : S.rmDone ? '✔ ' + Object.keys(S.place).length + ' page(s) posées · folios ' + folioRange() + '.' : (S.doc ? 'Les pages concernées : ' + rmPagesLabel() + '.' : '');
  if (S.run) renderRunPanel();
  $('anFields').disabled = !S.an.apply;
  if (S.step === 3) renderRecap();
  // bouton principal
  const p = $('primary'); p.disabled = false; p.hidden = false;
  if (S.step === 0) { p.textContent = 'Suivant : ReeMaker ▸'; p.disabled = !S.doc; }
  else if (S.step === 1) {
    if (S.run) p.textContent = '✔ Valider et page suivante';
    else if (!S.rm.apply || S.rmDone) p.textContent = 'Suivant : Annexio ▸';
    else p.textContent = '▶ Démarrer le foliotage';
  } else if (S.step === 2) p.textContent = 'Suivant : Export ▸';
  else { p.textContent = '📤 Exporter le PDF…'; p.disabled = !canExport(); }
  refreshThumbs();
}
const escapeHtml = s => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
function rmPagesLabel() { const l = rmPages(); return l.length ? (l.length === S.n ? 'toutes (' + S.n + ')' : (l[0] + 1) + ' à ' + (l[l.length - 1] + 1)) : 'aucune'; }

function renderRunPanel() {
  const r = S.run, i = r.list[r.pos];
  $('runInfo').textContent = 'Page ' + (i + 1) + ' · folio ' + folioOf(i) + ' · ' + (r.pos + 1) + '/' + r.list.length;
  $('runPrev').disabled = r.pos === 0;
  $('runKeep').checked = S.set.keepPos;
  const box = $('runList'); box.replaceChildren();
  r.list.forEach((p, idx) => {
    const done = S.place[p], sk = S.skipped[p], cur = idx === r.pos;
    box.append(mk('div', '', (cur ? '● ' : done ? '✔ ' : sk ? '⇥ ' : '○ ') + 'page ' + (p + 1) + (sk ? ' — ignorée' : ' → folio ' + folioOf(p))));
    if (cur) box.lastChild.className = 'cur';
  });
  const cur = box.querySelector('.cur'); if (cur) cur.scrollIntoView({ block: 'nearest' });
}

function canExport() { return !!S.doc && ((S.rm.apply && Object.keys(S.place).length > 0) || (anActive() && anSet().size > 0)); }
function renderRecap() {
  const ul = $('recap'); ul.replaceChildren();
  const li = (ok, text) => { const e = mk('li', '', (ok === true ? '✓ ' : ok === 'warn' ? '⚠ ' : '⇥ ') + text); if (ok === 'warn') e.className = 'w'; ul.append(e); };
  li(true, 'Fichier : ' + S.name + ' (' + S.n + ' pages)');
  const placed = Object.keys(S.place).length;
  if (!S.rm.apply) li(false, 'ReeMaker : ignoré');
  else if (S.rmDone) li(true, 'ReeMaker : cartouche sur ' + placed + ' page(s), folios ' + folioRange());
  else li('warn', 'ReeMaker : foliotage non terminé (' + placed + ' page(s) posée(s))');
  if (!S.an.apply) li(false, 'Annexio : ignoré');
  else if (!S.seen[2]) li(false, 'Annexio : étape non ouverte (aucun tampon ajouté)');
  else { let n = 0; try { n = anSet().size; } catch (_) { /* plage invalide */ } li(n > 0 ? true : 'warn', 'Annexio : tampon « ' + (St.annexText(S.an, 1, S.n)) + ' » sur ' + n + ' page(s)'); }
  if (document.activeElement !== $('outName')) $('outName').value = S.outName;
}

// ───────────────────────── export ─────────────────────────
async function exportPdf() {
  if (!canExport()) return say('Rien à exporter : appliquez au moins un tampon.', true);
  const btn = $('exportBtn'); btn.disabled = true; $('primary').disabled = true; $('exportMsg').textContent = ''; say('Export en cours…');
  try {
    const doc = await PDFDocument.load(S.bytes, { ignoreEncryption: true });
    const reg = await doc.embedFont(StandardFonts.Helvetica), bold = await doc.embedFont(StandardFonts.HelveticaBold);
    const set = anSet(), pages = doc.getPages();
    let nc = 0, na = 0;
    pages.forEach((page, i) => {
      if (S.rm.apply && S.place[i]) { St.drawCartouche(page, reg, St.cartLayout(reg, rmView(), folioOf(i)), S.place[i], { fill: S.set.fill }); nc++; }
      if (anActive() && set.has(i + 1)) { St.drawAnnexio(page, bold, St.annexText(S.an, i + 1, S.n), { ...S.an, size: clampNum(S.an.size, 6, 72, 14), fill: S.set.fill }); na++; }
    });
    const out = await doc.save();
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([out], { type: 'application/pdf' }));
    a.download = (S.outName || baseName() + '_folioté.pdf').replace(/[\\/:*?"<>|]/g, '_');
    if (!/\.pdf$/i.test(a.download)) a.download += '.pdf';
    a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 8000);
    S.exported = true;
    const msg = '✔ Exporté : ' + a.download + ' (' + nc + ' cartouche(s), ' + na + ' tampon(s) Annexio).';
    $('exportMsg').textContent = msg; say(msg);
  } catch (e) { say('Export impossible : ' + e.message, true); }
  btn.disabled = false; updateUI();
}

// ───────────────────────── formulaires ─────────────────────────
const FIELDS = [
  ['rmSite', 'rm', 'site', 's'], ['rmRee', 'rm', 'ree', 's'], ['rmIndice', 'rm', 'indice', 's'], ['rmStart', 'rm', 'start', 'i'],
  ['rmArret', 'rm', 'arret', 's'], ['rmCycle', 'rm', 'cycle', 's'], ['rmShowCycle', 'rm', 'showCycle', 'b'],
  ['rmFrom', 'rm', 'from', 'i'], ['rmTo', 'rm', 'to', 'i'], ['rmZoom', 'rm', 'zoom', 'f'], ['rmOrient', 'rm', 'orient', 's'],
  ['anAnnexe', 'an', 'annexe', 's'], ['anStart', 'an', 'start', 'i'], ['anTpl', 'an', 'template', 's'], ['anTotal', 'an', 'total', 's'],
  ['anSize', 'an', 'size', 'f'], ['anPages', 'an', 'pages', 's'], ['anMargin', 'an', 'margin', 'f'],
];
function syncForm() {
  for (const [id, grp, key, t] of FIELDS) { const el = $(id), v = S[grp][key]; if (t === 'b') el.checked = !!v; else el.value = v; }
  $$('input[name=rmApply]').forEach(r => { r.checked = (r.value === '1') === S.rm.apply; });
  $$('input[name=anApply]').forEach(r => { r.checked = (r.value === '1') === S.an.apply; });
  $$('input[name=rmMode]').forEach(r => { r.checked = r.value === S.rm.mode; });
  $$('#rmTranches input').forEach(c => { c.checked = S.rm.tranches.includes(+c.value); });
  $('arrets').replaceChildren(...S.arrets.map(v => { const o = document.createElement('option'); o.value = v; return o; }));
  $('setTheme').value = S.set.theme; $('setBase').value = S.set.baseSize; $('setFill').checked = S.set.fill; $('setKeep').checked = S.set.keepPos;
}
function onFieldChange(id, grp, key, t) {
  const el = $(id);
  let v = t === 'b' ? el.checked : t === 'i' ? (parseInt(el.value) || '') : t === 'f' ? num(el.value, '') : el.value;
  if (key === 'zoom') v = St.clamp(num(el.value, 1.2), 0.5, 3);
  S[grp][key] = v;
  if (grp === 'rm' && ['mode', 'from', 'to', 'start'].includes(key)) S.rmDone = false;
  if (id === 'rmArret') rememberArret(el.value);
  if (key === 'orient' && S.run) pendingFor(S.run.list[S.run.pos]).then(p => { S.run.pending.orient = p.orient; renderOverlays(); });
  persist(); renderOverlays(); updateUI();
}
function rememberArret(v) {
  v = v.trim(); if (!v || S.arrets.includes(v)) return;
  S.arrets.push(v); S.arrets = S.arrets.slice(-12); $('arrets').append(Object.assign(document.createElement('option'), { value: v })); persist();
}
function buildTranches() {
  const box = $('rmTranches');
  for (let t = 0; t <= 9; t++) {
    const l = mk('label'), c = document.createElement('input'); c.type = 'checkbox'; c.value = t;
    c.onchange = () => { S.rm.tranches = $$('#rmTranches input').filter(x => x.checked).map(x => +x.value); persist(); renderOverlays(); updateUI(); };
    l.append(c, ' ' + t); box.append(l);
  }
}
function setAnPages(arr) { S.an.pages = arr; $('anPages').value = arr; persist(); renderOverlays(); updateUI(); }
function placeAn(h, v) {
  const vis = S.vis[S.cur], ad = annexData(S.cur) || { ly: St.annexLayout(S.fonts.bold, St.annexText(S.an, S.cur + 1, S.n), clampNum(S.an.size, 6, 72, 14)) };
  if (!vis) return;
  const m = num(S.an.margin, 20), w = ad.ly.w, hh = ad.ly.h;
  const pos = (f, page, b) => f <= 0 ? m : f >= 1 ? page - b - m : (page - b) / 2;
  S.an.fx = St.clamp(pos(h, vis.w, w) / vis.w, 0, 1); S.an.fy = St.clamp(pos(v, vis.h, hh) / vis.h, 0, 1);
  persist(); renderOverlays();
}

// ───────────────────────── thème et réglages ─────────────────────────
function applyTheme() { document.documentElement.dataset.theme = S.set.theme; }

// ───────────────────────── branchements ─────────────────────────
function wire() {
  for (const f of FIELDS) { const el = $(f[0]); el.addEventListener(f[3] === 'b' ? 'change' : 'input', () => onFieldChange(...f)); }
  $$('input[name=rmApply]').forEach(r => r.addEventListener('change', () => { S.rm.apply = r.value === '1'; if (!S.rm.apply && S.run) cancelRun(); persist(); renderOverlays(); updateUI(); }));
  $$('input[name=anApply]').forEach(r => r.addEventListener('change', () => { S.an.apply = r.value === '1'; persist(); renderOverlays(); updateUI(); }));
  $$('input[name=rmMode]').forEach(r => r.addEventListener('change', () => { S.rm.mode = r.value; S.rmDone = false; persist(); updateUI(); }));
  $$('.chev').forEach(b => b.addEventListener('click', () => setStep(+b.dataset.step)));
  $('file').addEventListener('change', e => openFile(e.target.files[0]));
  $('btnPrev').onclick = () => gotoPage(S.cur - 1); $('btnNext').onclick = () => gotoPage(S.cur + 1);
  $('btnClose').onclick = () => { if (!S.exported && (Object.keys(S.place).length || S.rmDone) && !confirm('Fermer ce PDF ? Le foliotage non exporté sera perdu.')) return; closeFile(); };
  $('rmStartBtn').onclick = startRun; $('runOk').onclick = validateRun; $('runSkip').onclick = skipRun; $('runPrev').onclick = prevRun; $('runCancel').onclick = cancelRun;
  $('runKeep').onchange = e => { S.set.keepPos = e.target.checked; $('setKeep').checked = S.set.keepPos; persist(); };
  $('runRot').onclick = () => { if (S.run) { S.run.pending.orient = ((S.run.pending.orient || 0) + 90) % 360; renderOverlays(); } };
  $('runZm').onclick = () => { S.rm.zoom = Math.max(0.5, Math.round((S.rm.zoom - 0.1) * 10) / 10); $('rmZoom').value = S.rm.zoom; persist(); renderOverlays(); };
  $('runZp').onclick = () => { S.rm.zoom = Math.min(3, Math.round((S.rm.zoom + 0.1) * 10) / 10); $('rmZoom').value = S.rm.zoom; persist(); renderOverlays(); };
  $('anAll').onclick = () => setAnPages(''); $('anCur').onclick = () => S.n && setAnPages(String(S.cur + 1));
  $('anEven').onclick = () => S.n && setAnPages(Array.from({ length: S.n }, (_, i) => i + 1).filter(p => p % 2 === 0).join(','));
  $('anOdd').onclick = () => S.n && setAnPages(Array.from({ length: S.n }, (_, i) => i + 1).filter(p => p % 2 === 1).join(','));
  $$('#anPos button').forEach(b => b.onclick = () => placeAn(+b.dataset.h, +b.dataset.v));
  $('outName').addEventListener('input', e => { S.outName = e.target.value; });
  $('exportBtn').onclick = exportPdf;
  $('primary').onclick = () => {
    if (S.step === 0) setStep(1);
    else if (S.step === 1) { if (S.run) validateRun(); else if (!S.rm.apply || S.rmDone) setStep(2); else startRun(); }
    else if (S.step === 2) setStep(3); else exportPdf();
  };
  // zoom
  $('zIn').onclick = () => setZoom('custom', S.zoom.k * 1.25); $('zOut').onclick = () => setZoom('custom', S.zoom.k * 0.8);
  $('zFit').onclick = () => setZoom('fit'); $('zWidth').onclick = () => setZoom('width');
  addEventListener('resize', () => S.doc && S.zoom.mode !== 'custom' && setZoom(S.zoom.mode));
  const sc = $('scroller');
  sc.addEventListener('wheel', e => {
    if (!S.doc) return;
    if (e.ctrlKey) { e.preventDefault(); setZoom('custom', S.zoom.k * (e.deltaY < 0 ? 1.1 : 1 / 1.1)); return; }
    if (S.run || sc.scrollHeight > sc.clientHeight + 2 || sc.scrollWidth > sc.clientWidth + 2) { S.wheel = 0; return; }
    e.preventDefault(); S.wheel += e.deltaY;
    if (Math.abs(S.wheel) >= 40) { const d = Math.sign(S.wheel); S.wheel = 0; gotoPage(S.cur + d); }
  }, { passive: false });
  // placement
  $('wrap').addEventListener('pointerdown', onPointerDown);
  $('wrap').addEventListener('contextmenu', e => { if (S.run && S.step === 1) { e.preventDefault(); validateRun(); } });
  // glisser-déposer
  const drop = $('drop');
  ['dragover', 'dragenter'].forEach(t => addEventListener(t, e => { e.preventDefault(); drop.classList.add('over'); }));
  ['dragleave', 'drop'].forEach(t => addEventListener(t, e => { e.preventDefault(); drop.classList.remove('over'); }));
  addEventListener('drop', e => { const f = [...(e.dataTransfer?.files || [])].find(x => /pdf$/i.test(x.type) || /\.pdf$/i.test(x.name)); if (f) openFile(f); });
  // réglages
  $('btnSettings').onclick = () => $('settings').showModal();
  $('setTheme').onchange = e => { S.set.theme = e.target.value; applyTheme(); persist(); };
  $('setBase').oninput = e => { S.set.baseSize = St.clamp(num(e.target.value, 9), 5, 30); persist(); renderOverlays(); };
  $('setFill').onchange = e => { S.set.fill = e.target.checked; persist(); renderOverlays(); };
  $('setKeep').onchange = e => { S.set.keepPos = e.target.checked; persist(); };
  $('setReset').onclick = () => { try { localStorage.removeItem(KEY); } catch (_) { /* ignoré */ } S.rm = DEF_RM(); S.an = DEF_AN(); S.set = DEF_SET(); S.arrets = ['TEM']; if (S.doc) { S.rm.to = S.n; } applyTheme(); syncForm(); renderOverlays(); updateUI(); say('Réglages réinitialisés.'); };
  // clavier
  addEventListener('keydown', onKey);
}

function onKey(e) {
  const t = e.target, typing = t && (t.tagName === 'INPUT' && !['checkbox', 'radio', 'button'].includes(t.type) || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT');
  if ($('settings').open) return;
  const mod = e.ctrlKey || e.metaKey;
  if (mod && e.key.toLowerCase() === 'o') { e.preventDefault(); $('file').click(); return; }
  if (mod && e.key.toLowerCase() === 's') { e.preventDefault(); if (S.step === 3) exportPdf(); else if (S.doc) setStep(3); return; }
  if (e.altKey && /^[1-4]$/.test(e.key)) { e.preventDefault(); setStep(+e.key - 1); return; }
  if (e.altKey && e.key === ',') { e.preventDefault(); $('settings').showModal(); return; }
  if (e.key === 'Escape') { if (typing) { t.blur(); return; } if (S.run) cancelRun(); return; }
  if (typing) return;
  if (S.run && S.step === 1 && e.key === 'Enter') { e.preventDefault(); validateRun(); return; }
  const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  if (arrows[e.key] && !mod) { const s = e.shiftKey ? 10 : 1; if (nudge(arrows[e.key][0] * s, arrows[e.key][1] * s)) e.preventDefault(); return; }
  if (!S.run && S.doc) {
    if (e.key === 'PageDown') { e.preventDefault(); gotoPage(S.cur + 1); } else if (e.key === 'PageUp') { e.preventDefault(); gotoPage(S.cur - 1); }
    else if (e.key === 'Home') { e.preventDefault(); gotoPage(0); } else if (e.key === 'End') { e.preventDefault(); gotoPage(S.n - 1); }
  }
}

// ───────────────────────── démarrage ─────────────────────────
(async function init() {
  $('ver').textContent = 'v' + APP_VERSION; $('verAbout').textContent = 'v' + APP_VERSION;
  restore(); applyTheme(); buildTranches(); syncForm();
  const d = await PDFDocument.create();
  S.fonts = { reg: await d.embedFont(StandardFonts.Helvetica), bold: await d.embedFont(StandardFonts.HelveticaBold) };
  wire(); showStepPanel(); updateUI();
  say('Ouvrez un PDF (Ctrl+O, bouton ou glisser-déposer).');
})();
