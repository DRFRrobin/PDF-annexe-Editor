// Tests du module stamps.js : node tests/stamps.test.js  (écrit aussi un PDF d'essai si OUT est défini)
'use strict';
const assert = require('assert');
globalThis.PDFLib = require('../vendor/pdf-lib.min.js');
require('../stamps.js');
const S = globalThis.Stamps;
const { PDFDocument, StandardFonts, degrees } = PDFLib;

(async () => {
  // trancheLabel / parseRange
  assert.strictEqual(S.trancheLabel([0]), '0');
  assert.strictEqual(S.trancheLabel([2, 1]), '1, 2');
  assert.strictEqual(S.trancheLabel([1, 2, 3, 4]), '1-4');
  assert.strictEqual(S.trancheLabel([1, 3]), '1, 3');
  assert.deepStrictEqual([...S.parseRange('1-3,5', 10)], [1, 2, 3, 5]);
  assert.strictEqual(S.parseRange('', 4).size, 4);
  assert.throws(() => S.parseRange('a', 4));

  // nom du fichier exporté
  const T = '[REE {ree}][ Tr {tranche}][ Ind {indice}]';
  assert.strictEqual(S.buildName(T, { ree: 'JPJ 701', tranche: '0', indice: 'A' }), 'REE JPJ 701 Tr 0 Ind A');
  assert.strictEqual(S.buildName(T, { ree: 'JPJ 701', tranche: '', indice: 'A' }), 'REE JPJ 701 Ind A');
  assert.strictEqual(S.buildName(T, { ree: '', tranche: '1', indice: '' }), 'Tr 1');
  assert.strictEqual(S.buildName(T, {}), '');
  assert.strictEqual(S.buildName('[REE {ree}]', { ree: 'JPJ/701:x' }), 'REE JPJ-701-x');
  assert.strictEqual(S.buildName('{site} - {nom}', { site: 'A', nom: 'doc' }), 'A - doc');
  const rmN = { apply: true, ree: 'JPJ 701', indice: 'A', tranches: [0], site: 'S' };
  assert.strictEqual(S.exportName(T, rmN, 'FNC 228', null), 'REE JPJ 701 Tr 0 Ind A.pdf');
  assert.strictEqual(S.exportName(T, { ...rmN, tranches: [1, 2] }, 'FNC 228', null), 'REE JPJ 701 Tr 1,2 Ind A.pdf');
  assert.strictEqual(S.exportName(T, { ...rmN, apply: false }, 'FNC 228', null), 'FNC 228_folioté.pdf'); // ReeMaker ignoré
  assert.strictEqual(S.exportName(T, { apply: true, tranches: [0] }, 'FNC 228', null), 'FNC 228_folioté.pdf'); // seule la tranche par défaut

  const doc = await PDFDocument.create();
  const reg = await doc.embedFont(StandardFonts.Helvetica), bold = await doc.embedFont(StandardFonts.HelveticaBold);

  // texte du cartouche
  const rm = { site: 'SITE EXEMPLE', ree: 'REE 001', indice: 'A', arret: 'TEM', cycle: '36', showCycle: false, tranches: [0], zoom: 1.2, baseSize: 9 };
  let ly = S.cartLayout(reg, rm, 46);
  assert.deepStrictEqual(ly.t, { l1: 'Site de SITE EXEMPLE Tr 0', l2a: 'REE REE 001', l2b: 'Ind. A', l3a: 'Folio 46', l3b: 'TEM' });
  assert.ok(ly.W > ly.leftW && ly.leftW > 0 && Math.abs(ly.H - 3 * ly.rowH) < 1e-9);
  assert.strictEqual(S.cartLayout(reg, { ...rm, showCycle: true }, 46).t.l3b, 'TEM 36');
  // le zoom agrandit proportionnellement
  const big = S.cartLayout(reg, { ...rm, zoom: 2.4 }, 46);
  assert.ok(Math.abs(big.W / ly.W - 2) < 1e-6);
  // caractère non encodable : remplacé, pas d'exception
  assert.strictEqual(S.safe(reg, 'a€b☃c').includes('☃'), false);

  // cohérence local -> visuel : les 4 coins d'un cadre tourné restent dans le cadre englobant
  for (const o of [0, 90, 180, 270]) {
    const bb = S.bbox(ly, o);
    for (const [x, y] of [[0, 0], [ly.W, 0], [ly.W, ly.H], [0, ly.H]]) {
      const v = S.localToVisual(ly, o, 100, 50, x, y);
      assert.ok(v.x >= 100 - 1e-6 && v.x <= 100 + bb.w + 1e-6 && v.y >= 50 - 1e-6 && v.y <= 50 + bb.h + 1e-6, `coin hors cadre (${o}°)`);
    }
  }

  // PDF d'essai : 4 pages, rotations 0/90/180/270 ; cartouche (0° et 90°) + tampon Annexe
  const pages = [];
  for (const r of [0, 90, 180, 270]) { const p = doc.addPage([420, 300]); p.setRotation(degrees(r)); pages.push(p); }
  for (let i = 0; i < pages.length; i++) {
    const p = pages[i];
    S.drawCartouche(p, reg, S.cartLayout(reg, rm, 46 + i), { fx: 0.55, fy: 0.05, orient: 0 }, { fill: true });
    S.drawCartouche(p, reg, S.cartLayout(reg, rm, 46 + i), { fx: 0.05, fy: 0.35, orient: 90 }, { fill: true });
    const an = { annexe: '1', template: 'Annexe {annexe} Folio {n}/{total}', start: 1, total: '', size: 12, fill: true, fx: 0.05, fy: 0.88 };
    S.drawAnnexio(p, bold, S.annexText(an, i + 1, 4), an);
  }
  const out = await doc.save();
  assert.ok(out.length > 1000);
  require('fs').writeFileSync(process.env.OUT || '/tmp/stamps-test.pdf', out);
  console.log('stamps.test.js : OK');
})().catch(e => { console.error(e); process.exit(1); });
