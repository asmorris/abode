//! The garden plan: one garden bed's rows, patches and plants for a year.

use super::{Draw, Hit, PlanState, SnapSet, rgb};
use crate::theme::{self, c, icon};
use crate::ui::Env;
use crate::ui::garden::{crop_color, set_year};
use abode_core::garden::Hint;
use abode_core::geometry::*;
use abode_core::*;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape as EShape, Stroke, Ui, vec2};

pub(super) fn add_snaps(s: &mut SnapSet, project: &Project, level: &Level, garden: FeatureId, year: i32, exclude: Option<EntityRef>) {
    if let Some(f) = level.features.get(&garden) {
        s.add_path(&f.points, true);
    }
    for p in project.plantings_in(garden, year) {
        if exclude == Some(EntityRef::Planting(p.id)) {
            continue;
        }
        match p.layout {
            Layout::Row => s.add_path(&p.points, false),
            Layout::Patch => s.add_path(&p.points, true),
            Layout::Plant => s.points.extend(p.points.first().copied()),
        }
    }
}

pub(super) fn hit_test(project: &Project, level: &Level, garden: FeatureId, year: i32, sel: Option<EntityRef>, w: Vec2, px: f64) -> Option<Hit> {
    let plantings = project.plantings_in(garden, year);
    if let Some(EntityRef::Planting(id)) = sel {
        if let Some(p) = plantings.iter().find(|p| p.id == id && p.layout != Layout::Plant) {
            if let Some(i) = p.points.iter().position(|v| v.distance(w) < 8.0 * px) {
                return Some(Hit::PlantingVertex(id, i));
            }
        }
    }
    // Top to bottom: single plants, then rows, then patches.
    for p in plantings.iter().rev() {
        let hit = match p.layout {
            Layout::Plant => p.distance_to(w) < 4.0 * px,
            Layout::Row => p.distance_to(w) < (p.spacing_m() * 0.4).max(6.0 * px),
            Layout::Patch => p.contains(w),
        };
        if hit {
            return Some(Hit::Planting(p.id));
        }
    }
    level.features.get(&garden).filter(|f| f.contains(w)).map(|f| Hit::Feature(f.id))
}

/// When dragging a planting with snapping on, it moves in these steps.
pub(super) fn move_step(grid: f64) -> f64 {
    grid / 2.0
}

pub(super) fn add_planting(project: &mut Project, env: &mut Env, st: &mut PlanState, layout: Layout, points: Vec<Vec2>) {
    let Some(g) = st.garden else { return };
    let Some((_, bed)) = project.find_feature(g) else { return };
    let inside = match layout {
        Layout::Patch => point_in_polygon(label_point(&points), &bed.points),
        _ => points.iter().any(|p| point_in_polygon(*p, &bed.points)),
    };
    if !inside {
        env.toasts.info("Draw it inside the garden bed.");
        return;
    }
    let p = Planting::new(g, st.year, layout, points, &st.crop);
    let id = p.id;
    project.plantings.push(p);
    env.select(EntityRef::Planting(id));
}

/// "◀ 2026 ▶" for the garden plan's toolbar.
pub(super) fn year_stepper(ui: &mut Ui, project: &Project, garden: FeatureId, year: &mut i32, today: chrono::NaiveDate) {
    let name = project.entity_name(EntityRef::Feature(garden));
    ui.label(egui::RichText::new(format!("{}  {name}", icon::PLANT)).color(c::forest()));
    let before = *year;
    if theme::icon_button_sized(ui, icon::CARET_LEFT, "Previous year", false, 28.0).clicked() {
        *year -= 1;
    }
    let this = chrono::Datelike::year(&today);
    let text = egui::RichText::new(year.to_string()).strong();
    let r = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
    if *year != this && r.on_hover_text(format!("Back to {this}")).clicked() {
        *year = this;
    }
    if theme::icon_button_sized(ui, icon::CARET_RIGHT, "Next year", false, 28.0).clicked() {
        *year += 1;
    }
    if *year != before {
        set_year(ui.ctx(), *year);
    }
}

pub(super) fn crop_picker(ui: &mut Ui, id: &str, crop: &mut String) {
    crate::ui::garden::crop_combo(ui, id, crop);
}

impl Draw<'_> {
    /// Plantings in every garden bed, small, on the Outside plan.
    pub(super) fn garden_previews(&self, project: &Project, li: usize, year: i32) {
        let level = &project.levels[li];
        for f in level.features.values().filter(|f| f.kind.is_garden()) {
            for p in project.plantings_in(f.id, year) {
                self.planting(p, false, false, false);
            }
        }
    }

    /// The garden plan: everything else faded, the bed, then its plantings.
    pub(super) fn garden(&self, project: &Project, li: usize, garden: FeatureId, year: i32, ghost: bool) {
        let painter = self.painter;
        let Some(bed) = project.levels[li].features.get(&garden) else { return };
        painter.rect_filled(self.rect, 0.0, c::canvas().gamma_multiply(0.72));
        let soil = theme::mix(c::canvas(), rgb(bed.display_color()), 0.38);
        let pts = self.screen(&bed.points);
        super::fill_polygon(painter, &bed.points, &pts, soil);
        painter.add(EShape::closed_line(pts.clone(), Stroke::new(2.0_f32, theme::mix(c::canvas(), rgb(bed.display_color()), 0.95))));

        if ghost {
            for p in project.plantings.iter().filter(|p| p.garden == garden && p.year == year - 1 && !p.in_year(year)) {
                self.ghost(p);
            }
        }
        let sel = match self.sel {
            Some(EntityRef::Planting(id)) => Some(id),
            _ => None,
        };
        // Neighbours of the selected planting get a hint ring.
        let hints = sel.map(|id| project.hints(id)).unwrap_or_default();
        for p in project.plantings_in(garden, year) {
            let selected = sel == Some(p.id);
            let hovered = self.hover == Some(Hit::Planting(p.id));
            self.planting(p, true, selected, hovered);
            for h in &hints {
                match h {
                    Hint::Good { other } if *other == p.id => self.ring(p, c::forest()),
                    Hint::Bad { other } if *other == p.id => self.ring(p, c::coral()),
                    _ => {}
                }
            }
        }
        if let Some(p) = sel.and_then(|id| project.planting(id)) {
            if p.layout != Layout::Plant {
                self.handles(&p.points);
            }
            if p.layout == Layout::Row && self.show_dims {
                self.edge_dims(&p.points, false, c::teal());
            }
        }
        if self.show_dims {
            self.edge_dims(&bed.points, true, c::muted());
        }
    }

    fn planting(&self, p: &Planting, detailed: bool, selected: bool, hovered: bool) {
        let painter = self.painter;
        let base = rgb(p.color.unwrap_or_else(|| crop_color(&p.crop)));
        let zoom = self.view.zoom as f64;
        let line = if selected { c::teal() } else if hovered { theme::mix(base, c::ink(), 0.35) } else { theme::mix(base, c::ink(), 0.15) };
        let spacing = p.spacing_m();
        let dot_r = ((spacing * 0.22).min(0.12) * zoom).clamp(1.5, 24.0) as f32;
        let show_dots = detailed && spacing * zoom > 7.0;
        let finished = p.removed.is_some();
        let alpha = if finished { 0.35 } else { 1.0 };
        match p.layout {
            Layout::Row => {
                let pts = self.screen(&p.points);
                let w = ((spacing * 0.7 * zoom) as f32).clamp(3.0, 60.0);
                painter.add(EShape::line(pts.clone(), Stroke::new(w, theme::mix(c::canvas(), base, 0.45).gamma_multiply(alpha))));
                if selected || hovered {
                    painter.add(EShape::line(pts, Stroke::new(1.5_f32, line)));
                }
                if show_dots {
                    let len = p.length();
                    let n = p.plant_count().clamp(1, 600) as usize;
                    let step = if n > 1 { len / (n - 1) as f64 } else { 0.0 };
                    for i in 0..n {
                        if let Some((q, _)) = point_along(&p.points, (i as f64 * step).min(len)) {
                            painter.circle_filled(self.to(q), dot_r, base.gamma_multiply(alpha));
                        }
                    }
                }
            }
            Layout::Patch => {
                let pts = self.screen(&p.points);
                super::fill_polygon(painter, &p.points, &pts, theme::mix(c::canvas(), base, 0.35).gamma_multiply(alpha));
                painter.add(EShape::closed_line(pts, Stroke::new(if selected { 2.5 } else { 1.2_f32 }, line)));
                if show_dots {
                    if let Some((lo, hi)) = bounds(&p.points) {
                        let row = p.row_spacing_m();
                        let (nx, ny) = (((hi.x - lo.x) / spacing) as usize + 1, ((hi.y - lo.y) / row) as usize + 1);
                        if nx * ny <= 1500 {
                            let ox = lo.x + ((hi.x - lo.x) - (nx - 1) as f64 * spacing) / 2.0;
                            let oy = lo.y + ((hi.y - lo.y) - (ny - 1) as f64 * row) / 2.0;
                            for j in 0..ny {
                                for i in 0..nx {
                                    let q = Vec2::new(ox + i as f64 * spacing, oy + j as f64 * row);
                                    if point_in_polygon(q, &p.points) {
                                        painter.circle_filled(self.to(q), dot_r, base.gamma_multiply(0.85 * alpha));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Layout::Plant => {
                let Some(cp) = p.points.first() else { return };
                let r = ((p.size / 2.0 * zoom) as f32).max(3.0);
                painter.circle_filled(self.to(*cp), r, theme::mix(c::canvas(), base, 0.55).gamma_multiply(alpha));
                painter.circle_stroke(self.to(*cp), r, Stroke::new(if selected { 2.5 } else { 1.2_f32 }, line));
                if detailed {
                    painter.circle_filled(self.to(*cp), (r * 0.18).clamp(1.5, 5.0), base);
                }
            }
        }
        if detailed {
            self.planting_label(p, finished);
        }
    }

    fn planting_label(&self, p: &Planting, finished: bool) {
        let name = p.display_name();
        let font = FontId::new(11.5, theme::medium());
        let color = if finished { c::faint() } else { c::ink() };
        match p.layout {
            Layout::Row => {
                let (Some(a), Some(b)) = (p.points.first(), p.points.last()) else { return };
                let (sa, sb) = (self.to(*a), self.to(*b));
                let len = sa.distance(sb);
                let galley = self.painter.layout_no_wrap(name.clone(), font.clone(), color);
                if galley.size().x + 8.0 > len {
                    return;
                }
                let mid = sa.lerp(sb, 0.5);
                let w = ((p.spacing_m() * 0.7 * self.view.zoom as f64) as f32).clamp(3.0, 60.0);
                let mut angle = (sb - sa).angle();
                if angle.abs() > std::f32::consts::FRAC_PI_2 {
                    angle += std::f32::consts::PI;
                }
                let normal = vec2(-angle.sin(), angle.cos());
                super::text_along(self.painter, mid - normal * (w / 2.0 + 9.0), angle, name, font, color);
            }
            Layout::Patch => {
                let c = self.to(label_point(&p.points));
                self.boxed_label(c, &name, font, color);
            }
            Layout::Plant => {
                let Some(cp) = p.points.first() else { return };
                let r = ((p.size / 2.0 * self.view.zoom as f64) as f32).max(3.0);
                let galley = self.painter.layout_no_wrap(name, font, color);
                if r * 2.0 > 26.0 || galley.size().x < 120.0 {
                    let pos: Pos2 = self.to(*cp) + vec2(0.0, r + 3.0);
                    self.painter.galley(pos - vec2(galley.size().x / 2.0, 0.0), galley, color);
                }
            }
        }
    }

    fn boxed_label(&self, at: Pos2, text: &str, font: FontId, color: Color32) {
        let galley = self.painter.layout_no_wrap(text.to_string(), font, color);
        let r = egui::Rect::from_center_size(at, galley.size() + vec2(10.0, 4.0));
        self.painter.rect_filled(r, 6.0, c::panel().gamma_multiply(0.85));
        self.painter.galley(r.center() - galley.size() / 2.0, galley, color);
    }

    /// Last year's planting: a dashed outline with its name.
    fn ghost(&self, p: &Planting) {
        let col = theme::mix(c::canvas(), rgb(crop_color(&p.crop)), 0.8);
        let stroke = Stroke::new(1.5_f32, col);
        let label = format!("{} · {}", p.crop, p.year);
        let font = FontId::new(11.0, theme::medium());
        match p.layout {
            Layout::Row => {
                let pts = self.screen(&p.points);
                self.painter.extend(EShape::dashed_line(&pts, stroke, 6.0, 4.0));
                if let Some(a) = pts.first() {
                    self.painter.text(*a + vec2(0.0, -10.0), Align2::LEFT_BOTTOM, label, font, col);
                }
            }
            Layout::Patch => {
                let mut pts = self.screen(&p.points);
                pts.extend(pts.first().copied());
                self.painter.extend(EShape::dashed_line(&pts, stroke, 6.0, 4.0));
                self.painter.text(self.to(label_point(&p.points)), Align2::CENTER_CENTER, label, font, col);
            }
            Layout::Plant => {
                if let Some(cp) = p.points.first() {
                    let r = ((p.size / 2.0 * self.view.zoom as f64) as f32).max(3.0);
                    let pts: Vec<Pos2> = (0..=32)
                        .map(|i| self.to(*cp) + egui::Vec2::angled(i as f32 / 32.0 * std::f32::consts::TAU) * r)
                        .collect();
                    self.painter.extend(EShape::dashed_line(&pts, stroke, 5.0, 4.0));
                }
            }
        }
    }

    /// A ring around a neighbour of the selected planting: green for a good
    /// companion, coral for a bad one.
    fn ring(&self, p: &Planting, col: Color32) {
        let Some((lo, hi)) = p.bounds() else { return };
        let r = egui::Rect::from_two_pos(self.to(lo), self.to(hi)).expand(4.0);
        self.painter.rect_stroke(r, 6.0, Stroke::new(2.0_f32, col.gamma_multiply(0.85)), egui::StrokeKind::Outside);
    }
}
