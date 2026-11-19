//! Interface graphique : aperçu du PDF, rectangle rouge déplaçable, génération. 100 % local.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use pdf_annexe::{box_size, render_text, stamp, Options};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

mod update;

const RED: Color32 = Color32::from_rgb(222, 0, 0);
const PREVIEW_SCALE: f32 = 1.6;

struct Page {
    image: egui::ColorImage,
    /// Taille visuelle de la page en points.
    pts: (f32, f32),
}

fn load_page(bytes: &[u8], idx: usize) -> Result<(Page, usize), String> {
    let pdf = Pdf::new(bytes.to_vec()).map_err(|e| format!("PDF illisible : {e:?}"))?;
    let pages = pdf.pages();
    let n = pages.len();
    let page = pages.get(idx).ok_or("Page introuvable")?;
    let (w, h) = page.render_dimensions();
    let px = hayro::render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings { x_scale: PREVIEW_SCALE, y_scale: PREVIEW_SCALE, bg_color: WHITE },
    );
    let image = egui::ColorImage::from_rgba_premultiplied(
        [px.width() as usize, px.height() as usize],
        px.data_as_u8_slice(),
    );
    Ok((Page { image, pts: (w, h) }, n))
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    File,
    Settings,
}

struct App {
    tab: Tab,
    bytes: Option<Vec<u8>>,
    name: String,
    path: Option<PathBuf>,
    n_pages: usize,
    cur: usize, // 0-based
    page: Option<Page>,
    tex: Option<egui::TextureHandle>,
    opts: Options,
    start_s: String,
    total_s: String,
    size_s: String,
    status: String,
    is_err: bool,
    upd: Upd,
    upd_rx: Option<Receiver<UpdMsg>>,
}

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

impl Default for App {
    fn default() -> Self {
        let opts = Options::default();
        App {
            tab: Tab::File,
            bytes: None,
            name: String::new(),
            path: None,
            n_pages: 0,
            cur: 0,
            page: None,
            tex: None,
            start_s: opts.start.to_string(),
            total_s: String::new(),
            size_s: opts.font_size.to_string(),
            opts,
            status: "Ouvrez un PDF (bouton ou glisser-déposer).".into(),
            is_err: false,
            upd: Upd::Idle,
            upd_rx: None,
        }
    }
}

impl App {
    fn say(&mut self, msg: impl Into<String>, err: bool) {
        self.status = msg.into();
        self.is_err = err;
    }

    fn open(&mut self, path: PathBuf) {
        match std::fs::read(&path) {
            Err(e) => self.say(format!("{} : {e}", path.display()), true),
            Ok(bytes) => match load_page(&bytes, 0) {
                Err(e) => self.say(e, true),
                Ok((page, n)) => {
                    self.name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    self.path = Some(path);
                    self.bytes = Some(bytes);
                    self.n_pages = n;
                    self.cur = 0;
                    self.page = Some(page);
                    self.tex = None;
                    self.say(format!("{} ({n} p.) chargé.", self.name), false);
                }
            },
        }
    }

    fn goto(&mut self, idx: usize) {
        let Some(bytes) = &self.bytes else { return };
        match load_page(bytes, idx) {
            Ok((page, _)) => {
                self.cur = idx;
                self.page = Some(page);
                self.tex = None;
            }
            Err(e) => self.say(e, true),
        }
    }

    fn start_check(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UpdMsg::Checked(update::check(env!("CARGO_PKG_VERSION"))));
            ctx.request_repaint();
        });
        self.upd_rx = Some(rx);
        self.upd = Upd::Checking;
    }

    fn start_install(&mut self, ctx: &egui::Context, rel: update::Release) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UpdMsg::Installed(update::install(&rel).map(|_| ())));
            ctx.request_repaint();
        });
        self.upd_rx = Some(rx);
        self.upd = Upd::Installing;
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
                self.say(e, true);
            }
            UpdMsg::Installed(Ok(())) => self.upd = Upd::Done,
            UpdMsg::Installed(Err(e)) => {
                self.upd = Upd::Idle;
                self.say(e, true);
            }
        }
    }

    fn update_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        match &self.upd {
            Upd::Idle => {
                if ui.small_button("🔄 Rechercher une mise à jour").clicked() {
                    self.start_check(&ctx);
                }
                ui.small("Contacte uniquement github.com ; aucun fichier n'est envoyé.");
            }
            Upd::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Recherche en cours…");
                });
            }
            Upd::UpToDate => {
                ui.colored_label(Color32::from_rgb(0, 120, 40), "✔ Vous avez la dernière version.");
                if ui.small_button("Revérifier").clicked() {
                    self.start_check(&ctx);
                }
            }
            Upd::Available(rel) => {
                let rel = rel.clone();
                ui.colored_label(RED, format!("Nouvelle version disponible : {}", rel.version));
                if rel.asset.is_some() {
                    if ui.button("⬇ Installer la mise à jour").clicked() {
                        self.start_install(&ctx, rel);
                    }
                } else {
                    ui.small("Pas de fichier pour ce système dans cette release.");
                    ui.hyperlink_to("Voir la release ↗", &rel.page);
                }
            }
            Upd::Installing => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Téléchargement et installation…");
                });
            }
            Upd::Done => {
                ui.colored_label(Color32::from_rgb(0, 120, 40), "✔ Mise à jour installée.");
                if ui.button("Redémarrer maintenant").clicked() {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).spawn();
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn sync_opts(&mut self) {
        self.opts.start = self.start_s.trim().parse().unwrap_or(1).max(1);
        self.opts.total = self.total_s.trim().parse().ok().filter(|t: &u32| *t > 0);
        self.opts.font_size = self.size_s.trim().parse::<f32>().unwrap_or(14.0).clamp(6.0, 72.0);
    }

    fn generate(&mut self) {
        let Some(bytes) = &self.bytes else { return };
        let stem = self.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let mut dlg = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{stem}_annexe.pdf"));
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dlg = dlg.set_directory(dir);
        }
        let Some(out) = dlg.save_file() else { return };
        match stamp(bytes, &self.opts).and_then(|(b, n)| std::fs::write(&out, b).map(|_| n).map_err(|e| e.to_string())) {
            Ok(n) => self.say(format!("Terminé : {n} page(s) tamponnée(s) → {}", out.display()), false),
            Err(e) => self.say(e, true),
        }
    }

    fn tabs_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Annexe PDF");
            ui.add_space(16.0);
            ui.selectable_value(&mut self.tab, Tab::File, "📄 Fichier");
            ui.selectable_value(&mut self.tab, Tab::Settings, "⚙ Paramètres");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(format!("v{}", env!("CARGO_PKG_VERSION")));
            });
        });
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        self.sync_opts();
        match self.tab {
            Tab::File => self.file_tab(ui),
            Tab::Settings => self.settings_tab(ui),
        }
        ui.add_space(8.0);
        let color = if self.is_err { RED } else { Color32::from_rgb(0, 120, 40) };
        ui.colored_label(color, &self.status);
    }

    fn file_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        if ui.add_sized([ui.available_width(), 32.0], egui::Button::new("📂 Ouvrir un PDF…")).clicked() {
            if let Some(p) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).pick_file() {
                self.open(p);
            }
        }
        ui.small("ou glissez un PDF dans la fenêtre");
        if self.bytes.is_none() {
            return;
        }
        ui.separator();
        ui.label(egui::RichText::new(&self.name).strong());
        ui.label(format!("{} page(s)", self.n_pages));
        ui.horizontal(|ui| {
            if ui.add_enabled(self.cur > 0, egui::Button::new("‹")).clicked() {
                self.goto(self.cur - 1);
            }
            ui.label(format!("Page {}/{}", self.cur + 1, self.n_pages));
            if ui.add_enabled(self.cur + 1 < self.n_pages, egui::Button::new("›")).clicked() {
                self.goto(self.cur + 1);
            }
        });
        ui.separator();
        let go = egui::Button::new(egui::RichText::new("Générer le PDF…").color(Color32::WHITE).strong()).fill(RED);
        if ui.add_sized([ui.available_width(), 36.0], go).clicked() {
            self.generate();
        }
        ui.small("Glissez le rectangle rouge, ou cliquez sur la page pour le placer.");
        ui.separator();
        if ui.button("Fermer le PDF").clicked() {
            self.close();
        }
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Tampon").strong());
        egui::Grid::new("opts").num_columns(2).spacing([8.0, 8.0]).show(ui, |ui| {
            ui.label("Annexe (X)");
            ui.text_edit_singleline(&mut self.opts.annexe);
            ui.end_row();
            ui.label("Texte");
            ui.text_edit_singleline(&mut self.opts.template);
            ui.end_row();
            ui.label("Premier folio");
            ui.text_edit_singleline(&mut self.start_s);
            ui.end_row();
            ui.label("Total (N)");
            ui.add(egui::TextEdit::singleline(&mut self.total_s).hint_text("auto"));
            ui.end_row();
            ui.label("Taille (pt)");
            ui.text_edit_singleline(&mut self.size_s);
            ui.end_row();
            ui.label("Pages");
            ui.add(egui::TextEdit::singleline(&mut self.opts.pages).hint_text("toutes (ex : 1-3,5)"));
            ui.end_row();
        });
        ui.small("Variables : {annexe} {n} {total}");
        ui.checkbox(&mut self.opts.white_fill, "Fond blanc");
        if ui.button("Réinitialiser").clicked() {
            self.reset_opts();
        }
        ui.separator();
        ui.label(egui::RichText::new("Application").strong());
        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
        self.update_ui(ui);
        ui.add_space(4.0);
        ui.hyperlink_to("Ouvrir le dépôt GitHub ↗", "https://github.com/DRFRrobin/PDF-annexe-Editor");
        ui.small("Tout reste sur votre ordinateur.");
    }

    fn close(&mut self) {
        self.bytes = None;
        self.page = None;
        self.tex = None;
        self.name.clear();
        self.path = None;
        self.n_pages = 0;
        self.cur = 0;
        self.say("Ouvrez un PDF (bouton ou glisser-déposer).", false);
    }

    fn reset_opts(&mut self) {
        let keep = (self.opts.fx, self.opts.fy);
        self.opts = Options::default();
        (self.opts.fx, self.opts.fy) = keep;
        self.start_s = self.opts.start.to_string();
        self.total_s.clear();
        self.size_s = self.opts.font_size.to_string();
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let Some(page) = &self.page else {
            ui.centered_and_justified(|ui| ui.label("Aucun PDF chargé."));
            return;
        };
        let tex = self.tex.get_or_insert_with(|| ui.ctx().load_texture("page", page.image.clone(), Default::default()));
        let (pw, ph) = page.pts;
        let avail = ui.available_size() - Vec2::splat(16.0);
        let k = (avail.x / pw).min(avail.y / ph).max(0.05); // pixels écran par point
        let size = Vec2::new(pw * k, ph * k);
        let origin = ui.max_rect().center() - size / 2.0;
        let page_rect = Rect::from_min_size(origin, size);
        let resp = ui.allocate_rect(page_rect, Sense::click_and_drag());
        let painter = ui.painter_at(ui.max_rect());
        painter.rect_filled(page_rect.expand(2.0), 0.0, Color32::from_black_alpha(60));
        painter.image(tex.id(), page_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

        let text = render_text(&self.opts, self.opts.start + self.cur as u32 - self.opts.start + 1, self.n_pages as u32);
        let s = self.opts.font_size;
        let (bw, bh) = box_size(&text, s);
        let fx_max = ((pw - bw) / pw).max(0.0);
        let fy_max = ((ph - bh) / ph).max(0.0);
        let cx = |o: &Options| o.fx.clamp(0.0, fx_max);
        let cy = |o: &Options| o.fy.clamp(0.0, fy_max);
        let box_rect = |o: &Options| Rect::from_min_size(origin + Vec2::new(cx(o) * pw * k, cy(o) * ph * k), Vec2::new(bw * k, bh * k));

        // clic sur la page : place le centre du rectangle
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
            self.opts.fx = (((p.x - origin.x) / k - bw / 2.0) / pw).clamp(0.0, fx_max);
            self.opts.fy = (((p.y - origin.y) / k - bh / 2.0) / ph).clamp(0.0, fy_max);
        }
        let r = box_rect(&self.opts);
        #[allow(deprecated)]
        let drag = ui.interact(r, egui::Id::new("anx_box"), Sense::drag());
        if drag.dragged() {
            let d = drag.drag_delta() / k;
            self.opts.fx = (cx(&self.opts) + d.x / pw).clamp(0.0, fx_max);
            self.opts.fy = (cy(&self.opts) + d.y / ph).clamp(0.0, fy_max);
        }
        if drag.hovered() || drag.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        let r = box_rect(&self.opts);
        if self.opts.white_fill {
            painter.rect_filled(r, 0.0, Color32::WHITE);
        }
        painter.rect_stroke(r, 0.0, Stroke::new(1.5 * k, RED), egui::StrokeKind::Inside);
        painter.text(r.center(), egui::Align2::CENTER_CENTER, &text, FontId::proportional(s * k * 0.95), RED);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_update();
        let dropped = ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).next());
        if let Some(p) = dropped {
            self.open(p);
        }
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(4.0);
            self.tabs_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::left("panel").resizable(false).exact_size(290.0).show(ui, |ui| self.side_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.preview(ui));
    }
}

fn main() -> eframe::Result {
    update::cleanup();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 760.0]).with_min_inner_size([760.0, 520.0]).with_title("Annexe PDF"),
        ..Default::default()
    };
    eframe::run_native("Annexe PDF", opts, Box::new(|_cc| {
        let mut app = App::default();
        if let Some(p) = std::env::args().nth(1) {
            app.open(PathBuf::from(p)); // « Ouvrir avec » / glisser un PDF sur l'exe
        }
        Ok(Box::new(app))
    }))
}
