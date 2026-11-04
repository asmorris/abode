//! Top-level app: menus, panels, dialogs, file operations.

use crate::document::Document;
use crate::textures::TextureCache;
use crate::ui::plan::{PlanState, Tool};
use crate::ui::timeline::TimelineState;
use crate::ui::view3d::View3dState;
use crate::ui::*;
use abode_core::geometry::label_point;
use abode_core::storage::FILE_EXTENSION;
use abode_core::units::parse_length;
use abode_core::*;
use chrono::{Local, Utc};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, ViewportCommand};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Settings {
    author: String,
    recent: Vec<PathBuf>,
}

pub struct AbodeApp {
    doc: Document,
    settings: Settings,
    selection: Option<EntityRef>,
    prev_selection: Option<EntityRef>,
    active_level: LevelId,
    tab: MainTab,
    plan: PlanState,
    view3d: View3dState,
    timeline: TimelineState,
    textures: TextureCache,
    toasts: Toasts,
    dialogs: Dialogs,
    actions: Vec<Action>,
    title: String,
    allow_close: bool,
    temp: Option<tempfile::TempDir>,
}

fn menu_item(ui: &mut egui::Ui, actions: &mut Vec<Action>, text: &str, shortcut: &str, a: Action) {
    if ui.add(egui::Button::new(text).shortcut_text(shortcut)).clicked() {
        actions.push(a);
        ui.close_menu();
    }
}

fn blank_project() -> Project {
    Project::new("My House")
}

impl AbodeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        let mut toasts = Toasts::default();
        let sample = initial.as_deref().is_some_and(|p| p == Path::new("--sample"));
        let initial = if sample { None } else { initial };
        let start = if sample { abode_core::sample::sample_house() } else { blank_project() };
        let doc = initial
            .and_then(|p| match Document::open(&p) {
                Ok(d) => Some(d),
                Err(e) => {
                    toasts.error(format!("Couldn't open {}: {e:#}", p.display()));
                    None
                }
            })
            .unwrap_or_else(|| Document::new_with(start).expect("couldn't create a temporary working file"));
        let active_level = doc.project.levels[0].id;
        let dialogs = Dialogs { settings: settings.author.trim().is_empty(), ..Default::default() };
        let mut app = Self {
            doc,
            settings,
            selection: None,
            prev_selection: None,
            active_level,
            tab: MainTab::Plan,
            plan: PlanState::default(),
            view3d: View3dState::default(),
            timeline: TimelineState::default(),
            textures: TextureCache::default(),
            toasts,
            dialogs,
            actions: Vec::new(),
            title: String::new(),
            allow_close: false,
            temp: None,
        };
        if let Some(p) = app.doc.path.clone() {
            app.remember_recent(&p);
        }
        app
    }

    fn author(&self) -> String {
        let a = self.settings.author.trim();
        if a.is_empty() { "Me".into() } else { a.to_string() }
    }

    fn replace_doc(&mut self, doc: Document) {
        self.doc = doc;
        self.selection = None;
        self.prev_selection = None;
        self.active_level = self.doc.project.levels[0].id;
        self.textures.clear();
        self.plan = PlanState::default();
        self.view3d = View3dState::default();
        self.timeline = TimelineState::default();
        if let Some(p) = self.doc.path.clone() {
            self.remember_recent(&p);
        }
    }

    fn remember_recent(&mut self, p: &Path) {
        self.settings.recent.retain(|x| x != p);
        self.settings.recent.insert(0, p.to_path_buf());
        self.settings.recent.truncate(8);
    }

    // ---- file operations ---------------------------------------------------

    fn guard(&mut self, ctx: &egui::Context, pending: Pending) {
        let author = self.author();
        self.doc.commit(&author);
        if self.doc.is_dirty() {
            self.dialogs.unsaved = Some(pending);
        } else {
            self.proceed(ctx, pending);
        }
    }

    fn proceed(&mut self, ctx: &egui::Context, pending: Pending) {
        match pending {
            Pending::New => self.new_doc(blank_project()),
            Pending::NewSample => self.new_doc(abode_core::sample::sample_house()),
            Pending::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Abode house file", &[FILE_EXTENSION])
                    .pick_file()
                {
                    self.open_path(&path);
                }
            }
            Pending::OpenPath(p) => self.open_path(&p),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn new_doc(&mut self, project: Project) {
        match Document::new_with(project) {
            Ok(d) => self.replace_doc(d),
            Err(e) => self.toasts.error(format!("Couldn't create a new house: {e:#}")),
        }
    }

    fn open_path(&mut self, path: &Path) {
        match Document::open(path) {
            Ok(d) => {
                self.replace_doc(d);
                self.toasts.info(format!("Opened {}", path.display()));
            }
            Err(e) => {
                if !path.exists() {
                    self.settings.recent.retain(|p| p != path);
                }
                self.toasts.error(format!("Couldn't open {}: {e:#}", path.display()));
            }
        }
    }

    /// Returns true if saved.
    fn save(&mut self, as_new: bool, message: &str) -> bool {
        let path = match (&self.doc.path, as_new) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new()
                .add_filter("Abode house file", &[FILE_EXTENSION])
                .set_file_name(format!("{}.{FILE_EXTENSION}", self.doc.display_name()))
                .save_file()
                .map(|p| if p.extension().is_none() { p.with_extension(FILE_EXTENSION) } else { p }),
        };
        let Some(path) = path else { return false };
        let author = self.author();
        match self.doc.save(&path, &author, message) {
            Ok(()) => {
                self.remember_recent(&path);
                self.timeline.revisions = None;
                self.toasts.info(format!("Saved {}", path.display()));
                true
            }
            Err(e) => {
                self.toasts.error(format!("Couldn't save: {e:#}"));
                false
            }
        }
    }

    fn export_shared(&mut self) {
        let name = format!("{} (for new owner).{FILE_EXTENSION}", self.doc.display_name());
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Abode house file", &[FILE_EXTENSION])
            .set_file_name(name)
            .save_file()
        else {
            return;
        };
        let author = self.author();
        match self.doc.export_shared(&path, &author) {
            Ok(()) => self.toasts.info(format!("Exported a shareable copy to {}", path.display())),
            Err(e) => self.toasts.error(format!("Couldn't export: {e:#}")),
        }
    }

    fn attach_files(&mut self, target: AttachTarget) {
        let Some(files) = rfd::FileDialog::new().set_title("Attach files").pick_files() else { return };
        let mut ids = Vec::new();
        for f in files {
            let name = f
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            match std::fs::read(&f).map_err(anyhow::Error::from).and_then(|b| self.doc.store.add_attachment(&name, &b)) {
                Ok(m) => ids.push(m.id),
                Err(e) => self.toasts.error(format!("Couldn't attach {name}: {e:#}")),
            }
        }
        let p = &mut self.doc.project;
        match target {
            AttachTarget::Note(id) => {
                if let Some(n) = p.notes.iter_mut().find(|n| n.id == id) {
                    n.attachments.extend(ids);
                    n.updated = Utc::now();
                }
            }
            AttachTarget::Service(id) => {
                if let Some(s) = p.service_records.iter_mut().find(|s| s.id == id) {
                    s.attachments.extend(ids);
                }
            }
        }
    }

    fn open_attachment(&mut self, id: AttachmentId) {
        let result = (|| -> anyhow::Result<()> {
            let meta = self
                .doc
                .store
                .attachment_meta(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("file is missing"))?;
            let bytes = self.doc.store.read_attachment(id)?;
            if self.temp.is_none() {
                self.temp = Some(tempfile::Builder::new().prefix("abode-files-").tempdir()?);
            }
            let dir = self.temp.as_ref().unwrap().path().join(id.to_string());
            std::fs::create_dir_all(&dir)?;
            let safe: String = meta
                .filename
                .chars()
                .map(|c| if matches!(c, '/' | '\\' | ':') { '_' } else { c })
                .collect();
            let path = dir.join(safe);
            std::fs::write(&path, bytes)?;
            open::that(&path)?;
            Ok(())
        })();
        if let Err(e) = result {
            self.toasts.error(format!("Couldn't open the file: {e:#}"));
        }
    }

    fn save_attachment_as(&mut self, id: AttachmentId) {
        let Some(meta) = self.doc.store.attachment_meta(id).cloned() else { return };
        let Some(path) = rfd::FileDialog::new().set_file_name(&meta.filename).save_file() else { return };
        match self.doc.store.read_attachment(id).and_then(|b| Ok(std::fs::write(&path, b)?)) {
            Ok(()) => self.toasts.info(format!("Saved {}", path.display())),
            Err(e) => self.toasts.error(format!("Couldn't save the file: {e:#}")),
        }
    }

    fn import_underlay(&mut self, level: LevelId) {
        let exts = abode_core::blueprint::supported_extensions();
        let Some(path) = rfd::FileDialog::new().add_filter("Blueprint image", &exts).pick_file() else { return };
        let Some(importer) = abode_core::blueprint::importer_for(&path) else {
            self.toasts.error("That file type isn't supported yet. Try a PNG or JPEG.");
            return;
        };
        let result = importer
            .import(&path)
            .and_then(|bp| Ok((self.doc.store.add_attachment(&bp.filename, &bp.bytes)?, bp.pixel_size)));
        match result {
            Ok((meta, size)) => {
                if let Some(l) = self.doc.project.level_mut(level) {
                    l.underlay = Some(Underlay::new(meta.id, size));
                }
                self.active_level = level;
                self.tab = MainTab::Plan;
                self.plan.fit_request = true;
                self.plan.set_tool(Tool::Calibrate);
                self.toasts.info("Blueprint added. Now calibrate it: click two points you know the real distance between.");
            }
            Err(e) => self.toasts.error(format!("Couldn't import the blueprint: {e:#}")),
        }
    }

    fn entity_position(&self, r: EntityRef) -> Option<Vec2> {
        let p = &self.doc.project;
        let l = &p.levels[p.level_index_of(r)?];
        match r {
            EntityRef::Room(id) => Some(label_point(&l.room_points(l.rooms.get(&id)?))),
            EntityRef::Wall(id) => l.wall_points(l.walls.get(&id)?).map(|(a, b)| a.lerp(b, 0.5)),
            EntityRef::Opening(id) => l.opening_segment(l.openings.get(&id)?).map(|(s, e, _)| s.lerp(e, 0.5)),
            EntityRef::Object(id) => Some(l.objects.get(&id)?.position),
            _ => None,
        }
    }

    fn run(&mut self, ctx: &egui::Context, action: Action) {
        let author = self.author();
        match action {
            Action::New => self.guard(ctx, Pending::New),
            Action::NewSample => self.guard(ctx, Pending::NewSample),
            Action::Open => self.guard(ctx, Pending::Open),
            Action::OpenPath(p) => self.guard(ctx, Pending::OpenPath(p)),
            Action::Quit => self.guard(ctx, Pending::Quit),
            Action::Save => {
                self.save(false, "Saved");
            }
            Action::SaveAs => {
                self.save(true, "Saved");
            }
            Action::ExportShared => self.dialogs.export_info = true,
            Action::Undo => self.doc.undo(&author),
            Action::Redo => self.doc.redo(&author),
            Action::AttachFiles(t) => self.attach_files(t),
            Action::OpenAttachment(id) => self.open_attachment(id),
            Action::SaveAttachmentAs(id) => self.save_attachment_as(id),
            Action::ImportUnderlay(l) => self.import_underlay(l),
            Action::Delete(r) => {
                if self.doc.project.delete_entity(r) {
                    if self.selection == Some(r) {
                        self.selection = None;
                    }
                } else {
                    self.toasts.info("That can't be deleted on its own.");
                }
            }
            Action::RestoreRevision(id) => match self.doc.store.load_revision(id) {
                Ok(snapshot) => {
                    let label = self
                        .timeline
                        .revisions
                        .as_ref()
                        .and_then(|r| r.iter().find(|r| r.id == id))
                        .map(|r| crate::ui::widgets::fmt_datetime(r.created))
                        .unwrap_or_else(|| "an earlier save".into());
                    self.doc.restore(snapshot, &label, &author);
                    self.selection = None;
                    if self.doc.project.level(self.active_level).is_none() {
                        self.active_level = self.doc.project.levels[0].id;
                    }
                    self.toasts.info("Restored. Undo (⌘Z) if that wasn't what you wanted.");
                }
                Err(e) => self.toasts.error(format!("Couldn't load that version: {e:#}")),
            },
            Action::Reveal(r) => {
                self.selection = Some(r);
                if let Some(li) = self.doc.project.level_index_of(r) {
                    self.active_level = self.doc.project.levels[li].id;
                    if let Some(p) = self.entity_position(r) {
                        self.plan.reveal(p);
                    }
                    if !matches!(self.tab, MainTab::Plan | MainTab::View3d) {
                        self.tab = MainTab::Plan;
                    }
                }
            }
        }
    }

    // ---- frame pieces ------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m, k| KeyboardShortcut::new(m, k);
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let typing = ctx.wants_keyboard_input();
        ctx.input_mut(|i| {
            if i.consume_shortcut(&sc(cmd_shift, Key::S)) {
                self.actions.push(Action::SaveAs);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::S)) {
                self.actions.push(Action::Save);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::O)) {
                self.actions.push(Action::Open);
            }
            if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::N)) {
                self.actions.push(Action::New);
            }
            if !typing {
                if i.consume_shortcut(&sc(cmd_shift, Key::Z)) || i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Y)) {
                    self.actions.push(Action::Redo);
                }
                if i.consume_shortcut(&sc(Modifiers::COMMAND, Key::Z)) {
                    self.actions.push(Action::Undo);
                }
            }
        });
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let recent = self.settings.recent.clone();
        let (can_undo, can_redo) = (self.doc.can_undo(), self.doc.can_redo());
        let actions = &mut self.actions;
        let dialogs = &mut self.dialogs;
        let tab = &mut self.tab;
        let plan = &mut self.plan;
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                menu_item(ui, actions, "New house", "⌘N", Action::New);
                menu_item(ui, actions, "New sample house", "", Action::NewSample);
                menu_item(ui, actions, "Open…", "⌘O", Action::Open);
                ui.menu_button("Open recent", |ui| {
                    if recent.is_empty() {
                        ui.weak("Nothing yet");
                    }
                    for p in &recent {
                        if ui.button(p.display().to_string()).clicked() {
                            actions.push(Action::OpenPath(p.clone()));
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                menu_item(ui, actions, "Save", "⌘S", Action::Save);
                menu_item(ui, actions, "Save as…", "⇧⌘S", Action::SaveAs);
                if ui.button("Save with a note…").clicked() {
                    dialogs.save_message = Some(String::new());
                    ui.close_menu();
                }
                ui.separator();
                menu_item(ui, actions, "Export copy for a new owner…", "", Action::ExportShared);
                ui.separator();
                if ui.button("Settings…").clicked() {
                    dialogs.settings = true;
                    ui.close_menu();
                }
                menu_item(ui, actions, "Quit", "⌘Q", Action::Quit);
            });
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(can_undo, egui::Button::new("Undo").shortcut_text("⌘Z")).clicked() {
                    actions.push(Action::Undo);
                    ui.close_menu();
                }
                if ui.add_enabled(can_redo, egui::Button::new("Redo").shortcut_text("⇧⌘Z")).clicked() {
                    actions.push(Action::Redo);
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                for (t, label) in [
                    (MainTab::Plan, "Floor plan"),
                    (MainTab::View3d, "3D"),
                    (MainTab::Timeline, "Timeline"),
                    (MainTab::Reminders, "Reminders"),
                ] {
                    if ui.selectable_label(*tab == t, label).clicked() {
                        *tab = t;
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Fit plan to window").clicked() {
                    plan.fit_request = true;
                    *tab = MainTab::Plan;
                    ui.close_menu();
                }
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match &self.doc.path {
                Some(p) => ui.weak(p.display().to_string()),
                None => ui.weak("Not saved yet"),
            };
            if self.doc.is_dirty() {
                ui.label("• Unsaved changes");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(format!("{} · editing as {}", self.doc.project.info.units.label(), self.author()));
            });
        });
    }

    fn modal(ctx: &egui::Context, title: &str, add: impl FnOnce(&mut egui::Ui)) {
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                add(ui);
            });
    }

    fn dialogs_ui(&mut self, ctx: &egui::Context) {
        // First-run / settings
        if self.dialogs.settings {
            let mut close = false;
            let author = &mut self.settings.author;
            Self::modal(ctx, "Settings", |ui| {
                ui.label("Your name. It's shown next to changes in the timeline, which helps when a contractor or the next owner adds to the file.");
                ui.add(egui::TextEdit::singleline(author).hint_text("e.g. Andrew"));
                ui.add_space(6.0);
                if ui.button("Done").clicked() {
                    close = true;
                }
            });
            if close {
                self.dialogs.settings = false;
            }
        }

        // Unlock room shape
        if let Some(rid) = self.dialogs.unlock_room {
            let mut decision = None;
            Self::modal(ctx, "Unlock room shape?", |ui| {
                ui.label("Room shapes rarely change unless you renovate, so they're locked once drawn.");
                ui.label("Unlocking lets you drag corners and change dimensions. Rooms that share walls with this one will move too. Changes are recorded in the timeline.");
                ui.horizontal(|ui| {
                    if ui.button("Unlock").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if let Some(d) = decision {
                if d {
                    self.doc.set_room_locked(rid, false);
                    self.prev_selection = self.selection;
                }
                self.dialogs.unlock_room = None;
            }
        }

        // Delete confirmation
        if let Some(target) = self.dialogs.delete {
            let name = self.doc.project.entity_name(target);
            let mut decision = None;
            Self::modal(ctx, "Delete?", |ui| {
                ui.label(format!("Delete “{name}”?"));
                ui.weak(match target {
                    EntityRef::Room(_) => "Its items, doors, windows, notes and history go too. You can undo this.",
                    EntityRef::Level(_) => "Everything on this level goes too. You can undo this.",
                    _ => "You can undo this.",
                });
                ui.horizontal(|ui| {
                    if crate::ui::widgets::danger_button(ui, "Delete") {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if let Some(d) = decision {
                if d {
                    self.actions.push(Action::Delete(target));
                }
                self.dialogs.delete = None;
            }
        }

        // Unsaved changes
        if let Some(pending) = self.dialogs.unsaved.clone() {
            let name = self.doc.display_name();
            let mut decision = None;
            Self::modal(ctx, "Unsaved changes", |ui| {
                ui.label(format!("Save changes to “{name}” first?"));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        decision = Some(0);
                    }
                    if ui.button("Don't save").clicked() {
                        decision = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(2);
                    }
                });
            });
            match decision {
                Some(0) => {
                    self.dialogs.unsaved = None;
                    if self.save(false, "Saved") {
                        self.proceed(ctx, pending);
                    }
                }
                Some(1) => {
                    self.dialogs.unsaved = None;
                    self.proceed(ctx, pending);
                }
                Some(_) => self.dialogs.unsaved = None,
                None => {}
            }
        }

        // Export explanation
        if self.dialogs.export_info {
            let mut decision = None;
            Self::modal(ctx, "Export a copy for a new owner", |ui| {
                ui.label("This saves a separate copy of the house file you can give to a buyer, tenant or contractor.");
                ui.label("Removed from the copy:");
                ui.label("  • notes, service records and custom fields marked 🔒 private");
                ui.label("  • files attached only to private notes");
                ui.label("  • saved versions (restore points)");
                ui.label("Your own file isn't changed.");
                ui.horizontal(|ui| {
                    if ui.button("Choose where to save…").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if let Some(d) = decision {
                self.dialogs.export_info = false;
                if d {
                    self.export_shared();
                }
            }
        }

        // Save with message
        if let Some(mut msg) = self.dialogs.save_message.take() {
            let mut decision = None;
            Self::modal(ctx, "Save with a note", |ui| {
                ui.label("Describe what changed. It appears in Saved versions on the Timeline.");
                let r = ui.add(egui::TextEdit::singleline(&mut msg).hint_text("e.g. Added basement and furnace details"));
                r.request_focus();
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            match decision {
                Some(true) => {
                    let m = if msg.trim().is_empty() { "Saved".to_string() } else { msg.trim().to_string() };
                    self.save(false, &m);
                }
                Some(false) => {}
                None => self.dialogs.save_message = Some(msg),
            }
        }

        // Restore a saved version
        if let Some((id, label)) = self.dialogs.restore.clone() {
            let mut decision = None;
            Self::modal(ctx, "Restore saved version?", |ui| {
                ui.label(format!("Go back to the version saved {label}?"));
                ui.weak("The timeline is kept, and you can undo this.");
                ui.horizontal(|ui| {
                    if ui.button("Restore").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if let Some(d) = decision {
                self.dialogs.restore = None;
                if d {
                    self.actions.push(Action::RestoreRevision(id));
                }
            }
        }

        // Blueprint calibration
        let mut close_calibration = false;
        if let Some(cal) = &mut self.dialogs.calibrate {
            let units = self.doc.project.info.units;
            let mut decision = None;
            Self::modal(ctx, "Calibrate blueprint", |ui| {
                ui.label("How far apart are the two points you clicked, in real life?");
                let r = ui.add(egui::TextEdit::singleline(&mut cal.text).hint_text("e.g. 12' 6\" or 3.8 m"));
                r.request_focus();
                if let Some(e) = &cal.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        decision = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            match decision {
                Some(true) => match parse_length(&cal.text, units) {
                    Ok(d) if d > 0.0 => {
                        let ok = self
                            .doc
                            .project
                            .level_mut(cal.level)
                            .and_then(|l| l.underlay.as_mut())
                            .is_some_and(|u| u.calibrate(cal.a, cal.b, d));
                        if ok {
                            self.plan.fit_request = true;
                            self.toasts.info("Blueprint scaled. Use ✋ Move blueprint to line it up, then draw rooms over it.");
                        } else {
                            self.toasts.error("Those points are too close together. Try again.");
                        }
                        close_calibration = true;
                    }
                    Ok(_) => cal.error = Some("Enter a distance greater than zero.".into()),
                    Err(e) => cal.error = Some(e.to_string()),
                },
                Some(false) => close_calibration = true,
                None => {}
            }
        }
        if close_calibration {
            self.dialogs.calibrate = None;
        }
    }

    /// Rooms lock again once you move on from them.
    fn auto_lock(&mut self) {
        if self.selection == self.prev_selection {
            return;
        }
        if let Some(EntityRef::Room(r)) = self.prev_selection {
            if self.dialogs.unlock_room != Some(r) {
                self.doc.set_room_locked(r, true);
            }
        }
        self.prev_selection = self.selection;
    }
}

impl AbodeApp {
    /// Debug aid: `ABODE_SCREENSHOT=out.png [ABODE_TAB=plan|3d|timeline|reminders]
    /// [ABODE_SELECT=<room name>]` renders a few frames, saves a screenshot, and exits.
    fn debug_screenshot(&mut self, ctx: &egui::Context) {
        let Ok(path) = std::env::var("ABODE_SCREENSHOT") else { return };
        let frame = ctx.cumulative_pass_nr();
        if frame == 1 {
            self.dialogs.settings = false;
            self.tab = match std::env::var("ABODE_TAB").as_deref() {
                Ok("3d") => MainTab::View3d,
                Ok("timeline") => MainTab::Timeline,
                Ok("reminders") => MainTab::Reminders,
                _ => MainTab::Plan,
            };
            if let Ok(name) = std::env::var("ABODE_SELECT") {
                let p = &self.doc.project;
                let found = p
                    .levels
                    .iter()
                    .flat_map(|l| {
                        l.rooms
                            .values()
                            .filter(|r| r.name == name)
                            .map(|r| EntityRef::Room(r.id))
                            .chain(l.objects.values().filter(|o| o.label == name).map(|o| EntityRef::Object(o.id)))
                    })
                    .next();
                self.selection = found;
                self.prev_selection = found;
            }
        }
        if frame == 20 {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                let _ = buf.save(&path);
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

impl eframe::App for AbodeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let today = Local::now().date_naive();
        if cfg!(debug_assertions) {
            self.debug_screenshot(ctx);
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            let author = self.author();
            self.doc.commit(&author);
            if self.doc.is_dirty() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.dialogs.unsaved = Some(Pending::Quit);
            }
        }
        self.shortcuts(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));

        {
            let author = self.author();
            let AbodeApp {
                doc,
                textures,
                selection,
                active_level,
                actions,
                toasts,
                dialogs,
                tab,
                plan,
                view3d,
                timeline,
                ..
            } = self;
            let project = &mut doc.project;
            let mut env = Env {
                store: &doc.store,
                textures,
                selection,
                active_level,
                actions,
                toasts,
                dialogs,
                today,
            };
            egui::SidePanel::left("nav")
                .resizable(true)
                .default_width(230.0)
                .show(ctx, |ui| nav::nav_ui(ui, project, &mut env, tab));
            egui::SidePanel::right("inspector")
                .resizable(true)
                .default_width(370.0)
                .min_width(300.0)
                .show(ctx, |ui| inspector::inspector_ui(ui, project, &mut env));
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (t, label) in [
                        (MainTab::Plan, "🗺 Floor plan"),
                        (MainTab::View3d, "🏠 3D"),
                        (MainTab::Timeline, "🕘 Timeline"),
                        (MainTab::Reminders, "⏰ Reminders"),
                    ] {
                        ui.selectable_value(tab, t, label);
                    }
                });
                ui.separator();
                match *tab {
                    MainTab::Plan => plan::plan_ui(ui, project, &mut env, plan),
                    MainTab::View3d => view3d::view3d_ui(ui, project, &mut env, view3d),
                    MainTab::Timeline => timeline::timeline_ui(ui, project, &mut env, timeline, &author),
                    MainTab::Reminders => crate::ui::reminders::reminders_ui(ui, project, &mut env),
                }
            });
        }

        self.dialogs_ui(ctx);
        self.toasts.show(ctx);
        for a in std::mem::take(&mut self.actions) {
            self.run(ctx, a);
        }
        self.auto_lock();

        // Commit once edits settle: not mid-typing, not mid-drag.
        if !ctx.wants_keyboard_input() && !ctx.input(|i| i.pointer.any_down()) {
            let author = self.author();
            self.doc.commit(&author);
        }

        let title = format!(
            "{}{} — Abode",
            if self.doc.is_dirty() { "• " } else { "" },
            self.doc.display_name()
        );
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }
}
