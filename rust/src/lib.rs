//! Tampon rouge « Annexe X Folio n/N » sur les pages d'un PDF.
//! Tout est local : aucune donnée n'est envoyée nulle part.

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use std::collections::BTreeSet;

#[cfg(feature = "wasm")]
pub mod wasm;

const PAD: f32 = 4.0;
const BORDER: f32 = 1.5;
const FONT_NAME: &str = "FAnx";

#[derive(Debug, Clone)]
pub struct Options {
    pub annexe: String,
    /// Modèle : `{annexe}`, `{n}`, `{total}`.
    pub template: String,
    pub start: u32,
    /// `None` = nombre de pages du document.
    pub total: Option<u32>,
    pub font_size: f32,
    /// Ex. « 1-3,5 » ; vide = toutes les pages.
    pub pages: String,
    pub white_fill: bool,
    /// Coin haut-gauche du rectangle, en fraction de la page affichée (0..1).
    pub fx: f32,
    pub fy: f32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            annexe: "1".into(),
            template: "Annexe {annexe} Folio {n}/{total}".into(),
            start: 1,
            total: None,
            font_size: 14.0,
            pages: String::new(),
            white_fill: true,
            fx: 0.55,
            fy: 0.03,
        }
    }
}

pub fn render_text(o: &Options, page: u32, num_pages: u32) -> String {
    let total = o.total.filter(|t| *t > 0).unwrap_or(num_pages);
    o.template
        .replace("{annexe}", &o.annexe)
        .replace("{n}", &(o.start + page - 1).to_string())
        .replace("{total}", &total.to_string())
}

pub fn parse_range(s: &str, num_pages: u32) -> Result<BTreeSet<u32>, String> {
    if s.trim().is_empty() {
        return Ok((1..=num_pages).collect());
    }
    let mut out = BTreeSet::new();
    for part in s.split(',') {
        let p = part.trim();
        let bad = || format!("Plage invalide : « {p} »");
        let (a, b) = match p.split_once('-') {
            Some((a, b)) => (a.trim().parse::<u32>().map_err(|_| bad())?, b.trim().parse::<u32>().map_err(|_| bad())?),
            None => {
                let a = p.parse::<u32>().map_err(|_| bad())?;
                (a, a)
            }
        };
        for i in a.min(b)..=a.max(b) {
            if (1..=num_pages).contains(&i) {
                out.insert(i);
            }
        }
    }
    Ok(out)
}

/// Largeur (1/1000 em) en Helvetica-Bold ; accents approchés par la lettre de base.
fn glyph_width(c: char) -> u32 {
    const W: [u16; 95] = [
        278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556,
        556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722,
        611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556,
        611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778,
        556, 556, 500, 389, 280, 389, 584,
    ];
    match c {
        ' '..='~' => W[c as usize - 32] as u32,
        'é' | 'è' | 'ê' | 'ë' | 'à' | 'â' | 'ä' | 'ç' | '–' => 556,
        'î' | 'ï' => 278,
        'É' | 'È' | 'Ê' => 667,
        'À' | 'Â' | 'Ç' => 722,
        '—' => 1000,
        _ => 611,
    }
}

pub fn text_width(text: &str, size: f32) -> f32 {
    text.chars().map(glyph_width).sum::<u32>() as f32 * size / 1000.0
}

/// Taille (largeur, hauteur) du rectangle en points pour un texte donné.
pub fn box_size(text: &str, font_size: f32) -> (f32, f32) {
    (text_width(text, font_size) + 2.0 * PAD + 2.0 * BORDER, font_size * 1.2 + 2.0 * PAD + 2.0 * BORDER)
}

/// Encodage WinAnsi (cp1252) pour la police standard ; `?` si impossible.
fn win_ansi(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match c {
            '–' => 0x96,
            '—' => 0x97,
            '’' => 0x92,
            '€' => 0x80,
            c if (c as u32) < 256 => c as u32 as u8,
            _ => b'?',
        })
        .collect()
}

fn pdf_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![b'('];
    for &b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

/// Attribut hérité dans l'arbre des pages, références résolues.
fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page;
    for _ in 0..64 {
        let d = doc.get_dictionary(id).ok()?;
        if let Ok(o) = d.get(key) {
            return Some(match o {
                Object::Reference(r) => doc.get_object(*r).ok()?.clone(),
                o => o.clone(),
            });
        }
        id = d.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

fn num(o: &Object) -> Option<f32> {
    o.as_float().ok().or_else(|| o.as_i64().ok().map(|i| i as f32))
}

fn media_box(doc: &Document, page: ObjectId) -> [f32; 4] {
    if let Some(Object::Array(a)) = inherited(doc, page, b"MediaBox") {
        let v: Vec<f32> = a
            .iter()
            .map(|o| match o {
                Object::Reference(r) => doc.get_object(*r).ok().and_then(num).unwrap_or(0.0),
                o => num(o).unwrap_or(0.0),
            })
            .collect();
        if v.len() == 4 {
            return [v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])];
        }
    }
    [0.0, 0.0, 612.0, 792.0]
}

fn rotation(doc: &Document, page: ObjectId) -> i64 {
    inherited(doc, page, b"Rotate").and_then(|o| o.as_i64().ok()).unwrap_or(0).rem_euclid(360) / 90 * 90
}

/// Contenu du tampon, dans le repère non tourné de la page.
fn stamp_content(o: &Options, text: &str, bx: [f32; 4], rot: i64) -> Vec<u8> {
    let s = o.font_size;
    let (w, h) = (bx[2] - bx[0], bx[3] - bx[1]);
    let (vw, vh) = if rot % 180 != 0 { (h, w) } else { (w, h) };
    let (bw, bh) = box_size(text, s);
    let vx = (o.fx * vw).clamp(0.0, (vw - bw).max(0.0));
    let vy = (o.fy * vh).clamp(0.0, (vh - bh).max(0.0));
    // coin bas-gauche visuel (depuis le haut) -> repère PDF non tourné
    let (px, py) = (vx, vy + bh);
    let (ox, oy) = match rot {
        0 => (px, h - py),
        90 => (py, px),
        180 => (w - px, py),
        _ => (w - py, h - px),
    };
    let (ox, oy) = (ox + bx[0], oy + bx[1]);
    let (c, sn) = match rot {
        0 => (1.0, 0.0),
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        _ => (0.0, -1.0),
    };
    let mut out = format!("q\n{c} {sn} {} {c} {ox:.3} {oy:.3} cm\n", -sn).into_bytes();
    let fill = if o.white_fill { "1 1 1 rg\n" } else { "" };
    let mode = if o.white_fill { "B" } else { "S" };
    out.extend(
        format!(
            "{fill}0.87 0 0 RG {BORDER} w\n{:.3} {:.3} {:.3} {:.3} re {mode}\n",
            BORDER / 2.0,
            BORDER / 2.0,
            bw - BORDER,
            bh - BORDER
        )
        .into_bytes(),
    );
    out.extend(format!("BT 0.87 0 0 rg /{FONT_NAME} {s} Tf {:.3} {:.3} Td ", BORDER + PAD, BORDER + PAD + s * 0.22).into_bytes());
    out.extend(pdf_string(&win_ansi(text)));
    out.extend(b" Tj ET\nQ\n");
    out
}

fn add_font_to_page(doc: &mut Document, page: ObjectId, font: ObjectId) -> Result<(), String> {
    let mut res = match inherited(doc, page, b"Resources") {
        Some(Object::Dictionary(d)) => d,
        _ => Dictionary::new(),
    };
    let mut fonts = match res.get(b"Font") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(r)) => doc.get_dictionary(*r).map(|d| d.clone()).unwrap_or_default(),
        _ => Dictionary::new(),
    };
    fonts.set(FONT_NAME, Object::Reference(font));
    res.set("Font", Object::Dictionary(fonts));
    let p = doc.get_object_mut(page).and_then(Object::as_dict_mut).map_err(|e| e.to_string())?;
    p.set("Resources", Object::Dictionary(res));
    Ok(())
}

/// Ajoute le tampon et renvoie le nouveau PDF. Retourne aussi le nombre de pages tamponnées.
pub fn stamp(input: &[u8], o: &Options) -> Result<(Vec<u8>, usize), String> {
    let mut doc = Document::load_mem(input).map_err(|e| format!("PDF illisible : {e}"))?;
    if doc.is_encrypted() {
        return Err("PDF chiffré : non pris en charge.".into());
    }
    let pages = doc.get_pages();
    let n = pages.len() as u32;
    let selected = parse_range(&o.pages, n)?;
    if selected.is_empty() {
        return Err("Aucune page sélectionnée.".into());
    }
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
    });
    for (&num, &id) in &pages {
        if !selected.contains(&num) {
            continue;
        }
        let text = render_text(o, num, n);
        let content = stamp_content(o, &text, media_box(&doc, id), rotation(&doc, id));
        add_font_to_page(&mut doc, id, font)?;
        // q ... Q autour de l'existant pour isoler son état graphique, puis le tampon
        doc.add_page_contents(id, content).map_err(|e| e.to_string())?;
        isolate_existing(&mut doc, id);
    }
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok((out, selected.len()))
}

fn isolate_existing(doc: &mut Document, page: ObjectId) {
    let Ok(Object::Array(list)) = doc.get_dictionary(page).and_then(|d| d.get(b"Contents")).cloned() else { return };
    if list.len() < 2 {
        return;
    }
    let q = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let big_q = doc.add_object(Stream::new(Dictionary::new(), b"\nQ\n".to_vec()));
    let mut v = vec![Object::Reference(q)];
    v.extend(list[..list.len() - 1].iter().cloned());
    v.push(Object::Reference(big_q));
    v.push(list[list.len() - 1].clone());
    if let Ok(d) = doc.get_object_mut(page).and_then(Object::as_dict_mut) {
        d.set("Contents", v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(rotate: Option<i64>) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let content = doc.add_object(Stream::new(Dictionary::new(), b"0 0 10 10 re f".to_vec()));
        let mut page = dictionary! {
            "Type" => "Page", "Parent" => pages, "Contents" => content,
            "MediaBox" => vec![0.into(), 0.into(), 400.into(), 300.into()],
        };
        if let Some(r) = rotate {
            page.set("Rotate", r);
        }
        let page = doc.add_object(page);
        doc.objects.insert(
            pages,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn texte_par_defaut() {
        assert_eq!(render_text(&Options::default(), 2, 5), "Annexe 1 Folio 2/5");
    }

    #[test]
    fn plages() {
        assert_eq!(parse_range("1-3,5", 10).unwrap().into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 5]);
        assert_eq!(parse_range("", 3).unwrap().len(), 3);
        assert!(parse_range("a", 3).is_err());
    }

    #[test]
    fn tampon_sur_pdf_normal_et_tourne() {
        for r in [None, Some(90), Some(180), Some(270)] {
            let (out, n) = stamp(&sample(r), &Options::default()).unwrap();
            assert_eq!(n, 1);
            let doc = Document::load_mem(&out).unwrap();
            let (&_, &id) = doc.get_pages().iter().next().unwrap();
            let content = String::from_utf8_lossy(&doc.get_page_content(id)).to_string();
            assert!(content.contains("(Annexe 1 Folio 1/1) Tj"), "{content}");
            assert!(content.contains("0.87 0 0 RG"));
        }
    }
}
