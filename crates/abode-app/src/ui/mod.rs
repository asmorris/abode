//! UI building blocks. Each panel gets `&mut Project` plus an [`Env`] with
//! everything else it may need (selection, file store, textures...). Things
//! that need the file system or dialogs are queued as [`Action`]s and run by
//! the app after the frame's UI is built.

pub mod inspector;
pub mod nav;
pub mod notes;
pub mod plan;
pub mod reminders;
pub mod timeline;
pub mod view3d;
pub mod widgets;

use crate::textures::TextureCache;
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachTarget {
    Note(NoteId),
    Service(ServiceId),
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
}

/// Something that needs "you have unsaved changes" confirmation first.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    New,
    NewSample,
    Open,
    OpenPath(PathBuf),
    Quit,
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
    pub settings: bool,
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

    pub fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.items.retain(|(_, _, until)| *until > now);
        if self.items.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -40.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (msg, kind, _) in &self.items {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(360.0);
                        let text = egui::RichText::new(msg);
                        match kind {
                            ToastKind::Info => ui.label(text),
                            ToastKind::Error => ui.label(text.color(ui.visuals().error_fg_color)),
                        };
                    });
                    ui.add_space(4.0);
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
}

impl Env<'_> {
    pub fn select(&mut self, r: EntityRef) {
        *self.selection = Some(r);
    }
}
