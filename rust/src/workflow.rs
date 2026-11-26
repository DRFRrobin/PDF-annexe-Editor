//! Chaîne d'étapes : Fichier → ReeMaker → Annexio → Export. Foliotage page par page, folios continus,
//! nom automatique et export cumulé (même logique que la version web).

use super::*;
use annexio::reemaker::{self, ExportPlan, Placement, RmPlan};
use std::collections::BTreeSet;

pub const STEP_NAMES: [&str; 4] = ["Fichier", "ReeMaker", "Annexio", "Export"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    File,
    Rm,
    Annexe,
    Export,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::File, Step::Rm, Step::Annexe, Step::Export];
    pub fn idx(self) -> usize {
        self as usize
    }
}

/// Copie de l'état du foliotage, pour « Annuler le foliotage ».
#[derive(Clone)]
pub struct Snapshot {
    place: BTreeMap<usize, Placement>,
    skipped: BTreeSet<usize>,
    done: bool,
    last: Option<Placement>,
}

pub struct Run {
    pub list: Vec<usize>,
    pub pos: usize,
    pub pending: Placement,
    snap: Snapshot,
}

pub const DEFAULT_PL: (f32, f32) = (0.6, 0.03);

impl App {
    // ── paramètres ReeMaker ──

    /// Recopie les champs de saisie dans les paramètres.
    pub fn rm_sync(&mut self) {
        self.rm.start = self.rm_start_s.trim().parse().unwrap_or(1).max(1);
        self.rm.tranches = (0..10u8).filter(|t| self.rm_tr[*t as usize]).collect();
        self.rm.base_size = self.base_size;
        self.rm.zoom = self.rm.zoom.clamp(0.5, 3.0);
    }

    /// Pages (0-based) concernées par le cartouche.
    pub fn rm_pages(&self) -> Vec<usize> {
        if self.n_pages == 0 {
            return vec![];
        }
        if self.rm_all {
            return (0..self.n_pages).collect();
        }
        let (mut a, mut b) = (self.rm_from.clamp(1, self.n_pages), self.rm_to.clamp(1, self.n_pages));
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        (a - 1..b).collect()
    }

    /// Folio de la page `i` : départ + nombre de pages concernées avant elle, sans compter celles passées.
    pub fn folio_of(&self, i: usize) -> u32 {
        let before = self.rm_pages().into_iter().take_while(|&j| j != i).filter(|j| !self.skipped.contains(j)).count() as u32;
        self.rm.start + before
    }

    pub fn folio_range(&self) -> String {
        match (self.place.keys().next(), self.place.keys().next_back()) {
            (Some(&a), Some(&b)) => format!("{}–{}", self.folio_of(a), self.folio_of(b)),
            _ => "—".into(),
        }
    }

    pub fn an_active(&self) -> bool {
        self.an_apply && self.seen[Step::Annexe.idx()]
    }

    pub fn an_selected(&self) -> BTreeSet<u32> {
        parse_range(&self.opts.pages, self.n_pages as u32).unwrap_or_default()
    }

    pub fn can_export(&self) -> bool {
        self.bytes.is_some() && ((self.rm_apply && !self.place.is_empty()) || (self.an_active() && !self.an_selected().is_empty()))
    }

    // ── orientation ──

    pub fn orient_for(&mut self, i: usize) -> u16 {
        if let Some(o) = self.rm_orient {
            return o;
        }
        if let Some(&o) = self.orient_cache.get(&i) {
            return o;
        }
        let o = match (&self.rm_doc, self.rm_doc.as_ref().and_then(|d| d.get_pages().get(&(i as u32 + 1)).cloned())) {
            (Some(doc), Some(id)) => reemaker::detect_orientation(doc, id),
            _ => 0,
        };
        self.orient_cache.insert(i, o);
        o
    }

    fn pending_for(&mut self, i: usize) -> Placement {
        let orient = self.orient_for(i);
        if let Some(p) = self.place.get(&i) {
            return *p;
        }
        match self.last_pl {
            Some(l) if self.keep_pos => Placement { fx: l.fx, fy: l.fy, orient },
            _ => Placement { fx: DEFAULT_PL.0, fy: DEFAULT_PL.1, orient },
        }
    }

    // ── foliotage page par page ──

    pub fn start_run(&mut self) {
        if self.bytes.is_none() {
            return self.say("Ouvrez d'abord un PDF.", true);
        }
        self.rm_sync();
        let list = self.rm_pages();
        if list.is_empty() {
            return self.say("Aucune page à folioter.", true);
        }
        let snap = Snapshot { place: self.place.clone(), skipped: self.skipped.clone(), done: self.rm_done, last: self.last_pl };
        self.rm_done = false;
        let first = list[0];
        let pending = self.pending_for(first);
        self.run = Some(Run { list, pos: 0, pending, snap });
        self.goto(first);
    }

    fn set_run_pos(&mut self, pos: usize) {
        let Some(i) = self.run.as_ref().map(|r| r.list[pos]) else { return };
        let pending = self.pending_for(i);
        if let Some(r) = self.run.as_mut() {
            r.pos = pos;
            r.pending = pending;
        }
        self.goto(i);
    }

    pub fn validate_run(&mut self) {
        let Some(r) = &self.run else { return };
        let (i, pl) = (r.list[r.pos], r.pending);
        self.place.insert(i, pl);
        self.skipped.remove(&i);
        self.last_pl = Some(pl);
        self.advance_run();
    }

    pub fn skip_run(&mut self) {
        let Some(r) = &self.run else { return };
        let i = r.list[r.pos];
        self.skipped.insert(i);
        self.place.remove(&i);
        self.advance_run();
    }

    fn advance_run(&mut self) {
        let Some(r) = &self.run else { return };
        if r.pos + 1 >= r.list.len() {
            self.run = None;
            self.rm_done = true;
            let msg = format!("Foliotage terminé : {} page(s) · folios {}.", self.place.len(), self.folio_range());
            self.say(msg, false);
        } else {
            let p = r.pos + 1;
            self.set_run_pos(p);
        }
    }

    pub fn prev_run(&mut self) {
        if let Some(p) = self.run.as_ref().map(|r| r.pos).filter(|p| *p > 0) {
            self.set_run_pos(p - 1);
        }
    }

    pub fn cancel_run(&mut self) {
        let Some(r) = self.run.take() else { return };
        self.place = r.snap.place;
        self.skipped = r.snap.skipped;
        self.rm_done = r.snap.done;
        self.last_pl = r.snap.last;
        self.say("Foliotage annulé.", false);
    }

    /// Page cliquée (miniature, flèches de la bande fichier) : pendant le foliotage, on saute à cette page de la liste.
    pub fn goto_page(&mut self, idx: usize) {
        if self.n_pages == 0 {
            return;
        }
        let idx = idx.min(self.n_pages - 1);
        if let Some(r) = &self.run {
            match r.list.iter().position(|&p| p == idx) {
                Some(pos) => self.set_run_pos(pos),
                None => self.say("Cette page n'est pas dans la plage à folioter.", true),
            }
        } else if idx != self.cur {
            self.goto(idx);
        }
    }

    // ── étapes ──

    pub fn set_step(&mut self, s: Step) {
        if s != Step::File && self.bytes.is_none() {
            return self.say("Ouvrez d'abord un PDF.", true);
        }
        if self.run.is_some() && s != Step::Rm {
            self.cancel_run(); // quitter l'étape pendant le foliotage l'annule
        }
        for j in 0..s.idx() {
            self.visited[j] = true;
        }
        self.step = s;
        self.visited[s.idx()] = true;
        self.seen[s.idx()] = true;
    }

    /// (état, résumé) d'une étape pour la barre à chevrons : état = "done" | "skip" | "".
    pub fn chev_state(&self, i: usize) -> (&'static str, String) {
        match i {
            0 => {
                if self.bytes.is_some() {
                    ("done", format!("{} · {} p.", self.name, self.n_pages))
                } else {
                    ("", String::new())
                }
            }
            1 => {
                if !self.rm_apply {
                    return if self.visited[1] { ("skip", "ignoré".into()) } else { ("", String::new()) };
                }
                if let Some(r) = &self.run {
                    return ("", format!("page {}/{}", r.pos + 1, r.list.len()));
                }
                if self.rm_done { ("done", format!("folios {}", self.folio_range())) } else { ("", String::new()) }
            }
            2 => {
                if !self.an_apply {
                    return if self.visited[2] && self.step != Step::Annexe { ("skip", "ignoré".into()) } else { ("", String::new()) };
                }
                if self.visited[2] && self.step != Step::Annexe { ("done", format!("Annexe {}", self.opts.annexe)) } else { ("", String::new()) }
            }
            _ => {
                if self.exported { ("done", "exporté".into()) } else { ("", String::new()) }
            }
        }
    }

    // ── nom et export ──

    pub fn auto_name(&self) -> String {
        let base = self.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let folios = match (self.place.keys().next(), self.place.keys().next_back()) {
            (Some(&a), Some(&b)) => Some((self.folio_of(a), self.folio_of(b))),
            _ => None,
        };
        reemaker::export_name(&self.name_tpl, &self.rm, self.rm_apply, if base.is_empty() { "document" } else { base }, folios)
    }

    pub fn export(&mut self) {
        if !self.can_export() {
            return self.say("Rien à exporter : appliquez au moins un tampon.", true);
        }
        self.rm_sync();
        self.sync_opts();
        let Some(bytes) = &self.bytes else { return };
        let rm = if self.rm_apply && !self.place.is_empty() {
            Some(RmPlan { rm: self.rm.clone(), pages: self.place.iter().map(|(&i, &pl)| (i, (pl, self.folio_of(i)))).collect() })
        } else {
            None
        };
        let plan = ExportPlan { rm, annexe: if self.an_active() { Some(self.opts.clone()) } else { None }, fill: self.opts.white_fill };
        if !self.out_manual {
            self.out_name = self.auto_name();
        }
        // version de développement uniquement : fichier de sortie imposé (tests sans boîte de dialogue)
        #[cfg(debug_assertions)]
        let forced = std::env::var_os("ANNEXIO_TEST_OUT").map(PathBuf::from);
        #[cfg(not(debug_assertions))]
        let forced: Option<PathBuf> = None;
        let out = match forced {
            Some(p) => p,
            None => {
                let mut dlg = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(self.out_name.clone());
                if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
                    dlg = dlg.set_directory(dir);
                }
                let Some(out) = dlg.save_file() else { return };
                out
            }
        };
        if self.path.as_deref() == Some(out.as_path()) {
            return self.say("Choisissez un autre nom : le fichier d'origine ne doit pas être remplacé.", true);
        }
        match reemaker::export(bytes, &plan) {
            Err(e) => self.say(e, true),
            Ok((data, c)) => match std::fs::write(&out, data) {
                Ok(()) => {
                    self.exported = true;
                    self.status = Status {
                        text: format!("✔ {} cartouche(s) et {} tampon(s) Annexio : {}", c.cartouches, c.annexes, out.display()),
                        err: false,
                        out: Some(out),
                    };
                }
                Err(e) => self.say(
                    format!("Impossible d'écrire « {} » ({e}). Le fichier est peut-être ouvert dans un autre programme.", out.display()),
                    true,
                ),
            },
        }
    }
}
