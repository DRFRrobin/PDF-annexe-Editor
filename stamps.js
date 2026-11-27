/* Annexio — géométrie et dessin des tampons (cartouche ReeMaker + tampon « Annexe X Folio n/N »).
 * Module sans interface : utilisé par app.js dans le navigateur et par tests/stamps.test.js sous Node.
 * Tout est en points PDF ; les positions sont des fractions de la page telle qu'on la voit (rotation incluse). */
(function (root) {
  'use strict';
  const L = () => root.PDFLib;
  const RED = () => L().rgb(0.85, 0, 0);

  // ───────────── utilitaires ─────────────
  const clamp = (v, a, b) => Math.min(Math.max(v, a), b);

  /** Remplace les caractères que la police standard ne sait pas écrire. */
  function safe(font, text) {
    let out = '';
    for (const ch of String(text)) {
      try { font.widthOfTextAtSize(ch, 10); out += ch; } catch (_) { out += '?'; }
    }
    return out;
  }
  const width = (font, text, size) => font.widthOfTextAtSize(safe(font, text), size);

  /** « 1-3,5 » -> Set de numéros de page (1-based) ; vide = toutes les pages. */
  function parseRange(s, n) {
    if (!String(s).trim()) return new Set(Array.from({ length: n }, (_, i) => i + 1));
    const out = new Set();
    for (const part of String(s).split(',')) {
      const m = part.trim().match(/^(\d+)(?:\s*-\s*(\d+))?$/);
      if (!m) throw new Error('Plage invalide : « ' + part.trim() + ' »');
      const a = +m[1], b = m[2] ? +m[2] : a;
      for (let i = Math.min(a, b); i <= Math.max(a, b); i++) if (i >= 1 && i <= n) out.add(i);
    }
    return out;
  }

  // ───────────── page : taille visible et conversion de coordonnées ─────────────
  function pageGeo(page) {
    const rot = ((page.getRotation().angle % 360) + 360) % 360;
    const { width: W, height: H } = page.getSize();
    const swap = rot % 180 !== 0;
    return { rot, W, H, VW: swap ? H : W, VH: swap ? W : H };
  }
  /** Point de la page vue (x, y depuis le coin haut-gauche) -> coordonnées PDF non tournées. */
  function visToPdf(g, x, y) {
    switch (g.rot) {
      case 0: return { x, y: g.H - y };
      case 90: return { x: y, y: x };
      case 180: return { x: g.W - x, y };
      default: return { x: g.W - y, y: g.H - x };
    }
  }

  // ───────────── cartouche (3 lignes) ─────────────
  /** Libellé des tranches cochées : [0] -> « 0 », [1,2] -> « 1-2 » (consécutives) ou « 1, 3 ». */
  function trancheLabel(list) {
    const t = [...new Set(list)].sort((a, b) => a - b);
    if (!t.length) return '';
    if (t.length > 2 && t.every((v, i) => i === 0 || v === t[i - 1] + 1)) return t[0] + '-' + t[t.length - 1];
    return t.join(', ');
  }

  function cartTexts(rm, folio) {
    const tr = trancheLabel(rm.tranches || []);
    const arret = String(rm.arret || '').trim();
    const cycle = String(rm.cycle || '').trim();
    return {
      l1: 'Site de ' + String(rm.site || '').trim() + (rm.cartVer === '1' && tr ? ' Tr ' + tr : ''),
      tr: rm.cartVer !== '1' && tr ? 'Tr ' + tr : '',
      l2a: 'REE ' + String(rm.ree || '').trim(),
      l2b: 'Ind. ' + String(rm.indice || '').trim(),
      l3a: 'Folio ' + folio,
      l3b: arret + (rm.showCycle && cycle ? ' ' + cycle : ''),
    };
  }

  /** Cartouche modèle (gabarit fixe, mesuré sur le cartouche d'origine). Toutes les cotes sont des multiples
   *  de la taille de police : un texte trop long déborde comme sur l'original (ex. « Tr 0 » sur le nom du site).
   *  Origine en haut à gauche, y vers le bas. */
  function cartLayout(font, rm, folio) {
    const fs = (rm.baseSize || 9) * (rm.zoom || 1);
    if (rm.cartVer === '1') return cartLayoutV1(font, rm, folio, fs);
    const rowH = fs * 2.0, bw = Math.max(0.6, fs * 0.13), padx = fs * 0.47;
    const W = fs * 13.55, d2 = fs * 8.9, d3 = fs * 6.17;
    const t = cartTexts(rm, folio);
    // items : x = bord gauche du texte, ou bord droit si right
    const items = [
      { s: t.l1, x: padx, row: 0 }, { s: t.tr, x: fs * 11.3, row: 0 },
      { s: t.l2a, x: padx, row: 1 }, { s: t.l2b, x: d2 + fs * 0.56, row: 1 },
      { s: t.l3a, x: padx, row: 2 }, { s: t.l3b, x: W - fs * 0.46, row: 2, right: true },
    ].filter(it => it.s);
    return { fs, padx, rowH, bw, base: 0.42, W, H: 3 * rowH, d2, d3, leftW: d2, rightW: W - d2, t, items };
  }
  /** V1 : gabarit adaptatif (la largeur suit le texte, « Ind. »/arrêt centrés). */
  function cartLayoutV1(font, rm, folio, fs) {
    const padx = fs * 0.45, pady = fs * 0.22, rowH = fs * 1.2 + 2 * pady, bw = Math.max(0.6, fs * 0.09);
    const t = cartTexts(rm, folio);
    const rightW = Math.max(width(font, t.l2b, fs), width(font, t.l3b, fs)) + 2 * padx;
    const leftMin = Math.max(width(font, t.l2a, fs), width(font, t.l3a, fs)) + 2 * padx;
    const W = Math.max(width(font, t.l1, fs) + 2 * padx, leftMin + rightW), leftW = W - rightW;
    const c = str => leftW + (rightW - width(font, str, fs)) / 2;
    const items = [{ s: t.l1, x: padx, row: 0 }, { s: t.l2a, x: padx, row: 1 }, { s: t.l2b, x: c(t.l2b), row: 1 },
      { s: t.l3a, x: padx, row: 2 }, { s: t.l3b, x: c(t.l3b), row: 2 }].filter(it => it.s);
    return { fs, padx, rowH, bw, base: 0.35, W, H: 3 * rowH, d2: leftW, d3: leftW, leftW, rightW, t, items };
  }
  const bbox = (ly, orient) => (orient === 90 || orient === 270) ? { w: ly.H, h: ly.W } : { w: ly.W, h: ly.H };

  /** Coin haut-gauche (vue) du cadre englobant, borné à la page. */
  function place(p, g, bb) {
    return {
      vx: clamp(p.fx * g.VW, 0, Math.max(0, g.VW - bb.w)),
      vy: clamp(p.fy * g.VH, 0, Math.max(0, g.VH - bb.h)),
    };
  }
  /** Point local du cartouche (lx, ly) -> page vue, selon l'orientation (sens horaire). */
  function localToVisual(ly, orient, vx, vy, lx, lyy) {
    switch (orient) {
      case 90: return { x: vx + (ly.H - lyy), y: vy + lx };
      case 180: return { x: vx + ly.W - lx, y: vy + ly.H - lyy };
      case 270: return { x: vx + lyy, y: vy + ly.W - lx };
      default: return { x: vx + lx, y: vy + lyy };
    }
  }

  function drawCartouche(page, font, ly, placement, opts) {
    const { rgb, degrees, LineCapStyle, pushGraphicsState, popGraphicsState, setFillingColor, moveTo, lineTo, closePath, fill } = L();
    const g = pageGeo(page), orient = placement.orient || 0, bb = bbox(ly, orient);
    const { vx, vy } = place(placement, g, bb);
    const P = (lx, lyy) => { const v = localToVisual(ly, orient, vx, vy, lx, lyy); return visToPdf(g, v.x, v.y); };
    const color = RED();
    if (!opts || opts.fill !== false) {
      const pts = [P(0, 0), P(ly.W, 0), P(ly.W, ly.H), P(0, ly.H)];
      page.pushOperators(pushGraphicsState(), setFillingColor(rgb(1, 1, 1)), moveTo(pts[0].x, pts[0].y),
        lineTo(pts[1].x, pts[1].y), lineTo(pts[2].x, pts[2].y), lineTo(pts[3].x, pts[3].y), closePath(), fill(), popGraphicsState());
    }
    const line = (a, b) => page.drawLine({ start: P(a[0], a[1]), end: P(b[0], b[1]), thickness: ly.bw, color, lineCap: LineCapStyle.Projecting });
    const { W, H, rowH } = ly;
    line([0, 0], [W, 0]); line([W, 0], [W, H]); line([W, H], [0, H]); line([0, H], [0, 0]);
    line([0, rowH], [W, rowH]); line([0, 2 * rowH], [W, 2 * rowH]); line([ly.d2, rowH], [ly.d2, 2 * rowH]); line([ly.d3, 2 * rowH], [ly.d3, H]);
    const angle = ((g.rot - orient) % 360 + 360) % 360;
    for (const it of ly.items) {
      const x = it.right ? it.x - width(font, it.s, ly.fs) : it.x;
      const p = P(x, it.row * rowH + rowH / 2 + ly.base * ly.fs);
      page.drawText(safe(font, it.s), { x: p.x, y: p.y, size: ly.fs, font, color, rotate: degrees(angle) });
    }
  }

  // ───────────── tampon « Annexe X Folio n/N » (une ligne, gras) ─────────────
  const A_PAD = 4, A_BORDER = 1.5;
  function annexText(an, pageNo, total) {
    const tot = parseInt(an.total) || total;
    return String(an.template).replace(/\{annexe\}/g, an.annexe).replace(/\{n\}/g, (parseInt(an.start) || 1) + pageNo - 1).replace(/\{total\}/g, tot);
  }
  function annexLayout(fontBold, text, size) {
    return { w: width(fontBold, text, size) + 2 * A_PAD + 2 * A_BORDER, h: size * 1.2 + 2 * A_PAD + 2 * A_BORDER, size, text };
  }
  function drawAnnexio(page, fontBold, text, an) {
    const { rgb, degrees } = L();
    const g = pageGeo(page), s = an.size, ly = annexLayout(fontBold, text, s);
    const { vx, vy } = place(an, g, { w: ly.w, h: ly.h });
    const map = (x, y) => visToPdf(g, x, y);
    const o = map(vx, vy + ly.h), t = map(vx + A_BORDER + A_PAD, vy + ly.h - A_BORDER - A_PAD - s * 0.22);
    const red = RED();
    page.drawRectangle({ x: o.x, y: o.y, width: ly.w, height: ly.h, rotate: degrees(g.rot), borderColor: red, borderWidth: A_BORDER,
      color: rgb(1, 1, 1), opacity: an.fill ? 1 : 0, borderOpacity: 1 });
    page.drawText(safe(fontBold, text), { x: t.x, y: t.y, size: s, font: fontBold, color: red, rotate: degrees(g.rot) });
  }

  // ───────────── orientation automatique (navigateur : page pdf.js) ─────────────
  /** Sens de lecture dominant du texte, vu à l'écran : 0, 90 (haut→bas), 180, 270 (bas→haut). */
  async function detectOrientation(pdfjsPage) {
    try {
      const vt = pdfjsPage.getViewport({ scale: 1 }).transform;
      const tc = await pdfjsPage.getTextContent();
      const w = { 0: 0, 90: 0, 180: 0, 270: 0 };
      let total = 0;
      for (const it of tc.items) {
        const n = (it.str || '').trim().length;
        if (!n || !it.transform) continue;
        const a = it.transform[0], b = it.transform[1];
        const dx = vt[0] * a + vt[2] * b, dy = vt[1] * a + vt[3] * b;
        const ang = ((Math.round(Math.atan2(dy, dx) * 180 / Math.PI / 90) * 90) % 360 + 360) % 360;
        w[ang] += n; total += n;
      }
      if (!total) return 0;
      const best = Object.keys(w).reduce((x, y) => (w[y] > w[x] ? y : x));
      return w[best] >= total * 0.4 ? +best : 0;
    } catch (_) { return 0; }
  }

  // ───────────── nom du fichier exporté ─────────────
  const FORBIDDEN = /[\\/:*?"<>|\u0000-\u001f]/g;
  /** Modèle -> nom. [ … ] = segment facultatif, supprimé si une variable qu'il contient est vide.
   *  Ex. « [REE {ree}][ Tr {tranche}][ Ind {indice}] » -> « REE JPJ 701 Tr 0 Ind A ». */
  function buildName(tpl, vars) {
    const val = k => String(vars[k] == null ? '' : vars[k]).trim();
    const fill = s => s.replace(/\{(\w+)\}/g, (_, k) => val(k));
    const hasEmpty = s => [...s.matchAll(/\{(\w+)\}/g)].some(m => !val(m[1]));
    let out = String(tpl).replace(/\[([^\]]*)\]/g, (_, seg) => (hasEmpty(seg) ? '' : fill(seg)));
    out = fill(out).replace(FORBIDDEN, '-').replace(/\s+/g, ' ').trim().replace(/[. ]+$/, '');
    return out;
  }
  /** Nom proposé à l'export : d'après ReeMaker s'il est rempli, sinon « <original>_folioté ». */
  function exportName(tpl, rm, original, folios) {
    const tr = trancheLabel(rm.tranches || []).replace(/, /g, ',');
    const vars = { ree: rm.ree, tranche: tr, indice: rm.indice, site: rm.site, arret: rm.arret, cycle: rm.cycle,
      nom: original, folio1: folios && folios[0], folio2: folios && folios[1],
      // folios du cartouche ReeMaker : « 36 à 54 », ou « 36 » si une seule page
      folios: folios ? (folios[0] === folios[1] ? String(folios[0]) : folios[0] + ' à ' + folios[1]) : '' };
    const filled = rm.apply && (String(rm.ree || '').trim() || String(rm.indice || '').trim()); // la tranche seule (0 par défaut) ne suffit pas
    const name = filled ? buildName(tpl, vars) : '';
    return (name || original + '_folioté') + '.pdf';
  }

  root.Stamps = { buildName, exportName, clamp, safe, width, parseRange, pageGeo, visToPdf, trancheLabel, cartTexts, cartLayout, bbox, place,
    localToVisual, drawCartouche, annexText, annexLayout, drawAnnexio, detectOrientation, A_PAD, A_BORDER };
})(typeof window !== 'undefined' ? window : globalThis);
