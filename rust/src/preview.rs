//! Miniatures, aperçu de la page et calques (cartouche ReeMaker + tampon Annexe).

use super::*;
use annexio::reemaker::{self, Placement};
use workflow::Step;

struct CartOv {
    layout: reemaker::Layout,
    pl: Placement,
    live: bool,
    ghost: bool,
}

struct AnnOv {
    text: String,
    size: f32,
    live: bool,
}

/// Dessine le cartouche (traits + texte, éventuellement tourné) sur la page affichée.
fn paint_cart(painter: &egui::Painter, origin: Pos2, k: f32, vis: (f32, f32), l: &reemaker::Layout, pl: &Placement, fill: bool, alpha: f32) {
    let o = pl.orient % 360;
    let bb = reemaker::bbox(l, o);
    let (vx, vy) = reemaker::place(pl.fx, pl.fy, vis.0, vis.1, bb);
    let pt = |lx: f32, ly: f32| {
        let (x, y) = reemaker::local_to_visual(l, o, vx, vy, lx, ly);
        origin + Vec2::new(x * k, y * k)
    };
    let red = RED.gamma_multiply(alpha);
    let corners = vec![pt(0.0, 0.0), pt(l.w, 0.0), pt(l.w, l.h), pt(0.0, l.h)];
    if fill {
        painter.add(egui::Shape::convex_polygon(corners.clone(), Color32::WHITE.gamma_multiply(alpha), Stroke::NONE));
    }
    let st = Stroke::new((l.bw * k).max(1.0), red);
    painter.add(egui::Shape::closed_line(corners, st));
    painter.line_segment([pt(0.0, l.row_h), pt(l.w, l.row_h)], st);
    painter.line_segment([pt(0.0, 2.0 * l.row_h), pt(l.w, 2.0 * l.row_h)], st);
    painter.line_segment([pt(l.left_w, l.row_h), pt(l.left_w, l.h)], st);
    let font = FontId::proportional((l.fs * k).max(4.0));
    let angle = (o as f32).to_radians();
    let text = |s: &str, row: usize, centered: bool| {
        let galley = painter.layout_no_wrap(s.to_string(), font.clone(), red);
        let (gw, gh) = (galley.size().x / k, galley.size().y / k);
        let x = if centered { l.left_w + (l.right_w - gw) / 2.0 } else { l.padx };
        let y = row as f32 * l.row_h + (l.row_h - gh) / 2.0;
        painter.add(egui::epaint::TextShape::new(pt(x, y), galley, red).with_angle(angle));
    };
    text(&l.t.l1, 0, false);
    text(&l.t.l2a, 1, false);
    text(&l.t.l2b, 1, true);
    text(&l.t.l3a, 2, false);
    text(&l.t.l3b, 2, true);
}

impl App {
    // ───────────────────────── calques ─────────────────────────

    fn cart_overlay(&self) -> Option<CartOv> {
        if !self.rm_apply || self.bytes.is_none() {
            return None;
        }
        let i = self.cur;
        let layout = reemaker::layout(&self.rm, self.folio_of(i));
        if let Some(r) = &self.run {
            if r.list[r.pos] == i {
                return Some(CartOv { layout, pl: r.pending, live: true, ghost: false });
            }
        }
        if let Some(pl) = self.place.get(&i) {
            return Some(CartOv { layout, pl: *pl, live: false, ghost: false });
        }
        if self.run.is_none() && !self.rm_done && self.step == Step::Rm && self.rm_pages().contains(&i) {
            let pl = Placement { fx: workflow::DEFAULT_PL.0, fy: workflow::DEFAULT_PL.1, orient: self.rm_orient.unwrap_or(0) };
            return Some(CartOv { layout, pl, live: false, ghost: true });
        }
        None
    }

    fn annex_overlay(&self) -> Option<AnnOv> {
        if !self.an_active() || !self.an_selected().contains(&(self.cur as u32 + 1)) {
            return None;
        }
        Some(AnnOv { text: self.stamp_text(), size: self.opts.font_size, live: self.step == Step::Annexe })
    }

    /// h, v ∈ {0, 0.5, 1} : gauche/centre/droite, haut/milieu/bas (avec la marge).
    pub fn place_annexe(&mut self, h: f32, v: f32) {
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

    /// Flèches : déplace le calque actif (cartouche pendant le foliotage, tampon Annexe sur son étape).
    pub fn nudge_live(&mut self, dx: f32, dy: f32) -> bool {
        let Some(page) = &self.page else { return false };
        let (pw, ph) = page.pts;
        let cur = self.cur;
        if self.step == Step::Rm {
            if let Some(r) = self.run.as_mut() {
                if r.list[r.pos] == cur {
                    r.pending.fx = (r.pending.fx + dx / pw).clamp(0.0, 1.0);
                    r.pending.fy = (r.pending.fy + dy / ph).clamp(0.0, 1.0);
                    return true;
                }
            }
        }
        if self.step == Step::Annexe && self.an_apply {
            self.opts.fx = (self.opts.fx + dx / pw).clamp(0.0, 1.0);
            self.opts.fy = (self.opts.fy + dy / ph).clamp(0.0, 1.0);
            return true;
        }
        false
    }

    // ───────────────────────── miniatures ─────────────────────────

    pub fn thumbs_panel(&mut self, ui: &mut egui::Ui) {
        // reçoit les miniatures rendues en arrière-plan
        if let Some(rx) = &self.thumb_rx {
            while let Ok((i, img)) = rx.try_recv() {
                let aspect = img.size[0] as f32 / img.size[1].max(1) as f32;
                if let Some(slot) = self.thumbs.get_mut(i) {
                    *slot = Some((ui.ctx().load_texture(format!("th{i}"), img, Default::default()), aspect));
                }
            }
        }
        let selected = self.an_selected();
        let an_on = self.an_active();
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
                let stamped = (self.rm_apply && self.place.contains_key(&i)) || (an_on && selected.contains(&(i as u32 + 1)));
                if stamped {
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
            self.goto_page(i);
        }
    }

    pub fn zoom_by(&mut self, f: f32) {
        self.zoom = Zoom::Custom((self.k * f).clamp(0.1, 6.0));
        self.zoom_at = self.ctx.input(|i| i.time);
    }

    // ───────────────────────── aperçu ─────────────────────────

    pub fn preview(&mut self, ui: &mut egui::Ui) {
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

        // calques à dessiner
        let cart = self.cart_overlay();
        let ann = self.annex_overlay();
        let fill = self.opts.white_fill;
        let cart_live = cart.as_ref().is_some_and(|c| c.live);
        let ann_live = !cart_live && ann.as_ref().is_some_and(|a| a.live);
        let (mut cfx, mut cfy) = cart.as_ref().map(|c| (c.pl.fx, c.pl.fy)).unwrap_or((0.0, 0.0));
        let (mut afx, mut afy) = (self.opts.fx, self.opts.fy);
        let cart_bb = cart.as_ref().map(|c| reemaker::bbox(&c.layout, c.pl.orient));
        let ann_bb = ann.as_ref().map(|a| box_size(&a.text, a.size));
        let mut validate = false;

        let Some(page) = self.page.as_ref() else { return };
        let tex = self.tex.get_or_insert_with(|| ui.ctx().load_texture("page", page.image.clone(), Default::default()));

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let total = Vec2::new(size.x.max(ui.available_width() - 2.0), size.y.max(ui.available_height() - 2.0));
            let (area, resp) = ui.allocate_exact_size(total, Sense::click_and_drag());
            let page_rect = Rect::from_center_size(area.center(), size);
            let origin = page_rect.min;
            let painter = ui.painter_at(area);
            painter.rect_filled(page_rect.expand(2.0), 0.0, Color32::from_black_alpha(70));
            painter.image(tex.id(), page_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

            // calque actif : clic = placer le centre, glisser = déplacer, clic droit = valider (foliotage)
            let live = if cart_live { cart_bb.map(|b| (b, true)) } else if ann_live { ann_bb.map(|b| (b, false)) } else { None };
            if let Some((bb, is_cart)) = live {
                let (fx, fy) = if is_cart { (&mut cfx, &mut cfy) } else { (&mut afx, &mut afy) };
                let fx_max = ((pw - bb.0) / pw).max(0.0);
                let fy_max = ((ph - bb.1) / ph).max(0.0);
                if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
                    *fx = (((p.x - origin.x) / k - bb.0 / 2.0) / pw).clamp(0.0, fx_max);
                    *fy = (((p.y - origin.y) / k - bb.1 / 2.0) / ph).clamp(0.0, fy_max);
                }
                let r = Rect::from_min_size(
                    origin + Vec2::new(fx.clamp(0.0, fx_max) * pw * k, fy.clamp(0.0, fy_max) * ph * k),
                    Vec2::new(bb.0 * k, bb.1 * k),
                );
                #[allow(deprecated)]
                let drag = ui.interact(r, egui::Id::new("live_box"), Sense::click_and_drag());
                if drag.dragged() {
                    let d = drag.drag_delta() / k;
                    *fx = (fx.clamp(0.0, fx_max) + d.x / pw).clamp(0.0, fx_max);
                    *fy = (fy.clamp(0.0, fy_max) + d.y / ph).clamp(0.0, fy_max);
                }
                if drag.hovered() || drag.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                if is_cart && (resp.secondary_clicked() || drag.secondary_clicked()) {
                    validate = true;
                }
            }

            if let Some(c) = &cart {
                let pl = Placement { fx: cfx, fy: cfy, orient: c.pl.orient };
                paint_cart(&painter, origin, k, (pw, ph), &c.layout, &pl, fill, if c.ghost { 0.5 } else { 1.0 });
            }
            if let (Some(a), Some((bw, bh))) = (&ann, ann_bb) {
                let fx_max = ((pw - bw) / pw).max(0.0);
                let fy_max = ((ph - bh) / ph).max(0.0);
                let r = Rect::from_min_size(
                    origin + Vec2::new(afx.clamp(0.0, fx_max) * pw * k, afy.clamp(0.0, fy_max) * ph * k),
                    Vec2::new(bw * k, bh * k),
                );
                if fill {
                    painter.rect_filled(r, 0.0, Color32::WHITE);
                }
                painter.rect_stroke(r, 0.0, Stroke::new(1.5 * k, RED), egui::StrokeKind::Inside);
                painter.text(r.center(), egui::Align2::CENTER_CENTER, &a.text, FontId::proportional(a.size * k * 0.95), RED);
            }
        });

        if cart_live {
            if let Some(r) = self.run.as_mut() {
                r.pending.fx = cfx;
                r.pending.fy = cfy;
            }
        }
        if ann_live {
            self.opts.fx = afx;
            self.opts.fy = afy;
        }
        if validate {
            self.validate_run();
        }
    }
}
