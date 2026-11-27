//! ReeMaker : cartouche à 3 lignes (site + tranche · REE | indice · folio | type d'arrêt), nommage automatique
//! du PDF exporté, orientation automatique d'après le sens du texte, et export cumulé avec le tampon « Annexe ».
//! Même logique que `stamps.js` (version web) : positions en fraction de la page *vue* (rotation incluse).

use crate::{add_font_to_page, isolate_existing, media_box, parse_range, pdf_string, render_text, rotation, stamp_content, win_ansi, Options};
use lopdf::{content::Content, dictionary, Document, Object, ObjectId};
use std::collections::BTreeMap;

pub const REG_FONT: &str = "FRee";

// ───────────────────────────── paramètres ─────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Rm {
    pub site: String,
    pub ree: String,
    pub indice: String,
    pub start: u32,
    pub arret: String,
    pub cycle: String,
    pub show_cycle: bool,
    pub tranches: Vec<u8>,
    pub zoom: f32,
    pub base_size: f32,
    /// 0 = V0 (gabarit du cartouche d'origine), 1 = V1 (mise en page adaptative précédente)
    pub cart_ver: u8,
}

impl Default for Rm {
    fn default() -> Self {
        Rm {
            site: String::new(),
            ree: String::new(),
            indice: String::new(),
            start: 1,
            arret: String::new(),
            cycle: String::new(),
            show_cycle: false,
            tranches: vec![0],
            zoom: 1.2,
            base_size: 9.0,
            cart_ver: 0,
        }
    }
}

/// Position du cartouche : coin haut-gauche du cadre englobant (fraction de la page vue) et sens horaire (0/90/180/270).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub fx: f32,
    pub fy: f32,
    pub orient: u16,
}

/// « 0 » -> « 0 » ; [1,2] -> « 1, 2 » ; [1,2,3,4] -> « 1-4 ».
pub fn tranche_label(list: &[u8]) -> String {
    let mut t: Vec<u8> = list.to_vec();
    t.sort_unstable();
    t.dedup();
    if t.is_empty() {
        return String::new();
    }
    if t.len() > 2 && t.windows(2).all(|w| w[1] == w[0] + 1) {
        return format!("{}-{}", t[0], t[t.len() - 1]);
    }
    t.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}

// ───────────────────────────── mesure du texte (Helvetica) ─────────────────────────────

/// Largeur (1/1000 em) en Helvetica ; accents approchés par la lettre de base.
fn reg_glyph(c: char) -> u32 {
    const W: [u16; 95] = [
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556,
        556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667,
        556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556,
        556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722,
        500, 500, 500, 334, 260, 334, 584,
    ];
    match c {
        ' '..='~' => W[c as usize - 32] as u32,
        'é' | 'è' | 'ê' | 'ë' | 'à' | 'â' | 'ä' => 556,
        'ç' => 500,
        'î' | 'ï' => 278,
        'ô' | 'ö' | 'ù' | 'û' | 'ü' => 556,
        'É' | 'È' | 'Ê' => 667,
        'À' | 'Â' => 667,
        'Ç' => 722,
        '–' => 556,
        '—' => 1000,
        _ => 556,
    }
}
pub fn reg_width(text: &str, size: f32) -> f32 {
    text.chars().map(reg_glyph).sum::<u32>() as f32 * size / 1000.0
}

// ───────────────────────────── mise en page du cartouche ─────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Texts {
    pub l1: String,
    pub tr: String,
    pub l2a: String,
    pub l2b: String,
    pub l3a: String,
    pub l3b: String,
}

pub fn texts(rm: &Rm, folio: u32) -> Texts {
    let tr = tranche_label(&rm.tranches);
    let arret = rm.arret.trim();
    let cycle = rm.cycle.trim();
    Texts {
        l1: format!("Site de {}{}", rm.site.trim(), if tr.is_empty() || rm.cart_ver != 1 { String::new() } else { format!(" Tr {tr}") }),
        tr: if tr.is_empty() || rm.cart_ver == 1 { String::new() } else { format!("Tr {tr}") },
        l2a: format!("REE {}", rm.ree.trim()),
        l2b: format!("Ind. {}", rm.indice.trim()),
        l3a: format!("Folio {folio}"),
        l3b: if rm.show_cycle && !cycle.is_empty() { format!("{arret} {cycle}").trim().to_string() } else { arret.to_string() },
    }
}

/// Dimensions du cartouche non tourné (origine en haut à gauche, y vers le bas), en points.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub fs: f32,
    pub padx: f32,
    pub row_h: f32,
    pub bw: f32,
    pub w: f32,
    pub h: f32,
    pub left_w: f32,
    pub right_w: f32,
    /// traits verticaux des lignes 2 et 3, et ligne de base du texte (fraction de la police)
    pub d2: f32,
    pub d3: f32,
    pub base: f32,
    pub t: Texts,
    pub items: Vec<Item>,
}

/// Texte placé : `x` = bord gauche, ou bord droit si `right`.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub s: String,
    pub x: f32,
    pub row: usize,
    pub right: bool,
}

pub fn layout(rm: &Rm, folio: u32) -> Layout {
    let fs = rm.base_size * rm.zoom;
    let t = texts(rm, folio);
    let it = |s: &str, x: f32, row: usize, right: bool| Item { s: s.to_string(), x, row, right };
    let mut l = if rm.cart_ver == 1 {
        let padx = fs * 0.45;
        let pady = fs * 0.22;
        let row_h = fs * 1.2 + 2.0 * pady;
        let right_w = reg_width(&t.l2b, fs).max(reg_width(&t.l3b, fs)) + 2.0 * padx;
        let left_min = reg_width(&t.l2a, fs).max(reg_width(&t.l3a, fs)) + 2.0 * padx;
        let w = (reg_width(&t.l1, fs) + 2.0 * padx).max(left_min + right_w);
        let left_w = w - right_w;
        let c = |s: &str| left_w + (right_w - reg_width(s, fs)) / 2.0;
        let items = vec![it(&t.l1, padx, 0, false), it(&t.l2a, padx, 1, false), it(&t.l2b, c(&t.l2b), 1, false), it(&t.l3a, padx, 2, false), it(&t.l3b, c(&t.l3b), 2, false)];
        Layout { fs, padx, row_h, bw: (fs * 0.09).max(0.6), w, h: 3.0 * row_h, left_w, right_w, d2: left_w, d3: left_w, base: 0.35, t: t.clone(), items }
    } else {
        // V0 : gabarit fixe mesuré sur le cartouche d'origine ; un texte trop long déborde (« Tr 0 » sur le site)
        let row_h = fs * 2.0;
        let padx = fs * 0.47;
        let (w, d2, d3) = (fs * 13.55, fs * 8.9, fs * 6.17);
        let items = vec![
            it(&t.l1, padx, 0, false),
            it(&t.tr, fs * 11.3, 0, false),
            it(&t.l2a, padx, 1, false),
            it(&t.l2b, d2 + fs * 0.56, 1, false),
            it(&t.l3a, padx, 2, false),
            it(&t.l3b, w - fs * 0.46, 2, true),
        ];
        Layout { fs, padx, row_h, bw: (fs * 0.13).max(0.6), w, h: 3.0 * row_h, left_w: d2, right_w: w - d2, d2, d3, base: 0.42, t: t.clone(), items }
    };
    l.items.retain(|i| !i.s.is_empty());
    l
}

/// Cadre englobant (largeur, hauteur) selon l'orientation.
pub fn bbox(l: &Layout, orient: u16) -> (f32, f32) {
    if orient == 90 || orient == 270 {
        (l.h, l.w)
    } else {
        (l.w, l.h)
    }
}

/// Coin haut-gauche (vue) du cadre englobant, borné à la page.
pub fn place(fx: f32, fy: f32, vw: f32, vh: f32, bb: (f32, f32)) -> (f32, f32) {
    ((fx * vw).clamp(0.0, (vw - bb.0).max(0.0)), (fy * vh).clamp(0.0, (vh - bb.1).max(0.0)))
}

/// Point local du cartouche (x, y vers le bas) -> page vue, orientation en sens horaire.
pub fn local_to_visual(l: &Layout, orient: u16, vx: f32, vy: f32, lx: f32, ly: f32) -> (f32, f32) {
    match orient {
        90 => (vx + (l.h - ly), vy + lx),
        180 => (vx + l.w - lx, vy + l.h - ly),
        270 => (vx + ly, vy + l.w - lx),
        _ => (vx + lx, vy + ly),
    }
}

// ───────────────────────────── page : géométrie ─────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Geo {
    pub rot: i64,
    pub x0: f32,
    pub y0: f32,
    pub w: f32,
    pub h: f32,
    pub vw: f32,
    pub vh: f32,
}

pub fn geo(doc: &Document, page: ObjectId) -> Geo {
    let b = media_box(doc, page);
    let rot = rotation(doc, page);
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    let swap = rot % 180 != 0;
    Geo { rot, x0: b[0], y0: b[1], w, h, vw: if swap { h } else { w }, vh: if swap { w } else { h } }
}

fn vis_to_pdf(g: &Geo, x: f32, y: f32) -> (f32, f32) {
    let (px, py) = match g.rot {
        0 => (x, g.h - y),
        90 => (y, x),
        180 => (g.w - x, y),
        _ => (g.w - y, g.h - x),
    };
    (px + g.x0, py + g.y0)
}

// ───────────────────────────── dessin dans le PDF ─────────────────────────────

/// Opérateurs PDF du cartouche, dans le repère de la page.
pub fn cart_content(l: &Layout, p: &Placement, g: &Geo, fill: bool) -> Vec<u8> {
    let orient = p.orient % 360;
    let bb = bbox(l, orient);
    let (vx, vy) = place(p.fx, p.fy, g.vw, g.vh, bb);
    let (bx, by) = local_to_visual(l, orient, vx, vy, 0.0, l.h); // coin bas-gauche local
    let (ox, oy) = vis_to_pdf(g, bx, by);
    let ang = (g.rot - orient as i64).rem_euclid(360);
    let (c, s) = match ang {
        0 => (1, 0),
        90 => (0, 1),
        180 => (-1, 0),
        _ => (0, -1),
    };
    let mut out = format!("q\n{c} {s} {} {c} {ox:.3} {oy:.3} cm\n", -s).into_bytes();
    if fill {
        out.extend(format!("1 1 1 rg\n0 0 {:.3} {:.3} re f\n", l.w, l.h).into_bytes());
    }
    let y1 = l.h - l.row_h;
    let y2 = l.h - 2.0 * l.row_h;
    out.extend(
        format!(
            "0.85 0 0 RG {:.3} w 2 J 0 j\n0 0 {w:.3} {h:.3} re S\n0 {y1:.3} m {w:.3} {y1:.3} l S\n0 {y2:.3} m {w:.3} {y2:.3} l S\n{d2:.3} {y1:.3} m {d2:.3} {y2:.3} l S\n{d3:.3} {y2:.3} m {d3:.3} 0 l S\n",
            l.bw,
            w = l.w,
            h = l.h,
            d2 = l.d2,
            d3 = l.d3
        )
        .into_bytes(),
    );
    for i in &l.items {
        let x = if i.right { i.x - reg_width(&i.s, l.fs) } else { i.x };
        let y = l.h - (i.row as f32 * l.row_h + l.row_h / 2.0 + l.base * l.fs);
        out.extend(format!("BT 0.85 0 0 rg /{REG_FONT} {:.3} Tf 1 0 0 1 {x:.3} {y:.3} Tm ", l.fs).into_bytes());
        out.extend(pdf_string(&win_ansi(&i.s)));
        out.extend(b" Tj ET\n");
    }
    out.extend(b"Q\n");
    out
}

// ───────────────────────────── nom du fichier exporté ─────────────────────────────

fn sanitize(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || (c as u32) < 32 { '-' } else { c }).collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(|c| c == '.' || c == ' ').to_string()
}

/// Modèle -> nom. `[ … ]` = segment facultatif, supprimé si une variable qu'il contient est vide.
/// Ex. « [REE {ree}][ Tr {tranche}][ Ind {indice}] » -> « REE JPJ 701 Tr 0 Ind A ».
pub fn build_name(tpl: &str, vars: &BTreeMap<&str, String>) -> String {
    let val = |k: &str| vars.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
    let fill = |s: &str| {
        let mut out = String::new();
        let mut rest = s;
        while let Some(a) = rest.find('{') {
            out.push_str(&rest[..a]);
            match rest[a..].find('}') {
                Some(b) => {
                    out.push_str(&val(&rest[a + 1..a + b]));
                    rest = &rest[a + b + 1..];
                }
                None => {
                    out.push_str(&rest[a..]);
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        out
    };
    let has_empty = |s: &str| {
        let mut rest = s;
        while let Some(a) = rest.find('{') {
            let Some(b) = rest[a..].find('}') else { break };
            if val(&rest[a + 1..a + b]).is_empty() {
                return true;
            }
            rest = &rest[a + b + 1..];
        }
        false
    };
    let mut out = String::new();
    let mut rest = tpl;
    while let Some(a) = rest.find('[') {
        out.push_str(&fill(&rest[..a]));
        match rest[a..].find(']') {
            Some(b) => {
                let seg = &rest[a + 1..a + b];
                if !has_empty(seg) {
                    out.push_str(&fill(seg));
                }
                rest = &rest[a + b + 1..];
            }
            None => {
                out.push_str(&fill(&rest[a..]));
                rest = "";
            }
        }
    }
    out.push_str(&fill(rest));
    sanitize(&out)
}

/// Nom proposé à l'export : d'après ReeMaker s'il est appliqué et rempli (REE ou indice), sinon « <original>_folioté.pdf ».
pub fn export_name(tpl: &str, rm: &Rm, apply: bool, original: &str, folios: Option<(u32, u32)>) -> String {
    let tr = tranche_label(&rm.tranches).replace(", ", ",");
    let mut vars: BTreeMap<&str, String> = BTreeMap::new();
    vars.insert("ree", rm.ree.clone());
    vars.insert("tranche", tr);
    vars.insert("indice", rm.indice.clone());
    vars.insert("site", rm.site.clone());
    vars.insert("arret", rm.arret.clone());
    vars.insert("cycle", rm.cycle.clone());
    vars.insert("nom", original.to_string());
    if let Some((a, b)) = folios {
        vars.insert("folio1", a.to_string());
        vars.insert("folio2", b.to_string());
        // folios du cartouche ReeMaker : « 36 à 54 », ou « 36 » si une seule page
        vars.insert("folios", if a == b { a.to_string() } else { format!("{a} à {b}") });
    }
    let filled = apply && (!rm.ree.trim().is_empty() || !rm.indice.trim().is_empty());
    let name = if filled { build_name(tpl, &vars) } else { String::new() };
    format!("{}.pdf", if name.is_empty() { format!("{original}_folioté") } else { name })
}

// ───────────────────────────── orientation automatique ─────────────────────────────

type Mat = [f32; 6];
fn mul(a: &Mat, b: &Mat) -> Mat {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}
fn nums(ops: &[Object]) -> Option<Mat> {
    if ops.len() < 6 {
        return None;
    }
    let mut m = [0.0; 6];
    for (i, o) in ops.iter().take(6).enumerate() {
        m[i] = o.as_float().ok().or_else(|| o.as_i64().ok().map(|v| v as f32))?;
    }
    Some(m)
}
fn str_len(o: &Object) -> usize {
    match o {
        Object::String(b, _) => b.len(),
        Object::Array(a) => a.iter().map(str_len).sum(),
        _ => 0,
    }
}

/// Sens de lecture dominant du texte, tel qu'on voit la page : 0, 90 (haut→bas), 180, 270 (bas→haut).
/// Lit les matrices du flux de contenu (cm, Tm) ; 0 si la page n'a pas de texte (scan).
pub fn detect_orientation(doc: &Document, page: ObjectId) -> u16 {
    let bytes = doc.get_page_content(page);
    let Ok(content) = Content::decode(&bytes) else { return 0 };
    let rot = rotation(doc, page);
    let ident: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let (mut ctm, mut tm, mut stack) = (ident, ident, Vec::<Mat>::new());
    let mut w = [0usize; 4];
    let mut total = 0usize;
    for op in &content.operations {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or(ctm),
            "cm" => {
                if let Some(m) = nums(&op.operands) {
                    ctm = mul(&m, &ctm);
                }
            }
            "BT" => tm = ident,
            "Tm" => {
                if let Some(m) = nums(&op.operands) {
                    tm = m;
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                let n: usize = op.operands.iter().map(str_len).sum();
                if n == 0 {
                    continue;
                }
                let m = mul(&tm, &ctm);
                let phi = m[1].atan2(m[0]).to_degrees(); // sens du texte, anti-horaire, repère PDF
                let theta = ((rot as f32 - phi) / 90.0).round() as i64; // vue : sens horaire
                w[theta.rem_euclid(4) as usize] += n;
                total += n;
            }
            _ => {}
        }
    }
    if total == 0 {
        return 0;
    }
    let (best, wb) = w.iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, v)| (i, *v)).unwrap_or((0, 0));
    if wb * 10 >= total * 4 {
        (best as u16) * 90
    } else {
        0
    }
}

// ───────────────────────────── export cumulé ─────────────────────────────

#[derive(Debug, Clone)]
pub struct RmPlan {
    pub rm: Rm,
    /// page (0-based) -> (position, folio)
    pub pages: BTreeMap<usize, (Placement, u32)>,
}

#[derive(Debug, Clone, Default)]
pub struct ExportPlan {
    pub rm: Option<RmPlan>,
    pub annexe: Option<Options>,
    pub fill: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub cartouches: usize,
    pub annexes: usize,
}

/// Dessine le cartouche ReeMaker puis le tampon Annexe sur le même document ; un seul PDF en sortie.
pub fn export(input: &[u8], plan: &ExportPlan) -> Result<(Vec<u8>, Counts), String> {
    let mut doc = Document::load_mem(input).map_err(|e| format!("PDF illisible : {e}"))?;
    if doc.is_encrypted() {
        return Err("PDF chiffré : non pris en charge.".into());
    }
    let pages: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
    let n = pages.len() as u32;
    let selected = match &plan.annexe {
        Some(o) => Some(parse_range(&o.pages, n)?),
        None => None,
    };
    let reg = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" });
    let bold = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding" });
    let mut counts = Counts::default();
    for (idx, (num, id)) in pages.iter().enumerate() {
        let mut content: Vec<u8> = Vec::new();
        let (mut uses_reg, mut uses_bold) = (false, false);
        if let Some(rp) = &plan.rm {
            if let Some((pl, folio)) = rp.pages.get(&idx) {
                let g = geo(&doc, *id);
                content.extend(cart_content(&layout(&rp.rm, *folio), pl, &g, plan.fill));
                uses_reg = true;
                counts.cartouches += 1;
            }
        }
        if let (Some(o), Some(sel)) = (&plan.annexe, &selected) {
            if sel.contains(num) {
                let mut o = o.clone();
                o.white_fill = plan.fill;
                let text = render_text(&o, *num, n);
                content.extend(stamp_content(&o, &text, media_box(&doc, *id), rotation(&doc, *id)));
                uses_bold = true;
                counts.annexes += 1;
            }
        }
        if content.is_empty() {
            continue;
        }
        if uses_reg {
            add_font_to_page(&mut doc, *id, REG_FONT, reg)?;
        }
        if uses_bold {
            add_font_to_page(&mut doc, *id, crate::FONT_NAME, bold)?;
        }
        doc.add_page_contents(*id, content).map_err(|e| e.to_string())?;
        isolate_existing(&mut doc, *id);
    }
    if counts == Counts::default() {
        return Err("Rien à exporter : aucun tampon à appliquer.".into());
    }
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok((out, counts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Dictionary, Stream};

    /// PDF d'essai : une page par couple (rotation, contenu).
    fn pdf(pages: &[(Option<i64>, &str)]) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let mut kids = vec![];
        for (rot, content) in pages {
            let c = doc.add_object(Stream::new(Dictionary::new(), content.as_bytes().to_vec()));
            let mut p = dictionary! {
                "Type" => "Page", "Parent" => pages_id, "Contents" => c,
                "MediaBox" => vec![0.into(), 0.into(), 420.into(), 300.into()],
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            };
            if let Some(r) = rot {
                p.set("Rotate", *r);
            }
            kids.push(Object::Reference(doc.add_object(p)));
        }
        let count = kids.len() as i64;
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }));
        let cat = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", cat);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn tranches() {
        assert_eq!(tranche_label(&[0]), "0");
        assert_eq!(tranche_label(&[2, 1]), "1, 2");
        assert_eq!(tranche_label(&[1, 2, 3, 4]), "1-4");
        assert_eq!(tranche_label(&[1, 3]), "1, 3");
        assert_eq!(tranche_label(&[]), "");
    }

    #[test]
    fn textes_du_cartouche() {
        let rm = Rm { site: "SITE EXEMPLE".into(), ree: "JPJ 701".into(), indice: "A".into(), arret: "TEM".into(), cycle: "36".into(), ..Rm::default() };
        let t = texts(&rm, 46);
        assert_eq!((t.l1.as_str(), t.tr.as_str()), ("Site de SITE EXEMPLE", "Tr 0"));
        assert_eq!(texts(&Rm { cart_ver: 1, ..rm.clone() }, 1).l1, "Site de SITE EXEMPLE Tr 0");
        assert_eq!((t.l2a.as_str(), t.l2b.as_str(), t.l3a.as_str(), t.l3b.as_str()), ("REE JPJ 701", "Ind. A", "Folio 46", "TEM"));
        assert_eq!(texts(&Rm { show_cycle: true, ..rm.clone() }, 1).l3b, "TEM 36");
        assert_eq!(texts(&Rm { tranches: vec![], ..rm }, 1).tr, "");
    }

    #[test]
    fn le_zoom_agrandit_proportionnellement() {
        let rm = Rm { site: "S".into(), ..Rm::default() };
        let (a, b) = (layout(&rm, 1), layout(&Rm { zoom: 2.4, ..rm }, 1));
        assert!((b.w / a.w - 2.0).abs() < 1e-4 && (b.h / a.h - 2.0).abs() < 1e-4);
    }

    #[test]
    fn coins_dans_le_cadre_pour_chaque_orientation() {
        let l = layout(&Rm { site: "SITE".into(), ..Rm::default() }, 1);
        for o in [0u16, 90, 180, 270] {
            let bb = bbox(&l, o);
            for (x, y) in [(0.0, 0.0), (l.w, 0.0), (l.w, l.h), (0.0, l.h)] {
                let (vx, vy) = local_to_visual(&l, o, 100.0, 50.0, x, y);
                assert!(vx >= 99.99 && vx <= 100.01 + bb.0 && vy >= 49.99 && vy <= 50.01 + bb.1, "orientation {o}");
            }
        }
    }

    #[test]
    fn noms_de_fichier() {
        let t = "[Folio {folios} de ][REE {ree}][ Tr {tranche}][ Ind {indice}]";
        let rm = Rm { ree: "JPJ 701".into(), indice: "A".into(), ..Rm::default() };
        assert_eq!(export_name(t, &rm, true, "FNC 228", None), "REE JPJ 701 Tr 0 Ind A.pdf");
        assert_eq!(export_name(t, &rm, true, "FNC 228", Some((36, 54))), "Folio 36 à 54 de REE JPJ 701 Tr 0 Ind A.pdf"); // folios du cartouche
        assert_eq!(export_name(t, &rm, true, "FNC 228", Some((36, 36))), "Folio 36 de REE JPJ 701 Tr 0 Ind A.pdf"); // une seule page
        assert_eq!(export_name(t, &rm, false, "FNC 228", Some((36, 54))), "FNC 228_folioté.pdf"); // ReeMaker ignoré
        assert_eq!(export_name(t, &Rm { tranches: vec![1, 2], ..rm.clone() }, true, "FNC 228", Some((1, 6))), "Folio 1 à 6 de REE JPJ 701 Tr 1,2 Ind A.pdf");
        assert_eq!(export_name(t, &Rm { tranches: vec![], ..rm.clone() }, true, "FNC 228", None), "REE JPJ 701 Ind A.pdf");
        assert_eq!(export_name(t, &rm, false, "FNC 228", None), "FNC 228_folioté.pdf"); // ReeMaker ignoré
        assert_eq!(export_name(t, &Rm::default(), true, "FNC 228", None), "FNC 228_folioté.pdf"); // seule la tranche 0 par défaut
        let mut v = BTreeMap::new();
        v.insert("ree", "JPJ/701:x".to_string());
        assert_eq!(build_name("[REE {ree}]", &v), "REE JPJ-701-x");
        v.insert("site", "A".to_string());
        v.insert("nom", "doc".to_string());
        assert_eq!(build_name("{site} - {nom}", &v), "A - doc");
    }

    #[test]
    fn orientation_automatique() {
        let up = "BT /F1 12 Tf 30 150 Td (Texte de la page pour la detection) Tj ET";
        let ccw = "BT /F1 12 Tf 0 1 -1 0 200 20 Tm (Texte de la page pour la detection) Tj ET"; // couché, bas -> haut
        let bytes = pdf(&[(None, up), (None, ccw), (Some(90), up), (None, "0 0 10 10 re f")]);
        let doc = Document::load_mem(&bytes).unwrap();
        let ids: Vec<_> = doc.get_pages().values().cloned().collect();
        let o: Vec<u16> = ids.iter().map(|id| detect_orientation(&doc, *id)).collect();
        assert_eq!(o, vec![0, 270, 90, 0]);
    }

    #[test]
    fn export_cumule_toutes_rotations() {
        let up = "BT /F1 12 Tf 30 150 Td (Hello) Tj ET";
        let bytes = pdf(&[(None, up), (Some(90), up), (Some(180), up), (Some(270), up)]);
        let rm = Rm { site: "SITE".into(), ree: "001".into(), indice: "B".into(), arret: "TEM".into(), ..Rm::default() };
        let mut pages = BTreeMap::new();
        for i in 0..4 {
            pages.insert(i, (Placement { fx: 0.55, fy: 0.05, orient: if i % 2 == 0 { 0 } else { 90 } }, 46 + i as u32));
        }
        let plan = ExportPlan { rm: Some(RmPlan { rm, pages }), annexe: Some(Options { fx: 0.04, fy: 0.9, ..Options::default() }), fill: true };
        let (out, c) = export(&bytes, &plan).unwrap();
        assert_eq!(c, Counts { cartouches: 4, annexes: 4 });
        let doc = Document::load_mem(&out).unwrap();
        let (_, id) = doc.get_pages().into_iter().next().unwrap();
        let text = String::from_utf8_lossy(&doc.get_page_content(id)).to_string();
        assert!(text.contains("(Site de SITE) Tj") && text.contains("(Tr 0) Tj") && text.contains("(Folio 46) Tj") && text.contains("(Annexe 1 Folio 1/4) Tj"), "{text}");
        if let Ok(p) = std::env::var("OUT_PDF") {
            std::fs::write(p, &out).unwrap();
        }
        assert!(export(&bytes, &ExportPlan::default()).is_err());
    }
}
