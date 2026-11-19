//! Interface graphique : aperçu du PDF, rectangle rouge déplaçable, génération. 100 % local.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod settings;

use eframe::egui::{self, Align, Color32, FontId, Key, Layout, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use pdf_annexe::{box_size, parse_range, render_text, stamp, Options};
use settings::Settings;
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
enum Tab {
    File,
    Settings,
}

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

struct Status {
    text: String,
    err: bool,
    out: Option<PathBuf>,
}

struct App {
    ctx: egui::Context,
    tab: Tab,
    // document
    bytes: Option<Vec<u8>>,
    name: String,
    path: Option<PathBuf>,
    n_pages: usize,
    cur: usize, // 0-based
    page: Option<Page>,
    tex: Option<egui::TextureHandle>,
    thumbs: Vec<Option<(egui::TextureHandle, f32)>>, // (texture, largeur/hauteur)
    thumb_rx: Option<Receiver<(usize, egui::ColorImage)>>,
    scroll_thumb: bool,
    // tampon
    opts: Options,
    start_s: String,
    total_s: String,
    size_s: String,
    margin: f32,
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
    // historique
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last_snap: Snap,
    last_change: f64,
    // divers
    recents: Vec<String>,
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
        let mut app = App {
            ctx,
            tab: Tab::File,
            bytes: None,
            name: String::new(),
            path: None,
            n_pages: 0,
            cur: 0,
            page: None,
            tex: None,
            thumbs: Vec::new(),
            thumb_rx: None,
            scroll_thumb: false,
            opts,
            start_s: s.start.clone(),
            total_s: s.total.clone(),
            size_s: s.size.clone(),
            margin: s.margin,
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
            status: Status { text: String::new(), err: false, out: None },
            saved_text: s.to_text(),
            last_save_check: 0.0,
        };
        app.sync_opts();
        app.last_snap = app.snap();
        app.say("Ouvrez un PDF (Ctrl+O, bouton ou glisser-déposer).", false);
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

    // ── fichiers ──

    fn pick_and_open(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file() {
            self.open(p);
        }
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
                self.name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let p = path.to_string_lossy().into_owned();
                self.recents.retain(|r| *r != p);
                self.recents.insert(0, p);
                self.recents.truncate(8);
                self.thumb_rx = Some(spawn_thumbs(bytes.clone(), self.ctx.clone()));
                self.thumbs = (0..n).map(|_| None).collect();
                self.path = Some(path);
                self.bytes = Some(bytes);
                self.n_pages = n;
                self.cur = 0;
                self.page = Some(page);
                self.tex = None;
                self.scroll_thumb = true;
                self.say(format!("« {} » chargé ({n} page(s)).", self.name), false);
            }
        }
    }

    fn close(&mut self) {
        self.bytes = None;
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

    fn generate(&mut self) {
        let Some(bytes) = &self.bytes else { return };
        let stem = self.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let mut dlg = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{stem}_annexe.pdf"));
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        let Some(out) = dlg.save_file() else { return };
        if self.path.as_deref() == Some(out.as_path()) {
            return self.say("Choisissez un autre nom : le fichier d'origine ne doit pas être remplacé.", true);
        }
        match stamp(bytes, &self.opts) {
            Err(e) => self.say(e, true),
            Ok((data, n)) => match std::fs::write(&out, data) {
                Ok(()) => {
                    self.status = Status { text: format!("✔ {n} page(s) tamponnée(s) : {}", out.display()), err: false, out: Some(out) };
                }
                Err(e) => self.say(
                    format!("Impossible d'écrire « {} » ({e}). Le fichier est peut-être ouvert dans un autre programme.", out.display()),
                    true,
                ),
            },
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

    // ── position du tampon ──

    fn stamp_text(&self) -> String {
        render_text(&self.opts, self.cur as u32 + 1, self.n_pages.max(1) as u32)
    }

    /// h, v ∈ {0, 0.5, 1} : gauche/centre/droite, haut/milieu/bas (avec la marge).
    fn place(&mut self, h: f32, v: f32) {
        let Some(page) = &self.page else { return };
        let (pw, ph) = page.pts;
        let (bw, bh) = box_size(&self.stamp_text(), self.opts.font_size);
        let pos = |f: f32, page: f32, b: f32, m: f32| match f {
            x if x <= 0.0 => m,
            x if x >= 1.0 => page - b - m,
            _ => (page - b) / 2.0,
        };
        self.opts.fx = (pos(h, pw, bw, self.margin) / pw).clamp(0.0, 1.0);
        self.opts.fy = (pos(v, ph, bh, self.margin) / ph).clamp(0.0, 1.0);
    }

    fn nudge(&mut self, dx: f32, dy: f32) {
        let Some(page) = &self.page else { return };
        let (pw, ph) = page.pts;
        self.opts.fx = (self.opts.fx + dx / pw).clamp(0.0, 1.0);
        self.opts.fy = (self.opts.fy + dy / ph).clamp(0.0, 1.0);
    }

    // ── clavier ──

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let has_doc = self.bytes.is_some();
        let mut act = (false, false, false, false, false, false); // open, save, close, undo, redo, esc
        let mut nav = 0i32; // pages
        let mut jump: Option<usize> = None;
        let mut mv = (0.0f32, 0.0f32);
        ctx.input_mut(|i| {
            act.0 = i.consume_key(Modifiers::COMMAND, Key::O);
            act.1 = i.consume_key(Modifiers::COMMAND, Key::S);
            act.2 = i.consume_key(Modifiers::COMMAND, Key::W);
            act.5 = i.key_pressed(Key::Escape);
            if !typing {
                act.4 = i.consume_key(Modifiers::COMMAND, Key::Y) || i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
                act.3 = i.consume_key(Modifiers::COMMAND, Key::Z);
                if has_doc {
                    nav += i.consume_key(Modifiers::NONE, Key::PageDown) as i32;
                    nav -= i.consume_key(Modifiers::NONE, Key::PageUp) as i32;
                    if i.consume_key(Modifiers::NONE, Key::Home) {
                        jump = Some(0);
                    }
                    if i.consume_key(Modifiers::NONE, Key::End) {
                        jump = Some(usize::MAX);
                    }
                    for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
                        for (m, step) in [(Modifiers::SHIFT, 10.0), (Modifiers::NONE, 1.0)] {  // Maj d'abord : « NONE » accepte aussi Maj
                            if i.consume_key(m, key) {
                                mv.0 += dx * step;
                                mv.1 += dy * step;
                            }
                        }
                    }
                }
            }
        });
        if act.0 {
            self.pick_and_open();
        }
        if act.1 && has_doc {
            self.generate();
        }
        if act.2 && has_doc {
            self.close();
        }
        if act.3 {
            self.undo();
        }
        if act.4 {
            self.redo();
        }
        if act.5 {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        if mv != (0.0, 0.0) {
            self.nudge(mv.0, mv.1);
        }
        if let Some(j) = jump {
            self.goto(j.min(self.n_pages.saturating_sub(1)));
        }
        if nav != 0 {
            self.step_page(nav);
        }
    }

    fn step_page(&mut self, step: i32) {
        if self.n_pages == 0 {
            return;
        }
        let target = (self.cur as i32 + step.signum()).clamp(0, self.n_pages as i32 - 1) as usize;
        if target != self.cur {
            self.goto(target);
        }
    }

    /// Molette sur l'aperçu : change de page (sauf si la page est agrandie et défile).
    fn wheel_pages(&mut self, ctx: &egui::Context) {
        let over = ctx.pointer_hover_pos().is_some_and(|p| self.preview_rect.contains(p));
        if self.bytes.is_none() || !over || self.scrollable {
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

    // ── interface ──

    fn tabs_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Annexe PDF");
            ui.add_space(16.0);
            ui.selectable_value(&mut self.tab, Tab::File, "📄 Fichier");
            ui.selectable_value(&mut self.tab, Tab::Settings, "⚙ Paramètres");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.small(format!("v{}", env!("CARGO_PKG_VERSION")));
            });
        });
    }

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
                let go = egui::Button::new(RichText::new("Générer le PDF…").color(Color32::WHITE).strong()).fill(RED);
                if ui
                    .add_enabled(self.bytes.is_some(), go)
                    .on_hover_text("Ajoute le tampon et enregistre un nouveau PDF (Ctrl+S)")
                    .clicked()
                {
                    self.generate();
                }
                if self.bytes.is_some() {
                    ui.label(format!("Page {}/{}", self.cur + 1, self.n_pages));
                }
            });
        });
        ui.add_space(4.0);
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        self.sync_opts();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
            Tab::File => self.file_tab(ui),
            Tab::Settings => self.settings_tab(ui),
        });
    }

    fn file_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let open = egui::Button::new("📂 Ouvrir un PDF…");
        if ui.add_sized([ui.available_width(), 34.0], open).on_hover_text("Ctrl+O").clicked() {
            self.pick_and_open();
        }
        ui.small("ou glissez un PDF dans la fenêtre");
        if self.bytes.is_some() {
            ui.separator();
            ui.label(RichText::new(&self.name).strong());
            ui.label(format!("{} page(s)", self.n_pages));
            ui.horizontal(|ui| {
                if ui.add_enabled(self.cur > 0, egui::Button::new("‹")).on_hover_text("Page précédente (Page préc.)").clicked() {
                    self.step_page(-1);
                }
                ui.label(format!("Page {}/{}", self.cur + 1, self.n_pages));
                if ui.add_enabled(self.cur + 1 < self.n_pages, egui::Button::new("›")).on_hover_text("Page suivante (Page suiv.)").clicked() {
                    self.step_page(1);
                }
            });
            if ui.button("Fermer le PDF").on_hover_text("Ctrl+W").clicked() {
                self.close();
            }
        }
        if !self.recents.is_empty() {
            ui.separator();
            ui.label(RichText::new("Fichiers récents").strong());
            let mut to_open = None;
            for r in &self.recents {
                let name = Path::new(r).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| r.clone());
                if ui.link(name).on_hover_text(r).clicked() {
                    to_open = Some(PathBuf::from(r));
                }
            }
            if let Some(p) = to_open {
                self.open(p);
            }
            if ui.small_button("Effacer la liste").clicked() {
                self.recents.clear();
            }
        }
        ui.separator();
        ui.small("Glissez le rectangle rouge, ou cliquez sur la page pour le placer. Flèches : ajuster d'1 pt (Maj : 10 pt).");
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("Tampon").strong());
        egui::Grid::new("opts").num_columns(2).spacing([8.0, 8.0]).show(ui, |ui| {
            ui.label("Annexe (X)");
            ui.text_edit_singleline(&mut self.opts.annexe).on_hover_text("Remplace {annexe} dans le texte");
            ui.end_row();
            ui.label("Texte");
            ui.text_edit_singleline(&mut self.opts.template)
                .on_hover_text("{annexe} = annexe, {n} = numéro de folio, {total} = nombre total");
            ui.end_row();
            ui.label("Modèles");
            egui::ComboBox::from_id_salt("tpl").width(ui.available_width()).selected_text("Choisir…").show_ui(ui, |ui| {
                for t in TEMPLATES {
                    if ui.selectable_label(self.opts.template == t, t).clicked() {
                        self.opts.template = t.to_string();
                    }
                }
            });
            ui.end_row();
            ui.label("Premier folio");
            ui.text_edit_singleline(&mut self.start_s).on_hover_text("Numéro du folio de la première page");
            ui.end_row();
            ui.label("Total (N)");
            ui.add(egui::TextEdit::singleline(&mut self.total_s).hint_text("auto"))
                .on_hover_text("Vide = nombre de pages du PDF");
            ui.end_row();
            ui.label("Taille (pt)");
            ui.text_edit_singleline(&mut self.size_s).on_hover_text("Taille du texte, de 6 à 72 points");
            ui.end_row();
            ui.label("Pages");
            ui.add(egui::TextEdit::singleline(&mut self.opts.pages).hint_text("toutes (ex : 1-3,5)"))
                .on_hover_text("Pages à tamponner. Vide = toutes. Exemple : 1-3,5");
            ui.end_row();
        });
        ui.horizontal_wrapped(|ui| {
            let n = self.n_pages as u32;
            if ui.small_button("Toutes").clicked() {
                self.opts.pages.clear();
            }
            if ui.small_button("Paires").clicked() {
                self.opts.pages = (2..=n).step_by(2).map(|p| p.to_string()).collect::<Vec<_>>().join(",");
            }
            if ui.small_button("Impaires").clicked() {
                self.opts.pages = (1..=n).step_by(2).map(|p| p.to_string()).collect::<Vec<_>>().join(",");
            }
            if ui.small_button("Page courante").clicked() && n > 0 {
                self.opts.pages = (self.cur + 1).to_string();
            }
        });
        ui.checkbox(&mut self.opts.white_fill, "Fond blanc");
        ui.horizontal(|ui| {
            if ui.button("Réinitialiser").clicked() {
                self.reset_opts();
            }
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Annuler")).on_hover_text("Ctrl+Z").clicked() {
                self.undo();
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Rétablir")).on_hover_text("Ctrl+Y").clicked() {
                self.redo();
            }
        });

        ui.separator();
        ui.label(RichText::new("Position").strong());
        ui.horizontal(|ui| {
            ui.label("Marge");
            ui.add(egui::DragValue::new(&mut self.margin).range(0.0..=200.0).suffix(" pt"));
        });
        let presets: [(&str, f32, f32); 7] = [
            ("Haut gauche", 0.0, 0.0),
            ("Haut centre", 0.5, 0.0),
            ("Haut droite", 1.0, 0.0),
            ("Centre", 0.5, 0.5),
            ("Bas gauche", 0.0, 1.0),
            ("Bas centre", 0.5, 1.0),
            ("Bas droite", 1.0, 1.0),
        ];
        ui.horizontal_wrapped(|ui| {
            for (label, h, v) in presets {
                if ui.add_enabled(self.page.is_some(), egui::Button::new(label)).clicked() {
                    self.place(h, v);
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("Apparence").strong());
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.theme, 0, "Système");
            ui.selectable_value(&mut self.theme, 1, "Clair");
            ui.selectable_value(&mut self.theme, 2, "Sombre");
        });
        ui.horizontal(|ui| {
            ui.label("Taille de l'interface");
        });
        let mut pct = (self.ui_scale * 100.0).round();
        if ui.add(egui::Slider::new(&mut pct, 80.0..=200.0).suffix(" %")).on_hover_text("Ctrl + / Ctrl - fonctionnent aussi").changed() {
            self.ui_scale = pct / 100.0;
            ui.ctx().set_zoom_factor(self.ui_scale);
        }
        if ui.small_button("100 %").clicked() {
            self.ui_scale = 1.0;
            ui.ctx().set_zoom_factor(1.0);
        }
        ui.checkbox(&mut self.show_thumbs, "Afficher les miniatures des pages");

        ui.separator();
        egui::CollapsingHeader::new("Raccourcis clavier").show(ui, |ui| {
            for (k, d) in [
                ("Ctrl+O", "Ouvrir un PDF"),
                ("Ctrl+S", "Générer le PDF"),
                ("Ctrl+W", "Fermer le PDF"),
                ("Ctrl+Z / Ctrl+Y", "Annuler / rétablir"),
                ("Molette", "Changer de page"),
                ("Ctrl+molette", "Zoom de l'aperçu"),
                ("Page préc. / suiv.", "Changer de page"),
                ("Début / Fin", "Première / dernière page"),
                ("Flèches", "Déplacer le tampon (Maj : ×10)"),
                ("Échap", "Quitter un champ de saisie"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).strong());
                    ui.label(d);
                });
            }
        });
        ui.separator();
        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
        ui.small("Tout reste sur votre ordinateur. Réglages : mémorisés localement.");
    }

    fn thumbs_panel(&mut self, ui: &mut egui::Ui) {
        // reçoit les miniatures rendues en arrière-plan
        if let Some(rx) = &self.thumb_rx {
            while let Ok((i, img)) = rx.try_recv() {
                let aspect = img.size[0] as f32 / img.size[1].max(1) as f32;
                if let Some(slot) = self.thumbs.get_mut(i) {
                    *slot = Some((ui.ctx().load_texture(format!("th{i}"), img, Default::default()), aspect));
                }
            }
        }
        let stamped = parse_range(&self.opts.pages, self.n_pages as u32).unwrap_or_default();
        let mut goto = None;
        let mut scrolled = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            for i in 0..self.n_pages {
                let (aspect, tex) = match self.thumbs.get(i).and_then(|t| t.as_ref()) {
                    Some((t, a)) => (*a, Some(t.id())),
                    None => (0.7, None),
                };
                let size = Vec2::new(THUMB_W, THUMB_W / aspect);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let painter = ui.painter();
                match tex {
                    Some(id) => {
                        painter.image(id, rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        painter.rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
                        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "…", FontId::proportional(16.0), ui.visuals().text_color());
                    }
                }
                let current = i == self.cur;
                let stroke = if current { Stroke::new(3.0, RED) } else if resp.hovered() { Stroke::new(2.0, Color32::GRAY) } else { Stroke::new(1.0, Color32::from_gray(110)) };
                painter.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Outside);
                if stamped.contains(&(i as u32 + 1)) {
                    painter.circle_filled(rect.right_top() + Vec2::new(-9.0, 9.0), 5.0, RED);
                }
                ui.vertical_centered(|ui| ui.small(format!("{}", i + 1)));
                if resp.on_hover_text(format!("Page {}", i + 1)).clicked() {
                    goto = Some(i);
                }
                if current && self.scroll_thumb {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                    scrolled = true;
                }
                ui.add_space(4.0);
            }
        });
        if scrolled {
            self.scroll_thumb = false;
        }
        if let Some(i) = goto {
            self.goto(i);
        }
    }

    fn zoom_by(&mut self, f: f32) {
        self.zoom = Zoom::Custom((self.k * f).clamp(0.1, 6.0));
        self.zoom_at = self.ctx.input(|i| i.time);
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        self.preview_rect = ui.max_rect();
        let Some((pw, ph)) = self.page.as_ref().map(|p| p.pts) else {
            ui.centered_and_justified(|ui| {
                ui.label("Aucun PDF chargé. Ctrl+O pour en ouvrir un, ou glissez-le ici.");
            });
            return;
        };
        let now = ui.ctx().input(|i| i.time);

        // barre de zoom
        ui.horizontal(|ui| {
            if ui.button("−").on_hover_text("Zoom arrière (Ctrl+molette)").clicked() {
                self.zoom_by(0.8);
            }
            ui.label(format!("{:.0} %", self.k / BASE_K * 100.0));
            if ui.button("+").on_hover_text("Zoom avant (Ctrl+molette)").clicked() {
                self.zoom_by(1.25);
            }
            if ui.selectable_label(self.zoom == Zoom::FitPage, "Page entière").clicked() {
                self.zoom = Zoom::FitPage;
            }
            if ui.selectable_label(self.zoom == Zoom::FitWidth, "Largeur").clicked() {
                self.zoom = Zoom::FitWidth;
            }
        });
        ui.add_space(2.0);

        let avail = ui.available_size();
        let fit_page = ((avail.x - 16.0) / pw).min((avail.y - 16.0) / ph).max(0.05);
        let fit_w = ((avail.x - 30.0) / pw).max(0.05);
        let mut k = match self.zoom {
            Zoom::FitPage => fit_page,
            Zoom::FitWidth => fit_w,
            Zoom::Custom(z) => z,
        };
        // Ctrl+molette
        let over = ui.ctx().pointer_hover_pos().is_some_and(|p| self.preview_rect.contains(p));
        let zd = if over { ui.ctx().input(|i| i.zoom_delta()) } else { 1.0 };
        if (zd - 1.0).abs() > 1e-4 {
            k = (k * zd).clamp(0.1, 6.0);
            self.zoom = Zoom::Custom(k);
            self.zoom_at = now;
        }
        self.k = k;
        let size = Vec2::new(pw * k, ph * k);
        self.scrollable = size.x > avail.x + 1.0 || size.y > avail.y + 1.0;

        // netteté : re-rendu quand le zoom se stabilise
        let need = (k * ui.ctx().pixels_per_point()).clamp(0.5, 4.0).min(4096.0 / pw.max(ph));
        let have = self.page.as_ref().map_or(need, |p| p.scale);
        if need > have * 1.15 || need < have / 2.5 {
            if now - self.zoom_at > 0.25 {
                self.rerender(need);
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        let Some(page) = self.page.as_ref() else { return };
        let tex = self.tex.get_or_insert_with(|| ui.ctx().load_texture("page", page.image.clone(), Default::default()));
        let text = render_text(&self.opts, self.cur as u32 + 1, self.n_pages.max(1) as u32);
        let s = self.opts.font_size;
        let (bw, bh) = box_size(&text, s);
        let fx_max = ((pw - bw) / pw).max(0.0);
        let fy_max = ((ph - bh) / ph).max(0.0);
        let opts = &mut self.opts;

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let total = Vec2::new(size.x.max(ui.available_width() - 2.0), size.y.max(ui.available_height() - 2.0));
            let (area, resp) = ui.allocate_exact_size(total, Sense::click_and_drag());
            let page_rect = Rect::from_center_size(area.center(), size);
            let origin = page_rect.min;
            let painter = ui.painter_at(area);
            painter.rect_filled(page_rect.expand(2.0), 0.0, Color32::from_black_alpha(70));
            painter.image(tex.id(), page_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

            let box_rect = |o: &Options| {
                Rect::from_min_size(
                    origin + Vec2::new(o.fx.clamp(0.0, fx_max) * pw * k, o.fy.clamp(0.0, fy_max) * ph * k),
                    Vec2::new(bw * k, bh * k),
                )
            };
            // clic / glissement sur la page : place le centre du rectangle
            if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
                opts.fx = (((p.x - origin.x) / k - bw / 2.0) / pw).clamp(0.0, fx_max);
                opts.fy = (((p.y - origin.y) / k - bh / 2.0) / ph).clamp(0.0, fy_max);
            }
            let r = box_rect(opts);
            #[allow(deprecated)]
            let drag = ui.interact(r, egui::Id::new("anx_box"), Sense::drag());
            if drag.dragged() {
                let d = drag.drag_delta() / k;
                opts.fx = (opts.fx.clamp(0.0, fx_max) + d.x / pw).clamp(0.0, fx_max);
                opts.fy = (opts.fy.clamp(0.0, fy_max) + d.y / ph).clamp(0.0, fy_max);
            }
            if drag.hovered() || drag.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            let r = box_rect(opts);
            if opts.white_fill {
                painter.rect_filled(r, 0.0, Color32::WHITE);
            }
            painter.rect_stroke(r, 0.0, Stroke::new(1.5 * k, RED), egui::StrokeKind::Inside);
            painter.text(r.center(), egui::Align2::CENTER_CENTER, &text, FontId::proportional(s * k * 0.95), RED);
        });
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

        self.handle_keys(&ctx);
        self.wheel_pages(&ctx);
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).next()) {
            self.open(p);
        }

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(4.0);
            self.tabs_bar(ui);
            ui.add_space(4.0);
        });
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
    let s = settings::load();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Annexe PDF")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../../assets/logo-256.png")).expect("icône")),
        ..Default::default()
    };
    eframe::run_native(
        "Annexe PDF",
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
