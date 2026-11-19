'use strict';
const APP_VERSION = '1.1.12'; // +0.0.1 à chaque mise à jour (voir CHANGELOG.md)
const { PDFDocument, StandardFonts, rgb, degrees } = PDFLib;
pdfjsLib.GlobalWorkerOptions.workerSrc = 'vendor/pdf.worker.min.js';
const $ = id => document.getElementById(id);
$('ver').textContent = 'v' + APP_VERSION;
const PAD = 4, BORDER = 1.5;
let bytes = null, pdfjsDoc = null, numPages = 0, cur = 1, scale = 1, fileName = 'document.pdf';
let pos = { fx: 0.55, fy: 0.03 };            // coin haut-gauche, fraction de la page (vue affichée)
let helv = null;                              // police de mesure
let pageVis = { w: 1, h: 1 };                 // taille visuelle (pt) de la page affichée

const tplText = n => {
  const total = parseInt($('total').value) || numPages;
  return $('tpl').value.replace(/\{annexe\}/g, $('annexe').value)
    .replace(/\{n\}/g, n).replace(/\{total\}/g, total);
};
const folio = p => (parseInt($('start').value) || 1) + p - 1;
const size = () => Math.max(6, parseInt($('size').value) || 14);

function parseRange(s) {
  if (!s.trim()) return new Set(Array.from({ length: numPages }, (_, i) => i + 1));
  const out = new Set();
  for (const part of s.split(',')) {
    const m = part.trim().match(/^(\d+)(?:\s*-\s*(\d+))?$/);
    if (!m) throw new Error('Plage invalide : « ' + part.trim() + ' »');
    const a = +m[1], b = m[2] ? +m[2] : a;
    for (let i = Math.min(a, b); i <= Math.max(a, b); i++) if (i >= 1 && i <= numPages) out.add(i);
  }
  return out;
}

async function load(file) {
  msg('');
  try {
    bytes = new Uint8Array(await file.arrayBuffer());
    pdfjsDoc = await pdfjsLib.getDocument({ data: bytes.slice() }).promise;
  } catch (e) { return msg('PDF illisible : ' + e.message, true); }
  numPages = pdfjsDoc.numPages; cur = 1; fileName = file.name;
  helv = await (await PDFDocument.create()).embedFont(StandardFonts.HelveticaBold);
  $('opts').disabled = false; $('dropText').textContent = file.name + ' (' + numPages + ' p.)';
  $('wrap').hidden = false; $('empty').hidden = true;
  await render();
}

async function render() {
  const page = await pdfjsDoc.getPage(cur);
  const avail = Math.max(300, $('stage').clientWidth - 8);
  const base = page.getViewport({ scale: 1 });
  scale = Math.min(avail / base.width, 900 / base.width * 1.0, 1.6);
  const vp = page.getViewport({ scale });
  const cv = $('cv'), r = devicePixelRatio || 1;
  cv.width = vp.width * r; cv.height = vp.height * r;
  cv.style.width = vp.width + 'px'; cv.style.height = vp.height + 'px';
  pageVis = { w: base.width, h: base.height };
  await page.render({ canvasContext: cv.getContext('2d'), viewport: vp, transform: r !== 1 ? [r, 0, 0, r, 0, 0] : null }).promise;
  $('pg').textContent = cur + '/' + numPages;
  updateBox();
}

function boxSize(text) {
  const s = size();
  return { w: helv.widthOfTextAtSize(text, s) + 2 * PAD + 2 * BORDER, h: s * 1.2 + 2 * PAD + 2 * BORDER };
}

function updateBox() {
  if (!bytes) return;
  const text = tplText(folio(cur)), s = size(), b = boxSize(text), el = $('box');
  const maxX = Math.max(0, pageVis.w - b.w), maxY = Math.max(0, pageVis.h - b.h);
  const x = Math.min(Math.max(pos.fx * pageVis.w, 0), maxX), y = Math.min(Math.max(pos.fy * pageVis.h, 0), maxY);
  el.textContent = text;
  Object.assign(el.style, {
    left: x * scale + 'px', top: y * scale + 'px', width: b.w * scale + 'px', height: b.h * scale + 'px',
    fontSize: s * scale + 'px', background: $('fill').checked ? '#fff' : 'transparent', borderWidth: BORDER * scale + 'px',
  });
}

function setPosFromPointer(e, offX, offY) {
  const r = $('wrap').getBoundingClientRect(), b = boxSize(tplText(folio(cur)));
  const x = (e.clientX - r.left) / scale - offX, y = (e.clientY - r.top) / scale - offY;
  pos.fx = Math.min(Math.max(x, 0), pageVis.w - b.w) / pageVis.w;
  pos.fy = Math.min(Math.max(y, 0), pageVis.h - b.h) / pageVis.h;
  updateBox();
}

$('box').addEventListener('pointerdown', e => {
  e.preventDefault(); e.stopPropagation();
  const br = $('box').getBoundingClientRect();
  const ox = (e.clientX - br.left) / scale, oy = (e.clientY - br.top) / scale;
  const mv = ev => setPosFromPointer(ev, ox, oy);
  const up = () => { removeEventListener('pointermove', mv); removeEventListener('pointerup', up); };
  addEventListener('pointermove', mv); addEventListener('pointerup', up);
});
$('wrap').addEventListener('pointerdown', e => {          // clic = place le centre du rectangle
  if (e.target === $('box')) return;
  const b = boxSize(tplText(folio(cur)));
  setPosFromPointer(e, b.w / 2, b.h / 2);
});

for (const id of ['annexe', 'tpl', 'start', 'total', 'size', 'fill'])
  $(id).addEventListener('input', updateBox);
$('prev').onclick = () => { if (cur > 1) { cur--; render(); } };
$('next').onclick = () => { if (cur < numPages) { cur++; render(); } };
addEventListener('resize', () => bytes && render());

function msg(t, err) { const m = $('msg'); m.textContent = t; m.style.color = err ? '#c00' : '#060'; }

async function generate() {
  $('go').disabled = true; msg('Traitement…');
  try {
    const set = parseRange($('range').value);
    if (!set.size) throw new Error('Aucune page sélectionnée.');
    const doc = await PDFDocument.load(bytes, { ignoreEncryption: true });
    const font = await doc.embedFont(StandardFonts.HelveticaBold);
    const s = size(), red = rgb(0.87, 0, 0), pages = doc.getPages();
    pages.forEach((page, i) => {
      const p = i + 1; if (!set.has(p)) return;
      const text = tplText(folio(p));
      const bw = font.widthOfTextAtSize(text, s) + 2 * PAD + 2 * BORDER, bh = s * 1.2 + 2 * PAD + 2 * BORDER;
      const rot = ((page.getRotation().angle % 360) + 360) % 360;
      const { width: W, height: H } = page.getSize();
      const VW = rot % 180 ? H : W, VH = rot % 180 ? W : H;
      const vx = Math.min(Math.max(pos.fx * VW, 0), Math.max(0, VW - bw));
      const vy = Math.min(Math.max(pos.fy * VH, 0), Math.max(0, VH - bh));
      // point visuel (depuis le coin haut-gauche) -> coordonnées PDF non tournées
      const map = (x, y) => rot === 0 ? { x, y: H - y } : rot === 90 ? { x: y, y: x }
        : rot === 180 ? { x: W - x, y } : { x: W - y, y: H - x };
      const o = map(vx, vy + bh), t = map(vx + BORDER + PAD, vy + bh - BORDER - PAD - s * 0.22);
      page.drawRectangle({ x: o.x, y: o.y, width: bw, height: bh, rotate: degrees(rot),
        borderColor: red, borderWidth: BORDER, color: rgb(1, 1, 1), opacity: $('fill').checked ? 1 : 0, borderOpacity: 1 });
      page.drawText(text, { x: t.x, y: t.y, size: s, font, color: red, rotate: degrees(rot) });
    });
    const out = await doc.save();
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([out], { type: 'application/pdf' }));
    a.download = fileName.replace(/\.pdf$/i, '') + '_annexe.pdf';
    a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 5000);
    msg('Terminé (' + set.size + ' page(s) tamponnée(s)).');
  } catch (e) { msg(e.message, true); }
  $('go').disabled = false;
}
$('go').onclick = generate;

$('file').addEventListener('change', e => e.target.files[0] && load(e.target.files[0]));
const drop = $('drop');
['dragover', 'dragenter'].forEach(t => drop.addEventListener(t, e => { e.preventDefault(); drop.classList.add('over'); }));
['dragleave', 'drop'].forEach(t => drop.addEventListener(t, e => { e.preventDefault(); drop.classList.remove('over'); }));
drop.addEventListener('drop', e => { const f = e.dataTransfer.files[0]; if (f) load(f); });
