//! UI building blocks. Each panel gets `&mut Project` plus an [`Env`] with
//! everything else it may need (selection, file store, textures...). Things
//! that need the file system or dialogs are queued as [`Action`]s and run by
//! the app after the frame's UI is built.

pub mod garden;
pub mod hardware;
pub mod inspector;
pub mod nav;
pub mod notes;
pub mod pets;
pub mod plan;
pub mod reminders;
pub mod timeline;
pub mod vehicle;
pub mod view3d;
pub mod welcome;
pub mod widgets;

use crate::textures::TextureCache;
use crate::theme::{self, c, icon};
use abode_core::storage::Store;
use abode_core::*;
use chrono::NaiveDate;
use eframe::egui;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainTab {
    Plan,
    View3d,
    Timeline,
    Reminders,
    Vehicles,
    Pets,
}

impl MainTab {
    /// A full-width page over the canvas (not the plan or 3D view).
    pub fn is_sheet(self) -> bool {
        matches!(self, MainTab::Timeline | MainTab::Reminders | MainTab::Vehicles | MainTab::Pets)
    }
}

/// Space taken by floating panels around the canvas.
#[derive(Clone, Copy, Debug, Default)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    /// False while a sheet (timeline, reminders) covers the canvas.
    pub overlays: bool,
}

impl Insets {
    pub fn apply(self, r: egui::Rect) -> egui::Rect {
        egui::Rect::from_min_max(
            egui::pos2(r.left() + self.left, r.top() + self.top),
            egui::pos2((r.right() - self.right).max(r.left() + self.left + 100.0), (r.bottom() - self.bottom).max(r.top() + self.top + 100.0)),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachTarget {
    Note(NoteId),
    Service(ServiceId),
    /// A vehicle's documents (registration, insurance card, manual...).
    Vehicle(ObjectId),
    /// A pet's photo (the first file picked).
    PetPhoto(PetId),
    PetDocs(PetId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    New,
    NewSample,
    Open,
    OpenPath(PathBuf),
    Save,
    SaveAs,
    ExportShared,
    /// Export as Markdown pages, plans, files and JSON.
    ExportDocuments,
    Undo,
    Redo,
    Quit,
    AttachFiles(AttachTarget),
    OpenAttachment(AttachmentId),
    SaveAttachmentAs(AttachmentId),
    ImportUnderlay(LevelId),
    Delete(EntityRef),
    RestoreRevision(RevisionId),
    /// Select something and show it on the plan.
    Reveal(EntityRef),
    /// Show a floor's roof layer on the plan.
    EditRoof(LevelId),
    /// Open a garden bed's garden plan.
    OpenGarden(FeatureId),
    /// Add a floor to the house with this name, and go to it.
    AddFloor(String),
    /// Back to the start screen.
    Home,
    CheckUpdates,
    InstallUpdate(crate::updater::Release),
}

/// Something that needs "you have unsaved changes" confirmation first.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    New,
    NewSample,
    Open,
    OpenPath(PathBuf),
    Quit,
    Home,
    Update(crate::updater::Release),
}

pub struct CalibrateDialog {
    pub level: LevelId,
    pub a: Vec2,
    pub b: Vec2,
    pub text: String,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Dialogs {
    pub unlock_room: Option<RoomId>,
    pub calibrate: Option<CalibrateDialog>,
    pub unsaved: Option<Pending>,
    pub export_info: bool,
    /// The "Export as documents" dialog: Some(include private).
    pub export_docs: Option<bool>,
    pub settings: bool,
    /// Choosing which sections the sidebar shows.
    pub sidebar: bool,
    pub save_message: Option<String>,
    pub restore: Option<(RevisionId, String)>,
    pub delete: Option<EntityRef>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Error,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<(String, ToastKind, Instant)>,
}

impl Toasts {
    pub fn info(&mut self, msg: impl Into<String>) {
        self.items.push((msg.into(), ToastKind::Info, Instant::now() + Duration::from_secs(4)));
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.items.push((msg.into(), ToastKind::Error, Instant::now() + Duration::from_secs(7)));
    }

    pub fn show(&mut self, ctx: &egui::Context, bottom_center: egui::Pos2) {
        let now = Instant::now();
        self.items.retain(|(_, _, until)| *until > now);
        if self.items.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(bottom_center)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (msg, kind, _) in &self.items {
                    egui::Frame::new()
                        .fill(c::toast())
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .shadow(theme::shadow())
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.horizontal(|ui| {
                                let (i, col) = match kind {
                                    ToastKind::Info => (icon::CHECK_CIRCLE, egui::Color32::from_rgb(140, 214, 196)),
                                    ToastKind::Error => (icon::WARNING_CIRCLE, c::coral()),
                                };
                                ui.label(egui::RichText::new(i).size(16.0).color(col));
                                ui.label(egui::RichText::new(msg).color(egui::Color32::WHITE));
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

pub struct Env<'a> {
    pub store: &'a Store,
    pub textures: &'a mut TextureCache,
    pub selection: &'a mut Option<EntityRef>,
    pub active_level: &'a mut LevelId,
    pub actions: &'a mut Vec<Action>,
    pub toasts: &'a mut Toasts,
    pub dialogs: &'a mut Dialogs,
    pub today: NaiveDate,
    /// The garden bed open in the garden plan, if any.
    pub garden_open: Option<FeatureId>,
}

impl Env<'_> {
    pub fn select(&mut self, r: EntityRef) {
        *self.selection = Some(r);
    }
}
