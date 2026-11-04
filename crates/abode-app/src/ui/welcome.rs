//! The start screen: start a new house, try the sample, or pick a saved one.

use crate::theme::{self, c, icon};
use crate::updater::{Release, Status, VERSION};
use abode_core::storage;
use chrono::{DateTime, Local, Utc};
use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Ui, vec2};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where new houses are saved by default: `Documents/Abode`.
pub fn default_folder() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Abode")
}

#[derive(Clone, Debug)]
pub struct HouseEntry {
    pub path: PathBuf,
    pub name: String,
    pub detail: String,
    pub saved: Option<DateTime<Utc>>,
    pub readable: bool,
}

/// Saved houses: recently opened files plus everything in the default folder.
pub fn scan(recent: &[PathBuf], hidden: &[PathBuf]) -> Vec<HouseEntry> {
    let mut paths: Vec<PathBuf> = recent.iter().filter(|p| p.exists()).cloned().collect();
    if let Ok(dir) = std::fs::read_dir(default_folder()) {
        for e in dir.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == storage::FILE_EXTENSION) {
                paths.push(p);
            }
        }
    }
    let key = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let hidden: Vec<PathBuf> = hidden.iter().map(|p| key(p)).collect();
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for p in paths {
        let k = key(&p);
        if seen.contains(&k) || hidden.contains(&k) {
            continue;
        }
        seen.push(k);
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let folder = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
        let modified = std::fs::metadata(&p)
            .and_then(|m| m.modified())
            .ok()
            .map(|t: SystemTime| DateTime::<Utc>::from(t));
        let entry = match storage::peek(&p) {
            Ok(s) => HouseEntry {
                name: if s.name.trim().is_empty() { stem } else { s.name },
                detail: if s.address.trim().is_empty() { folder } else { s.address },
                saved: s.saved_at.or(modified),
                path: p,
                readable: true,
            },
            Err(_) => HouseEntry { name: stem, detail: "Couldn't read this file".into(), saved: modified, path: p, readable: false },
        };
        out.push(entry);
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.saved));
    out
}

pub enum WelcomeAction {
    New,
    Sample,
    Open,
    OpenPath(PathBuf),
    Forget(PathBuf),
    CheckUpdates,
    InstallUpdate(Release),
    DismissUpdate,
    Settings,
}

/// A big clickable choice card.
fn choice_card(ui: &mut Ui, width: f32, i: &str, title: &str, body: &str, accent: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 156.0), Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();
    p.rect(
        rect,
        CornerRadius::same(18),
        c::panel(),
        Stroke::new(if hovered { 1.5 } else { 1.0_f32 }, if hovered { c::forest() } else { c::border() }),
        egui::StrokeKind::Inside,
    );
    let tile = Rect::from_min_size(rect.min + vec2(20.0, 20.0), vec2(44.0, 44.0));
    let (fg, bg) = if accent { (c::coral(), c::coral_soft()) } else { (c::forest(), c::mint_soft()) };
    p.rect_filled(tile, 12.0, bg);
    p.text(tile.center(), Align2::CENTER_CENTER, i, FontId::proportional(22.0), fg);
    p.text(rect.min + vec2(20.0, 82.0), Align2::LEFT_TOP, title, FontId::new(17.0, theme::semibold()), c::ink());
    let galley = p.layout(body.to_string(), FontId::proportional(13.5), c::muted(), width - 40.0);
    p.galley(rect.min + vec2(20.0, 108.0), galley, c::muted());
    if hovered {
        p.text(rect.right_top() + vec2(-22.0, 32.0), Align2::RIGHT_CENTER, icon::ARROW_RIGHT, FontId::proportional(18.0), c::forest());
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn house_row(ui: &mut Ui, h: &HouseEntry, action: &mut Option<WelcomeAction>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::click());
    let hovered = resp.hovered() || ui.rect_contains_pointer(rect);
    let p = ui.painter();
    if hovered {
        p.rect_filled(rect, 12.0, c::hover());
    }
    let tile = Rect::from_min_size(rect.left_center() + vec2(12.0, -19.0), vec2(38.0, 38.0));
    p.rect_filled(tile, 11.0, c::coral_soft());
    p.text(tile.center(), Align2::CENTER_CENTER, icon::HOUSE_LINE, FontId::proportional(19.0), c::coral());
    let x = tile.right() + 14.0;
    p.text(egui::pos2(x, rect.center().y - 9.0), Align2::LEFT_CENTER, &h.name, FontId::new(15.0, theme::semibold()), c::ink());
    let detail_color = if h.readable { c::muted() } else { c::coral_dark() };
    let max_w = rect.right() - x - 190.0;
    let mut job = egui::text::LayoutJob::simple_singleline(h.detail.clone(), FontId::proportional(12.5), detail_color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(60.0));
    let g = ui.fonts(|f| f.layout_job(job));
    p.galley(egui::pos2(x, rect.center().y + 3.0), g, detail_color);

    let forget = Rect::from_center_size(rect.right_center() - vec2(26.0, 0.0), vec2(28.0, 28.0));
    if hovered {
        let fr = ui.interact(forget, resp.id.with("forget"), Sense::click()).on_hover_text("Remove from this list (the file isn't deleted)");
        if fr.hovered() {
            ui.painter().rect_filled(forget, 8.0, c::border());
        }
        ui.painter().text(forget.center(), Align2::CENTER_CENTER, icon::X, FontId::proportional(14.0), c::muted());
        if fr.clicked() {
            *action = Some(WelcomeAction::Forget(h.path.clone()));
            return;
        }
    }
    if let Some(saved) = h.saved {
        let when = saved.with_timezone(&Local).format("%b %-d, %Y").to_string();
        ui.painter().text(
            rect.right_center() - vec2(52.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("Saved {when}"),
            FontId::proportional(12.5),
            c::faint(),
        );
    }
    if resp.on_hover_text(h.path.display().to_string()).clicked() && h.readable {
        *action = Some(WelcomeAction::OpenPath(h.path.clone()));
    }
}

fn update_banner(ui: &mut Ui, status: &Status, action: &mut Option<WelcomeAction>) {
    let Status::Available(r) = status else { return };
    theme::card().fill(c::mint_soft()).stroke(Stroke::new(1.0_f32, c::mint())).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::icon_tile(ui, icon::ARROW_CIRCLE_UP, c::forest(), c::panel());
            ui.vertical(|ui| {
                ui.label(theme::strong(format!("Abode {} is available", r.version)));
                let notes: String = r.notes.lines().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().join(" · ");
                let text = if notes.is_empty() { format!("You have {VERSION}.") } else { notes };
                ui.add(egui::Label::new(theme::muted(text)).truncate());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::primary_button(ui, format!("{}  Update", icon::DOWNLOAD_SIMPLE)).clicked() {
                    *action = Some(WelcomeAction::InstallUpdate(r.clone()));
                }
                if theme::secondary_button(ui, "Later").clicked() {
                    *action = Some(WelcomeAction::DismissUpdate);
                }
            });
        });
    });
    ui.add_space(18.0);
}

pub fn welcome_ui(ui: &mut Ui, houses: &[HouseEntry], status: &Status, logo: Option<&egui::TextureHandle>) -> Option<WelcomeAction> {
    let mut action = None;
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, c::canvas());
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let width = (ui.available_width() - 64.0).clamp(320.0, 780.0);
        let side = ((ui.available_width() - width) / 2.0).max(0.0);
        ui.horizontal(|ui| {
            ui.add_space(side);
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.add_space((ui.ctx().screen_rect().height() * 0.08).clamp(24.0, 90.0));

                // Header
                ui.horizontal(|ui| {
                    if let Some(tex) = logo {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(vec2(64.0, 64.0)));
                    } else {
                        theme::icon_tile(ui, icon::HOUSE_LINE, c::coral(), c::coral_soft());
                    }
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Abode").family(theme::serif()).size(44.0).color(c::ink()));
                        ui.label(theme::muted("The owner's manual for your home."));
                    });
                });
                ui.add_space(26.0);

                update_banner(ui, status, &mut action);

                // Start
                let gap = 16.0;
                let card_w = (width - gap) / 2.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    if choice_card(ui, card_w, icon::PENCIL_SIMPLE_LINE, "Start from scratch", "Draw your rooms, or trace over a photo of your floor plan.", false).clicked() {
                        action = Some(WelcomeAction::New);
                    }
                    if choice_card(ui, card_w, icon::SPARKLE, "Explore the sample house", "A furnished example with roofs, a deck, a fence and a shed.", true).clicked() {
                        action = Some(WelcomeAction::Sample);
                    }
                });
                ui.add_space(30.0);

                // Saved houses
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Your houses").family(theme::serif()).size(26.0).color(c::ink()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::secondary_button(ui, format!("{}  Open a file…", icon::FOLDER_OPEN)).clicked() {
                            action = Some(WelcomeAction::Open);
                        }
                    });
                });
                ui.add_space(6.0);
                theme::panel().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if houses.is_empty() {
                        ui.add_space(14.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(icon::HOUSE_SIMPLE).size(26.0).color(c::faint()));
                            ui.label(theme::muted("Houses you save will show up here."));
                            ui.label(
                                RichText::new(format!("They're kept in {}", default_folder().display()))
                                    .size(12.5)
                                    .color(c::faint()),
                            );
                        });
                        ui.add_space(14.0);
                    }
                    for (i, h) in houses.iter().enumerate() {
                        if i > 0 {
                            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                            ui.painter().line_segment([r.left_center() + vec2(64.0, 0.0), r.right_center()], Stroke::new(1.0_f32, c::border()));
                        }
                        house_row(ui, h, &mut action);
                    }
                });

                // Footer
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(RichText::new(format!("Abode {VERSION}")).size(12.5).color(c::faint()));
                    ui.label(RichText::new("·").size(12.5).color(c::faint()));
                    let checking = matches!(status, Status::Checking | Status::Installing { .. });
                    let label = match status {
                        Status::Checking => "Checking for updates…".to_string(),
                        Status::UpToDate => "Up to date".to_string(),
                        _ => "Check for updates".to_string(),
                    };
                    let link = ui.add_enabled(
                        !checking,
                        egui::Button::new(RichText::new(label).size(12.5).color(c::forest())).frame(false),
                    );
                    if link.clicked() {
                        action = Some(WelcomeAction::CheckUpdates);
                    }
                    ui.label(RichText::new("·").size(12.5).color(c::faint()));
                    if ui.add(egui::Button::new(RichText::new("Settings").size(12.5).color(c::forest())).frame(false)).clicked() {
                        action = Some(WelcomeAction::Settings);
                    }
                });
                ui.add_space(30.0);
            });
        });
    });
    action
}
