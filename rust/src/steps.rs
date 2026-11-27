//! Barre à chevrons, bande fichier, panneaux des 4 étapes et fenêtre des paramètres.

use super::*;
use workflow::{Step, STEP_NAMES};

const CHEV_W: f32 = 172.0;
const CHEV_H: f32 = 38.0;
const TIP: f32 = 14.0;

impl App {
    // ───────────────────────── barre du haut : chevrons ─────────────────────────

    pub fn steps_bar(&mut self, ui: &mut egui::Ui) {
        let dark = ui.visuals().dark_mode;
        let mut goto: Option<Step> = None;
        ui.horizontal(|ui| {
            ui.heading("Annexio");
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.x = 3.0;
            for (i, step) in Step::ALL.into_iter().enumerate() {
                let (state, summary) = self.chev_state(i);
                let on = step == self.step;
                let enabled = i == 0 || self.bytes.is_some();
                let (bg, fg) = if on {
                    (if dark { Color32::from_rgb(43, 134, 176) } else { Color32::from_rgb(10, 85, 120) }, Color32::WHITE)
                } else if state == "done" {
                    if dark { (Color32::from_rgb(30, 51, 40), Color32::from_rgb(184, 230, 200)) } else { (Color32::from_rgb(215, 239, 225), Color32::from_rgb(20, 90, 55)) }
                } else if state == "skip" {
                    if dark { (Color32::from_rgb(40, 40, 40), Color32::from_gray(130)) } else { (Color32::from_rgb(235, 236, 240), Color32::from_gray(120)) }
                } else if dark {
                    (Color32::from_rgb(44, 44, 44), Color32::from_gray(205))
                } else {
                    (Color32::from_rgb(228, 231, 238), Color32::from_rgb(57, 67, 90))
                };
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(CHEV_W, CHEV_H), Sense::click());
                let alpha = if enabled { 1.0 } else { 0.45 };
                let pts = vec![
                    rect.left_top(),
                    Pos2::new(rect.right() - TIP, rect.top()),
                    Pos2::new(rect.right(), rect.center().y),
                    Pos2::new(rect.right() - TIP, rect.bottom()),
                    rect.left_bottom(),
                ];
                let bg = if resp.hovered() && enabled && !on { bg.gamma_multiply(1.15) } else { bg };
                ui.painter().add(egui::Shape::convex_polygon(pts, bg.gamma_multiply(alpha), Stroke::NONE));
                let c = Pos2::new(rect.left() + 20.0, rect.center().y);
                ui.painter().circle_stroke(c, 10.0, Stroke::new(1.5, fg.gamma_multiply(alpha)));
                let mark = if state == "done" && !on { "✓".to_string() } else { (i + 1).to_string() };
                if on {
                    ui.painter().circle_filled(c, 10.0, Color32::WHITE);
                }
                ui.painter().text(c, egui::Align2::CENTER_CENTER, mark, FontId::proportional(12.0), if on { bg } else { fg.gamma_multiply(alpha) });
                let tx = rect.left() + 36.0;
                if summary.is_empty() {
                    ui.painter().text(Pos2::new(tx, rect.center().y), egui::Align2::LEFT_CENTER, STEP_NAMES[i], FontId::proportional(14.0), fg.gamma_multiply(alpha));
                } else {
                    ui.painter().text(Pos2::new(tx, rect.center().y - 7.0), egui::Align2::LEFT_CENTER, STEP_NAMES[i], FontId::proportional(14.0), fg.gamma_multiply(alpha));
                    let mut sm = summary.clone();
                    if sm.chars().count() > 22 {
                        sm = sm.chars().take(21).collect::<String>() + "…";
                    }
                    ui.painter().text(Pos2::new(tx, rect.center().y + 8.0), egui::Align2::LEFT_CENTER, sm, FontId::proportional(10.5), fg.gamma_multiply(0.85 * alpha));
                }
                if resp.on_hover_text(format!("{} (Alt+{})", STEP_NAMES[i], i + 1)).clicked() && enabled {
                    goto = Some(step);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_sized([34.0, 34.0], egui::Button::new(RichText::new("⚙").size(18.0))).on_hover_text("Paramètres (Alt+,)").clicked() {
                    self.settings_open = !self.settings_open;
                }
                ui.small(format!("v{}", update::current_version()));
            });
        });
        if let Some(s) = goto {
            self.set_step(s);
        }
    }

    // ───────────────────────── bande fichier ─────────────────────────

    pub fn file_strip(&mut self, ui: &mut egui::Ui) {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if self.bytes.is_some() {
                ui.label(RichText::new(format!("📄 {}", self.name)).strong());
                ui.label(format!("· {} page(s)", self.n_pages));
                let nav = self.run.is_none();
                if ui.add_enabled(nav && self.cur > 0, egui::Button::new("‹")).on_hover_text("Page précédente (Page préc.)").clicked() {
                    self.step_page(-1);
                }
                ui.label(format!("Page {}/{}", self.cur + 1, self.n_pages));
                if ui.add_enabled(nav && self.cur + 1 < self.n_pages, egui::Button::new("›")).on_hover_text("Page suivante (Page suiv.)").clicked() {
                    self.step_page(1);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Fermer ✕").on_hover_text("Ctrl+W").clicked() {
                        self.close();
                    }
                });
            } else {
                ui.label("Aucun PDF — glissez-le dans la fenêtre ou cliquez sur « Ouvrir un PDF… » (Ctrl+O).");
            }
        });
        ui.add_space(3.0);
    }

    // ───────────────────────── panneau de gauche ─────────────────────────

    pub fn side_panel(&mut self, ui: &mut egui::Ui) {
        self.sync_opts();
        self.rm_sync();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.step {
            Step::File => self.file_step(ui),
            Step::Rm => self.rm_step(ui),
            Step::Annexe => self.annexe_step(ui),
            Step::Export => self.export_step(ui),
        });
    }

    fn file_step(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Fichier");
        let open = egui::Button::new(RichText::new("📂 Ouvrir un PDF…").size(15.0));
        if ui.add_sized([ui.available_width(), 56.0], open).on_hover_text("Ctrl+O").clicked() {
            self.pick_and_open();
        }
        ui.small("ou glissez un PDF dans la fenêtre. Tout reste sur votre ordinateur.");
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
    }

    fn rm_step(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("ReeMaker");
            ui.label(RichText::new("cartouche à 3 lignes").small().weak());
        });
        if self.run.is_some() {
            return self.rm_run_panel(ui);
        }
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.rm_apply, true, "Appliquer le cartouche");
            ui.radio_value(&mut self.rm_apply, false, "Ignorer cette étape");
        });
        let sig = (self.rm_all, self.rm_from, self.rm_to, self.rm_start_s.clone());
        ui.add_enabled_ui(self.rm_apply, |ui| {
            egui::Grid::new("rmgrid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Site");
                ui.text_edit_singleline(&mut self.rm.site);
                ui.end_row();
                ui.label("R.E.E.");
                ui.add(egui::TextEdit::singleline(&mut self.rm.ree).hint_text("ex. JPJ 701"));
                ui.end_row();
                ui.label("Indice");
                ui.add(egui::TextEdit::singleline(&mut self.rm.indice).hint_text("A"));
                ui.end_row();
                ui.label("Folio de départ");
                ui.text_edit_singleline(&mut self.rm_start_s);
                ui.end_row();
                ui.label("Type d'arrêt");
                ui.add(egui::TextEdit::singleline(&mut self.rm.arret).hint_text("TEM"));
                ui.end_row();
                ui.label("Cycle");
                ui.text_edit_singleline(&mut self.rm.cycle);
                ui.end_row();
            });
            ui.checkbox(&mut self.rm.show_cycle, "Afficher le cycle dans le cartouche");

            ui.separator();
            ui.label(RichText::new("Pages à folioter").strong());
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.rm_all, true, "Totalité");
                ui.radio_value(&mut self.rm_all, false, "Pages de");
                let max = self.n_pages.max(1);
                ui.add_enabled(!self.rm_all, egui::DragValue::new(&mut self.rm_from).range(1..=max));
                ui.label("à");
                ui.add_enabled(!self.rm_all, egui::DragValue::new(&mut self.rm_to).range(1..=max));
            });

            ui.separator();
            ui.label(RichText::new("Tranche(s) concernée(s)").strong());
            egui::Grid::new("trgrid").num_columns(5).spacing([10.0, 4.0]).show(ui, |ui| {
                for t in 0..10 {
                    ui.checkbox(&mut self.rm_tr[t], t.to_string());
                    if t % 5 == 4 {
                        ui.end_row();
                    }
                }
            });

            ui.separator();
            ui.label(RichText::new("Tampon").strong());
            ui.horizontal(|ui| {
                ui.label("Zoom");
                ui.add(egui::DragValue::new(&mut self.rm.zoom).range(0.5..=3.0).speed(0.05).fixed_decimals(1));
                ui.label("Orientation");
                let label = match self.rm_orient {
                    None => "Auto".to_string(),
                    Some(o) => format!("{o}°"),
                };
                egui::ComboBox::from_id_salt("orient").selected_text(label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.rm_orient, None, "Auto (sens du texte)");
                    for o in [0u16, 90, 180, 270] {
                        ui.selectable_value(&mut self.rm_orient, Some(o), format!("{o}°"));
                    }
                });
            });
        });
        if sig != (self.rm_all, self.rm_from, self.rm_to, self.rm_start_s.clone()) {
            self.rm_done = false; // plage ou folio de départ changés : à refaire
        }
        ui.add_space(8.0);
        if self.rm_apply {
            let label = if self.rm_done { "↺ Modifier le foliotage" } else { "▶ Démarrer le foliotage" };
            let b = egui::Button::new(RichText::new(label).color(Color32::WHITE).strong()).fill(RED);
            if ui.add_enabled(self.bytes.is_some(), b).clicked() {
                self.start_run();
            }
            if self.rm_done {
                ui.colored_label(GREEN, format!("✔ {} page(s) posées · folios {}.", self.place.len(), self.folio_range()));
            } else if self.bytes.is_some() {
                ui.small(format!("Pages concernées : {}.", self.rm_pages_label()));
            }
        } else {
            ui.small("Étape ignorée : aucun cartouche ne sera ajouté.");
        }
    }

    fn rm_pages_label(&self) -> String {
        let l = self.rm_pages();
        match (l.first(), l.last()) {
            (Some(&a), Some(&b)) if l.len() == self.n_pages => format!("toutes ({}) — de {} à {}", l.len(), a + 1, b + 1),
            (Some(&a), Some(&b)) => format!("de {} à {}", a + 1, b + 1),
            _ => "aucune".into(),
        }
    }

    fn rm_run_panel(&mut self, ui: &mut egui::Ui) {
        let Some((pos, total, i)) = self.run.as_ref().map(|r| (r.pos, r.list.len(), r.list[r.pos])) else { return };
        egui::Frame::group(ui.style()).fill(Color32::from_rgba_unmultiplied(199, 125, 0, 40)).show(ui, |ui| {
            ui.label(RichText::new("Foliotage en cours").strong());
            ui.label(format!("Page {} · folio {} · {}/{}", i + 1, self.folio_of(i), pos + 1, total));
        });
        ui.small("Clic : placer le tampon (ou le glisser). Entrée ou clic droit : valider et passer à la page suivante. Flèches : ajuster (Maj : ×10).");
        let b = egui::Button::new(RichText::new("✔ Valider et page suivante").color(Color32::WHITE).strong()).fill(RED);
        if ui.add_sized([ui.available_width(), 34.0], b).on_hover_text("Entrée").clicked() {
            self.validate_run();
            return;
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(pos > 0, egui::Button::new("‹ Précédente")).clicked() {
                self.prev_run();
            }
            if ui.button("Passer ›").on_hover_text("Ne pas folioter cette page").clicked() {
                self.skip_run();
            }
        });
        ui.horizontal(|ui| {
            if ui.small_button("↻ Pivoter").on_hover_text("Tourner le cartouche de 90°").clicked() {
                if let Some(r) = self.run.as_mut() {
                    r.pending.orient = (r.pending.orient + 90) % 360;
                }
            }
            if ui.small_button("Tampon −").clicked() {
                self.rm.zoom = ((self.rm.zoom - 0.1) * 10.0).round() / 10.0;
            }
            if ui.small_button("Tampon +").clicked() {
                self.rm.zoom = ((self.rm.zoom + 0.1) * 10.0).round() / 10.0;
            }
            self.rm.zoom = self.rm.zoom.clamp(0.5, 3.0);
        });
        ui.checkbox(&mut self.keep_pos, "Garder la position pour la suite");
        ui.separator();
        ui.label(RichText::new("Avancement").strong());
        let list = self.run.as_ref().map(|r| r.list.clone()).unwrap_or_default();
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (k, p) in list.iter().enumerate() {
                let (mark, text) = if k == pos {
                    ("●", format!("page {} → folio {}", p + 1, self.folio_of(*p)))
                } else if self.skipped.contains(p) {
                    ("⇥", format!("page {} — ignorée", p + 1))
                } else if self.place.contains_key(p) {
                    ("✔", format!("page {} → folio {}", p + 1, self.folio_of(*p)))
                } else {
                    ("○", format!("page {} → folio {}", p + 1, self.folio_of(*p)))
                };
                let t = RichText::new(format!("{mark} {text}"));
                ui.label(if k == pos { t.strong() } else { t });
            }
        });
        ui.add_space(6.0);
        if ui.button("Annuler le foliotage").on_hover_text("Échap").clicked() {
            self.cancel_run();
        }
    }

    fn annexe_step(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("Annexio");
            ui.label(RichText::new("tampon « Annexe X Folio n/N »").small().weak());
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.an_apply, true, "Appliquer le tampon");
            ui.radio_value(&mut self.an_apply, false, "Ignorer cette étape");
        });
        ui.add_enabled_ui(self.an_apply, |ui| {
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
                ui.add(egui::TextEdit::singleline(&mut self.total_s).hint_text("auto")).on_hover_text("Vide = nombre de pages du PDF");
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
                        self.place_annexe(h, v);
                    }
                }
            });
            ui.small("Glissez le rectangle rouge, ou cliquez sur la page. Flèches : ajuster d'1 pt (Maj : 10 pt).");
        });
    }

    fn export_step(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Export");
        let line = |ui: &mut egui::Ui, mark: &str, color: Color32, text: String| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(color, mark);
                ui.label(text);
            });
        };
        line(ui, "✓", GREEN, format!("Fichier : {} ({} pages)", self.name, self.n_pages));
        if !self.rm_apply {
            line(ui, "⇥", Color32::GRAY, "ReeMaker : ignoré".into());
        } else if self.rm_done {
            line(ui, "✓", GREEN, format!("ReeMaker : cartouche sur {} page(s), folios {}", self.place.len(), self.folio_range()));
        } else {
            line(ui, "⚠", Color32::from_rgb(199, 125, 0), format!("ReeMaker : foliotage non terminé ({} page(s) posée(s))", self.place.len()));
        }
        if !self.an_apply {
            line(ui, "⇥", Color32::GRAY, "Annexio : ignoré".into());
        } else if !self.seen[Step::Annexe.idx()] {
            line(ui, "⇥", Color32::GRAY, "Annexio : étape non ouverte (aucun tampon ajouté)".into());
        } else {
            line(ui, "✓", GREEN, format!("Annexio : « {} » sur {} page(s)", render_text(&self.opts, 1, self.n_pages.max(1) as u32), self.an_selected().len()));
        }
        ui.separator();
        if !self.out_manual {
            self.out_name = self.auto_name();
        }
        ui.label("Nom du fichier");
        if ui.add_sized([ui.available_width(), 24.0], egui::TextEdit::singleline(&mut self.out_name)).changed() {
            self.out_manual = true;
        }
        ui.horizontal(|ui| {
            if self.out_manual {
                ui.small("Nom modifié à la main.");
                if ui.small_button("↺ Nom automatique").clicked() {
                    self.out_manual = false;
                }
            } else {
                ui.small("Nom automatique d'après ReeMaker (REE, tranche, indice) ; modifiable.");
            }
        });
        ui.add_space(8.0);
        let b = egui::Button::new(RichText::new("📤 Exporter le PDF…").color(Color32::WHITE).strong()).fill(RED);
        if ui.add_enabled(self.can_export(), b).on_hover_text("Ctrl+S").clicked() {
            self.export();
        }
        ui.small("Le fichier d'origine n'est jamais modifié : un nouveau PDF est enregistré.");
    }

    // ───────────────────────── fenêtre des paramètres ─────────────────────────

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Paramètres").open(&mut open).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).resizable(false).default_width(380.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                ui.label(RichText::new("Apparence").strong());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.theme, 0, "Système");
                    ui.selectable_value(&mut self.theme, 1, "Clair");
                    ui.selectable_value(&mut self.theme, 2, "Sombre");
                });
                let mut pct = (self.ui_scale * 100.0).round();
                ui.horizontal(|ui| {
                    ui.label("Taille de l'interface");
                    if ui.add(egui::Slider::new(&mut pct, 80.0..=200.0).suffix(" %")).on_hover_text("Ctrl + / Ctrl - fonctionnent aussi").changed() {
                        self.ui_scale = pct / 100.0;
                        ctx.set_zoom_factor(self.ui_scale);
                    }
                    if ui.small_button("100 %").clicked() {
                        self.ui_scale = 1.0;
                        ctx.set_zoom_factor(1.0);
                    }
                });
                ui.checkbox(&mut self.show_thumbs, "Afficher les miniatures des pages");

                ui.separator();
                ui.label(RichText::new("Tampons").strong());
                ui.horizontal(|ui| {
                    ui.label("Taille de base du cartouche (pt)");
                    ui.add(egui::DragValue::new(&mut self.base_size).range(5.0..=30.0).speed(0.1).fixed_decimals(1));
                });
                ui.horizontal(|ui| {
                    ui.label("Version du cartouche");
                    ui.radio_value(&mut self.rm.cart_ver, 0, "V0 — modèle d'origine (défauts reproduits)");
                    ui.radio_value(&mut self.rm.cart_ver, 1, "V1 — version précédente");
                });
                ui.checkbox(&mut self.opts.white_fill, "Fond blanc sous les tampons");
                ui.checkbox(&mut self.keep_pos, "Garder la position pour la suite (foliotage)");
                ui.label("Modèle du nom du PDF exporté");
                ui.add(egui::TextEdit::singleline(&mut self.name_tpl).desired_width(f32::INFINITY));
                ui.small("Variables : {ree} {tranche} {indice} {site} {arret} {cycle} {nom} {folios} (« 36 à 54 », folios du cartouche ReeMaker) {folio1} {folio2}. Ce qui est entre [ ] disparaît si une variable est vide. Exemple : « Folio 36 à 54 de REE JPJ 701 Tr 0 Ind A ».");
                if ui.small_button("Modèle par défaut").clicked() {
                    self.name_tpl = settings::DEFAULT_NAME_TPL.to_string();
                }

                ui.separator();
                egui::CollapsingHeader::new("Raccourcis clavier").show(ui, |ui| {
                    for (k, d) in [
                        ("Alt+1 … Alt+4", "Fichier · ReeMaker · Annexio · Export"),
                        ("Ctrl+O / Ctrl+S / Ctrl+W", "Ouvrir / Exporter / Fermer"),
                        ("Entrée ou clic droit", "Valider la page (foliotage)"),
                        ("Échap", "Annuler le foliotage / quitter un champ"),
                        ("Flèches", "Déplacer le tampon (Maj : ×10)"),
                        ("Molette · Page préc./suiv.", "Changer de page"),
                        ("Ctrl+molette", "Zoom de l'aperçu"),
                        ("Ctrl+Z / Ctrl+Y", "Annuler / rétablir (étape Annexio)"),
                        ("Alt+,", "Paramètres"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(k).strong());
                            ui.label(d);
                        });
                    }
                });

                ui.separator();
                ui.label(RichText::new("Application").strong());
                ui.label(format!("Version {}", update::current_version()));
                ui.checkbox(&mut self.auto_update, "Vérifier les mises à jour au démarrage")
                    .on_hover_text("Contacte github.com pour savoir si une nouvelle version existe. Aucun de vos fichiers n'est envoyé.");
                match &self.upd {
                    Upd::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Recherche en cours…");
                        });
                    }
                    Upd::UpToDate => {
                        ui.colored_label(GREEN, "✔ Vous avez la dernière version.");
                    }
                    Upd::Available(rel) => {
                        ui.colored_label(RED, format!("Version {} disponible.", rel.version));
                    }
                    _ => {}
                }
                if let Some(e) = &self.upd_error {
                    ui.colored_label(RED, e);
                }
                let busy = matches!(self.upd, Upd::Checking | Upd::Installing);
                if ui.add_enabled(!busy, egui::Button::new("🔄 Rechercher une mise à jour")).clicked() {
                    self.start_check();
                }
                ui.small("Tout reste sur votre ordinateur, sauf cette vérification si elle est activée.");
            });
        });
        self.settings_open = open;
    }
}
