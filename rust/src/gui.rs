//! Interface graphique : aperçu du PDF, rectangle rouge déplaçable, génération. 100 % local.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod preview;
mod settings;
mod steps;
mod update;
mod workflow;

use eframe::egui::{self, Align, Color32, FontId, Key, Layout, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use annexio::reemaker::{Placement, Rm};
use annexio::{box_size, parse_range, render_text, Options};
use settings::Settings;
use workflow::{Run, Step};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

const RED: Color32 = Color32::from_rgb(222, 0, 0);
const GREEN: Color32 = Color32::from_rgb(0, 130, 50);
const SIDE_MIN: f32 = 220.0;
const SIDE_MAX: f32 = 640.0;
const THUMB_W: f32 = 100.0;
const MAX_THUMBS: usize = 400;
/// Défilement cumulé (en points) pour changer de page : environ une encoche de molette.
const WHEEL_STEP: f32 = 40.0;
/// Pixels écran par point PDF à 100 % (96 dpi).
const BASE_K: f32 = 96.0 / 72.0;
const TEMPLATES: [&str; 5] = [
    "Annexe {annexe} Folio {n}/{total}",
    "Annexe {annexe} – Folio {n}/{total}",
    "Annexe {annexe} – Page {n}/{total}",
    "Annexe {annexe}",
    "{n}/{total}",
];

// ───────────────────────────── rendu PDF ─────────────────────────────

struct Page {
    image: egui::ColorImage,
    /// Taille visuelle de la page en points.
    pts: (f32, f32),
    /// Échelle (pixels par point) à laquelle l'image a été rendue.
    scale: f32,
}

fn friendly_open_error(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("encrypt") || l.contains("decrypt") || l.contains("password") {
        "Ce PDF est protégé par un mot de passe : il ne peut pas être ouvert.".into()
    } else {
        format!("Ce fichier n'est pas un PDF lisible ou il est endommagé ({e}).")
    }
}

fn render_to_image(page: &hayro::hayro_syntax::page::Page, scale: f32) -> egui::ColorImage {
    let px = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings { x_scale: scale, y_scale: scale, bg_color: WHITE },
    );
    egui::ColorImage::from_rgba_premultiplied([px.width() as usize, px.height() as usize], px.data_as_u8_slice())
}

fn load_page(bytes: &[u8], idx: usize, scale: f32) -> Result<(Page, usize), String> {
    let pdf = Pdf::new(bytes.to_vec()).map_err(|e| friendly_open_error(&format!("{e:?}")))?;
    let pages = pdf.pages();
    let n = pages.len();
    if n == 0 {
        return Err("Ce PDF ne contient aucune page.".into());
    }
    let page = pages.get(idx).ok_or("Page introuvable")?;
    let (w, h) = page.render_dimensions();
    let scale = scale.min(4096.0 / w.max(h).max(1.0));
    Ok((Page { image: render_to_image(page, scale), pts: (w, h), scale }, n))
}

/// Rend les miniatures en arrière-plan, une par une.
fn spawn_thumbs(bytes: Vec<u8>, ctx: egui::Context) -> Receiver<(usize, egui::ColorImage)> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let Ok(pdf) = Pdf::new(bytes) else { return };
        let pages = pdf.pages();
        for i in 0..pages.len().min(MAX_THUMBS) {
            let Some(page) = pages.get(i) else { break };
            let (w, _) = page.render_dimensions();
            let img = render_to_image(page, THUMB_W * 2.0 / w.max(1.0));
            if tx.send((i, img)).is_err() {
                break; // PDF fermé ou remplacé
            }
            ctx.request_repaint();
        }
    });
    rx
}

fn open_in_os(path: &Path, select: bool) {
    use std::process::Command;
    #[cfg(windows)]
    {
        let mut c = Command::new("explorer");
        if select {
            c.arg(format!("/select,{}", path.display()));
        } else {
            c.arg(path);
        }
        let _ = c.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("open");
        if select {
            c.arg("-R");
        }
        let _ = c.arg(path).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = if select { path.parent().unwrap_or(path) } else { path };
        let _ = Command::new("xdg-open").arg(target).spawn();
    }
}


// ───────────────────────────── état ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Zoom {
    FitPage,
    FitWidth,
    Custom(f32),
}

/// Photo des réglages, pour annuler / rétablir.
#[derive(Clone, PartialEq)]
struct Snap {
    opts: Options,
    start: String,
    total: String,
    size: String,
}

/// État de la mise à jour.
enum Upd {
    Idle,
    Checking,
    UpToDate,
    Available(update::Release),
    Installing,
    Done,
}

enum UpdMsg {
    Checked(Result<Option<update::Release>, String>),
    Installed(Result<(), String>),
}

struct Status {
    text: String,
    err: bool,
    out: Option<PathBuf>,
}

struct App {
    ctx: egui::Context,
    step: Step,
    seen: [bool; 4],
    visited: [bool; 4],
    exported: bool,
    settings_open: bool,
    // document
    bytes: Option<Vec<u8>>,
    rm_doc: Option<lopdf::Document>,
    name: String,
    path: Option<PathBuf>,
    n_pages: usize,
    cur: usize, // 0-based
    page: Option<Page>,
    tex: Option<egui::TextureHandle>,
    thumbs: Vec<Option<(egui::TextureHandle, f32)>>, // (texture, largeur/hauteur)
    thumb_rx: Option<Receiver<(usize, egui::ColorImage)>>,
    scroll_thumb: bool,
    // étape ReeMaker
    rm: Rm,
    rm_apply: bool,
    rm_start_s: String,
    rm_all: bool,
    rm_from: usize,
    rm_to: usize,
    rm_tr: [bool; 10],
    rm_orient: Option<u16>,
    place: BTreeMap<usize, Placement>,
    skipped: std::collections::BTreeSet<usize>,
    rm_done: bool,
    last_pl: Option<Placement>,
    run: Option<Run>,
    orient_cache: HashMap<usize, u16>,
    keep_pos: bool,
    base_size: f32,
    // étape Annexio (tampon sur une ligne)
    an_apply: bool,
    opts: Options,
    start_s: String,
    total_s: String,
    size_s: String,
    margin: f32,
    // export
    name_tpl: String,
    out_name: String,
    out_manual: bool,
    // affichage
    zoom: Zoom,
    k: f32,
    zoom_at: f64,
    wheel: f32,
    scrollable: bool,
    preview_rect: Rect,
    side_w: f32,
    show_thumbs: bool,
    theme: u8,
    applied_theme: Option<u8>,
    ui_scale: f32,
    // historique (étape Annexio)
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last_snap: Snap,
    last_change: f64,
    // divers
    recents: Vec<String>,
    auto_update: bool,
    upd: Upd,
    upd_rx: Option<Receiver<UpdMsg>>,
    upd_dismissed: bool,
    upd_error: Option<String>,
    status: Status,
    saved_text: String,
    last_save_check: f64,
}

impl App {
    fn new(ctx: egui::Context, s: Settings) -> Self {
        let mut opts = Options::default();
        opts.annexe = s.annexe.clone();
        opts.template = s.template.clone();
        opts.white_fill = s.white_fill;
        opts.fx = s.fx;
        opts.fy = s.fy;
        let mut rm = Rm::default();
        rm.site = s.rm_site.clone();
        rm.ree = s.rm_ree.clone();
        rm.indice = s.rm_indice.clone();
        rm.arret = s.rm_arret.clone();
        rm.cycle = s.rm_cycle.clone();
        rm.show_cycle = s.rm_show_cycle;
        rm.zoom = s.rm_zoom;
        rm.base_size = s.base_size;
        rm.cart_ver = s.cart_ver;
        let mut rm_tr = [false; 10];
        for t in s.rm_tranches.split(',').filter_map(|x| x.trim().parse::<usize>().ok()).filter(|t| *t < 10) {
            rm_tr[t] = true;
        }
        let mut app = App {
            ctx,
            step: Step::File,
            seen: [true, false, false, false],
            visited: [true, false, false, false],
            exported: false,
            settings_open: false,
            bytes: None,
            rm_doc: None,
            name: String::new(),
            path: None,
            n_pages: 0,
            cur: 0,
            page: None,
            tex: None,
            thumbs: Vec::new(),
            thumb_rx: None,
            scroll_thumb: false,
            rm,
            rm_apply: true,
            rm_start_s: s.rm_start.clone(),
            rm_all: true,
            rm_from: 1,
            rm_to: 1,
            rm_tr,
            rm_orient: s.rm_orient.parse::<u16>().ok().filter(|o| [0, 90, 180, 270].contains(o)),
            place: BTreeMap::new(),
            skipped: Default::default(),
            rm_done: false,
            last_pl: None,
            run: None,
            orient_cache: HashMap::new(),
            keep_pos: s.keep_pos,
            base_size: s.base_size,
            an_apply: true,
            opts,
            start_s: s.start.clone(),
            total_s: s.total.clone(),
            size_s: s.size.clone(),
            margin: s.margin,
            name_tpl: s.name_tpl.clone(),
            out_name: String::new(),
            out_manual: false,
            zoom: Zoom::FitPage,
            k: BASE_K,
            zoom_at: -10.0,
            wheel: 0.0,
            scrollable: false,
            preview_rect: Rect::NOTHING,
            side_w: s.side_w,
            show_thumbs: s.thumbs,
            theme: s.theme,
            applied_theme: None,
            ui_scale: s.ui_scale,
            undo: Vec::new(),
            redo: Vec::new(),
            last_snap: Snap { opts: Options::default(), start: String::new(), total: String::new(), size: String::new() },
            last_change: -10.0,
            recents: s.recents.clone(),
            auto_update: s.auto_update,
            upd: Upd::Idle,
            upd_rx: None,
            upd_dismissed: false,
            upd_error: None,
            status: Status { text: String::new(), err: false, out: None },
            saved_text: s.to_text(),
            last_save_check: 0.0,
        };
        app.sync_opts();
        app.rm_sync();
        app.last_snap = app.snap();
        app.say("Ouvrez un PDF (Ctrl+O, bouton ou glisser-déposer).", false);
        if app.auto_update {
            app.start_check(); // en arrière-plan : n'empêche pas de travailler
        }
        app
    }

    fn say(&mut self, msg: impl Into<String>, err: bool) {
        self.status = Status { text: msg.into(), err, out: None };
    }

    fn settings(&self) -> Settings {
        Settings {
            annexe: self.opts.annexe.clone(),
            template: self.opts.template.clone(),
            start: self.start_s.clone(),
            total: self.total_s.clone(),
            size: self.size_s.clone(),
            white_fill: self.opts.white_fill,
            fx: self.opts.fx,
            fy: self.opts.fy,
            margin: self.margin,
            side_w: self.side_w,
            theme: self.theme,
            ui_scale: self.ui_scale,
            thumbs: self.show_thumbs,
            auto_update: self.auto_update,
            rm_site: self.rm.site.clone(),
            rm_ree: self.rm.ree.clone(),
            rm_indice: self.rm.indice.clone(),
            rm_start: self.rm_start_s.clone(),
            rm_arret: self.rm.arret.clone(),
            rm_cycle: self.rm.cycle.clone(),
            rm_show_cycle: self.rm.show_cycle,
            rm_tranches: (0..10).filter(|t| self.rm_tr[*t]).map(|t| t.to_string()).collect::<Vec<_>>().join(","),
            rm_zoom: self.rm.zoom,
            rm_orient: self.rm_orient.map_or("auto".to_string(), |o| o.to_string()),
            base_size: self.base_size,
            cart_ver: self.rm.cart_ver,
            keep_pos: self.keep_pos,
            name_tpl: self.name_tpl.clone(),
            recents: self.recents.clone(),
        }
    }

    fn save_settings_if_changed(&mut self) {
        let text = self.settings().to_text();
        if text != self.saved_text {
            settings::save(&self.settings());
            self.saved_text = text;
        }
    }

    // ── mise à jour ──

    // ── mise à jour ──

    fn start_check(&mut self) {
        let (tx, rx) = channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UpdMsg::Checked(update::check()));
            ctx.request_repaint();
        });
        self.upd_rx = Some(rx);
        self.upd = Upd::Checking;
        self.upd_dismissed = false;
        self.upd_error = None;
    }

    fn start_install(&mut self, rel: update::Release) {
        let (tx, rx) = channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UpdMsg::Installed(update::install(&rel).map(|_| ())));
            ctx.request_repaint();
        });
        self.upd_rx = Some(rx);
        self.upd = Upd::Installing;
        self.upd_error = None;
    }

    fn poll_update(&mut self) {
        let Some(rx) = &self.upd_rx else { return };
        let Ok(msg) = rx.try_recv() else { return };
        self.upd_rx = None;
        match msg {
            UpdMsg::Checked(Ok(Some(r))) => self.upd = Upd::Available(r),
            UpdMsg::Checked(Ok(None)) => self.upd = Upd::UpToDate,
            UpdMsg::Checked(Err(e)) => {
                self.upd = Upd::Idle;
                self.upd_error = Some(e);
            }
            UpdMsg::Installed(Ok(())) => self.upd = Upd::Done,
            UpdMsg::Installed(Err(e)) => {
                self.upd = Upd::Idle;
                self.upd_error = Some(e);
            }
        }
    }

    fn restart(&mut self, ctx: &egui::Context) {
        self.save_settings_if_changed();
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Bandeau en haut de la fenêtre : nouvelle version, installation, redémarrage.
    fn update_banner(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut action: Option<&str> = None;
        ui.add_space(4.0);
        ui.horizontal(|ui| match &self.upd {
            Upd::Available(rel) => {
                ui.colored_label(RED, format!("⬆ Annexio {} est disponible (vous avez {}).", rel.version, update::current_version()));
                if rel.asset.is_some() {
                    if ui.button("Mettre à jour").clicked() {
                        action = Some("install");
                    }
                } else {
                    ui.label("Pas de fichier pour ce système dans cette version.");
                }
                if ui.button("Plus tard").clicked() {
                    action = Some("later");
                }
            }
            Upd::Installing => {
                ui.spinner();
                ui.label("Téléchargement et installation de la mise à jour…");
            }
            Upd::Done => {
                ui.colored_label(GREEN, "✔ Mise à jour installée.");
                if ui.button("Redémarrer maintenant").clicked() {
                    action = Some("restart");
                }
                ui.label("(ou au prochain lancement)");
            }
            _ => {}
        });
        ui.add_space(4.0);
        match action {
            Some("install") => {
                if let Upd::Available(rel) = &self.upd {
                    let rel = rel.clone();
                    self.start_install(rel);
                }
            }
            Some("later") => self.upd_dismissed = true,
            Some("restart") => self.restart(&ctx),
            _ => {}
        }
    }

    fn banner_visible(&self) -> bool {
        match self.upd {
            Upd::Available(_) => !self.upd_dismissed,
            Upd::Installing | Upd::Done => true,
            _ => false,
        }
    }

    // ── fichiers ──

    fn pick_and_open(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file() {
            self.open(p);
        }
    }

    /// Remet à zéro tout ce qui dépend du document (foliotage, positions, nom d'export).
    fn reset_document(&mut self) {
        self.place.clear();
        self.skipped.clear();
        self.rm_done = false;
        self.last_pl = None;
        self.run = None;
        self.orient_cache.clear();
        self.exported = false;
        self.out_manual = false;
        self.out_name.clear();
        self.visited = [true, false, false, false];
        self.seen = [true, false, false, false];
        self.step = Step::File;
    }

    fn open(&mut self, path: PathBuf) {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.recents.retain(|r| Path::new(r) != path);
                return self.say(format!("Impossible de lire « {} » : {e}", path.display()), true);
            }
        };
        match load_page(&bytes, 0, self.want_scale()) {
            Err(e) => self.say(e, true),
            Ok((page, n)) => {
                self.reset_document();
                self.name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let p = path.to_string_lossy().into_owned();
                self.recents.retain(|r| *r != p);
                self.recents.insert(0, p);
                self.recents.truncate(8);
                self.thumb_rx = Some(spawn_thumbs(bytes.clone(), self.ctx.clone()));
                self.thumbs = (0..n).map(|_| None).collect();
                self.rm_doc = lopdf::Document::load_mem(&bytes).ok();
                self.path = Some(path);
                self.bytes = Some(bytes);
                self.n_pages = n;
                self.rm_from = 1;
                self.rm_to = n;
                self.cur = 0;
                self.page = Some(page);
                self.tex = None;
                self.scroll_thumb = true;
                self.say(format!("« {} » chargé ({n} page(s)).", self.name), false);
            }
        }
    }

    fn close(&mut self) {
        if !self.exported && (self.rm_done || !self.place.is_empty()) && self.run.is_none() {
            self.say("Foliotage non exporté fermé.", false);
        }
        self.reset_document();
        self.bytes = None;
        self.rm_doc = None;
        self.page = None;
        self.tex = None;
        self.thumbs.clear();
        self.thumb_rx = None;
        self.name.clear();
        self.path = None;
        self.n_pages = 0;
        self.cur = 0;
        self.say("Ouvrez un PDF (Ctrl+O, bouton ou glisser-déposer).", false);
    }

    fn want_scale(&self) -> f32 {
        (self.k * self.ctx.pixels_per_point()).clamp(0.5, 4.0)
    }

    fn goto(&mut self, idx: usize) {
        let Some(bytes) = &self.bytes else { return };
        match load_page(bytes, idx, self.want_scale()) {
            Ok((page, _)) => {
                self.cur = idx;
                self.page = Some(page);
                self.tex = None;
                self.scroll_thumb = true;
            }
            Err(e) => self.say(e, true),
        }
    }

    fn rerender(&mut self, scale: f32) {
        let Some(bytes) = &self.bytes else { return };
        if let Ok((page, _)) = load_page(bytes, self.cur, scale) {
            self.page = Some(page);
            self.tex = None;
        }
    }

    // ── réglages / historique ──

    fn sync_opts(&mut self) {
        self.opts.start = self.start_s.trim().parse().unwrap_or(1).max(1);
        self.opts.total = self.total_s.trim().parse().ok().filter(|t: &u32| *t > 0);
        self.opts.font_size = self.size_s.trim().parse::<f32>().unwrap_or(14.0).clamp(6.0, 72.0);
    }

    fn snap(&self) -> Snap {
        Snap { opts: self.opts.clone(), start: self.start_s.clone(), total: self.total_s.clone(), size: self.size_s.clone() }
    }

    fn apply(&mut self, s: &Snap) {
        self.opts = s.opts.clone();
        self.start_s = s.start.clone();
        self.total_s = s.total.clone();
        self.size_s = s.size.clone();
    }

    /// Enregistre un pas d'historique ; les modifications rapprochées (< 0,7 s) forment un seul pas.
    fn track_history(&mut self, now: f64) {
        let snap = self.snap();
        if snap != self.last_snap {
            if now - self.last_change > 0.7 {
                self.undo.push(self.last_snap.clone());
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
            }
            self.redo.clear();
            self.last_snap = snap;
            self.last_change = now;
        }
    }

    fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.last_snap.clone());
            self.apply(&s);
            self.last_snap = s;
            self.last_change = -10.0;
        }
    }

    fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.last_snap.clone());
            self.apply(&s);
            self.last_snap = s;
            self.last_change = -10.0;
        }
    }

    fn reset_opts(&mut self) {
        let keep = (self.opts.fx, self.opts.fy);
        self.opts = Options::default();
        (self.opts.fx, self.opts.fy) = keep;
        self.start_s = self.opts.start.to_string();
        self.total_s.clear();
        self.size_s = self.opts.font_size.to_string();
    }
    fn stamp_text(&self) -> String {
        render_text(&self.opts, self.cur as u32 + 1, self.n_pages.max(1) as u32)
    }

    // ── clavier ──

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let has_doc = self.bytes.is_some();
        let running = self.run.is_some();
        let (mut open, mut save, mut close, mut undo, mut redo, mut esc, mut enter) = (false, false, false, false, false, false, false);
        let mut step: Option<Step> = None;
        let mut settings_toggle = false;
        let mut nav = 0i32; // pages
        let mut jump: Option<usize> = None;
        let mut mv = (0.0f32, 0.0f32);
        ctx.input_mut(|i| {
            open = i.consume_key(Modifiers::COMMAND, Key::O);
            save = i.consume_key(Modifiers::COMMAND, Key::S);
            close = i.consume_key(Modifiers::COMMAND, Key::W);
            esc = i.key_pressed(Key::Escape);
            for (k, s) in [(Key::Num1, Step::File), (Key::Num2, Step::Rm), (Key::Num3, Step::Annexe), (Key::Num4, Step::Export)] {
                if i.consume_key(Modifiers::ALT, k) {
                    step = Some(s);
                }
            }
            if i.consume_key(Modifiers::ALT, Key::Comma) {
                settings_toggle = true;
            }
            if !typing {
                enter = running && i.consume_key(Modifiers::NONE, Key::Enter);
                redo = i.consume_key(Modifiers::COMMAND, Key::Y) || i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
                undo = i.consume_key(Modifiers::COMMAND, Key::Z);
                if has_doc {
                    if !running {
                        nav += i.consume_key(Modifiers::NONE, Key::PageDown) as i32;
                        nav -= i.consume_key(Modifiers::NONE, Key::PageUp) as i32;
                        if i.consume_key(Modifiers::NONE, Key::Home) {
                            jump = Some(0);
                        }
                        if i.consume_key(Modifiers::NONE, Key::End) {
                            jump = Some(usize::MAX);
                        }
                    }
                    for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
                        for (m, st) in [(Modifiers::SHIFT, 10.0), (Modifiers::NONE, 1.0)] {  // Maj d'abord : « NONE » accepte aussi Maj
                            if i.consume_key(m, key) {
                                mv.0 += dx * st;
                                mv.1 += dy * st;
                            }
                        }
                    }
                }
            }
        });
        if open {
            self.pick_and_open();
        }
        if let Some(s) = step {
            self.set_step(s);
        }
        if settings_toggle {
            self.settings_open = !self.settings_open;
        }
        if save && has_doc {
            if self.step == Step::Export {
                self.export();
            } else {
                self.set_step(Step::Export);
            }
        }
        if close && has_doc {
            self.close();
        }
        if undo && self.step == Step::Annexe {
            self.undo();
        }
        if redo && self.step == Step::Annexe {
            self.redo();
        }
        if esc {
            if typing {
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
            } else if running {
                self.cancel_run();
            } else if self.settings_open {
                self.settings_open = false;
            }
        }
        if enter {
            self.validate_run();
        }
        if mv != (0.0, 0.0) {
            self.nudge_live(mv.0, mv.1);
        }
        if let Some(j) = jump {
            self.goto_page(j.min(self.n_pages.saturating_sub(1)));
        }
        if nav != 0 {
            self.step_page(nav);
        }
    }

    fn step_page(&mut self, step: i32) {
        if self.n_pages == 0 || self.run.is_some() {
            return;
        }
        let target = (self.cur as i32 + step.signum()).clamp(0, self.n_pages as i32 - 1) as usize;
        if target != self.cur {
            self.goto(target);
        }
    }

    /// Molette sur l'aperçu : change de page (sauf si la page est agrandie et défile, ou pendant le foliotage).
    fn wheel_pages(&mut self, ctx: &egui::Context) {
        let over = ctx.pointer_hover_pos().is_some_and(|p| self.preview_rect.contains(p));
        if self.bytes.is_none() || !over || self.scrollable || self.run.is_some() {
            self.wheel = 0.0;
            return;
        }
        self.wheel -= ctx.input(|i| i.smooth_scroll_delta.y); // vers le bas = page suivante
        if self.wheel.abs() >= WHEEL_STEP {
            let s = self.wheel.signum() as i32;
            self.wheel = 0.0;
            self.step_page(s);
        }
    }

    // ── barre du bas ──

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let color = if self.status.err { RED } else { GREEN };
            let msg = RichText::new(&self.status.text).color(color);
            if let Some(out) = self.status.out.clone() {
                ui.label(msg);
                if ui.button("Ouvrir le PDF").clicked() {
                    open_in_os(&out, false);
                }
                if ui.button("Ouvrir le dossier").clicked() {
                    open_in_os(&out, true);
                }
            } else {
                ui.add(egui::Label::new(msg).wrap_mode(egui::TextWrapMode::Truncate));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (label, enabled): (&str, bool) = match self.step {
                    Step::File => ("Suivant : ReeMaker ▸", self.bytes.is_some()),
                    Step::Rm => {
                        if self.run.is_some() {
                            ("✔ Valider et page suivante", true)
                        } else if !self.rm_apply || self.rm_done {
                            ("Suivant : Annexio ▸", true)
                        } else {
                            ("▶ Démarrer le foliotage", self.bytes.is_some())
                        }
                    }
                    Step::Annexe => ("Suivant : Export ▸", true),
                    Step::Export => ("📤 Exporter le PDF…", self.can_export()),
                };
                let go = egui::Button::new(RichText::new(label).color(Color32::WHITE).strong()).fill(RED);
                if ui.add_enabled(enabled, go).clicked() {
                    match self.step {
                        Step::File => self.set_step(Step::Rm),
                        Step::Rm => {
                            if self.run.is_some() {
                                self.validate_run();
                            } else if !self.rm_apply || self.rm_done {
                                self.set_step(Step::Annexe);
                            } else {
                                self.start_run();
                            }
                        }
                        Step::Annexe => self.set_step(Step::Export),
                        Step::Export => self.export(),
                    }
                }
                if self.bytes.is_some() {
                    ui.label(format!("Page {}/{}", self.cur + 1, self.n_pages));
                }
            });
        });
        ui.add_space(4.0);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);

        // thème et taille de l'interface
        if self.applied_theme != Some(self.theme) {
            ctx.set_theme(match self.theme {
                1 => egui::ThemePreference::Light,
                2 => egui::ThemePreference::Dark,
                _ => egui::ThemePreference::System,
            });
            self.applied_theme = Some(self.theme);
        }
        let z = ctx.zoom_factor();
        if (z - self.ui_scale).abs() > 0.005 {
            self.ui_scale = z.clamp(0.8, 2.0); // Ctrl+ / Ctrl- gérés par egui
        }

        self.poll_update();
        self.handle_keys(&ctx);
        self.wheel_pages(&ctx);
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).next()) {
            self.open(p);
        }

        egui::Panel::top("steps").show(ui, |ui| {
            ui.add_space(5.0);
            self.steps_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::top("filestrip").show(ui, |ui| self.file_strip(ui));
        if self.banner_visible() {
            egui::Panel::top("update").show(ui, |ui| self.update_banner(ui));
        }
        egui::Panel::bottom("bar").show(ui, |ui| self.bottom_bar(ui));
        let side = egui::Panel::left("panel")
            .resizable(true) // glisser la bordure pour élargir / rétrécir (curseur ↔)
            .default_size(self.side_w)
            .size_range(SIDE_MIN..=SIDE_MAX)
            .show(ui, |ui| self.side_panel(ui));
        self.side_w = side.response.rect.width();
        if self.show_thumbs && self.bytes.is_some() {
            egui::Panel::left("thumbs").resizable(false).exact_size(THUMB_W + 22.0).show(ui, |ui| self.thumbs_panel(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| self.preview(ui));
        self.settings_window(&ctx);

        self.track_history(now);
        if now - self.last_save_check > 1.0 {
            self.last_save_check = now;
            self.save_settings_if_changed();
        }
    }

    fn on_exit(&mut self) {
        self.save_settings_if_changed();
    }
}

fn main() -> eframe::Result {
    update::cleanup();
    let s = settings::load();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Annexio")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../assets/logo-256.png")).expect("icône")),
        ..Default::default()
    };
    eframe::run_native(
        "Annexio",
        opts,
        Box::new(move |cc| {
            cc.egui_ctx.set_zoom_factor(s.ui_scale);
            let mut app = App::new(cc.egui_ctx.clone(), s);
            if let Some(p) = std::env::args().nth(1) {
                app.open(PathBuf::from(p)); // « Ouvrir avec » / glisser un PDF sur l'exe
            }
            Ok(Box::new(app))
        }),
    )
}
