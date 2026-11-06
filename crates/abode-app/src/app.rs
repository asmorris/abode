//! Top-level app: menus, panels, dialogs, file operations.

use crate::document::Document;
use crate::textures::TextureCache;
use crate::theme::{self, c, icon};
use crate::ui::plan::{Layer, PlanState, Tool};
use crate::ui::timeline::TimelineState;
use crate::ui::view3d::View3dState;
use crate::ui::welcome::{self, HouseEntry, WelcomeAction};
use crate::ui::*;
use crate::updater::{Status as UpdateStatus, Updater};
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
    appearance: theme::Appearance,
    /// Don't look for updates on launch.
    no_auto_update: bool,
    /// Houses removed from the start screen's list.
    hidden: Vec<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Welcome,
    Editor,
}

pub struct AbodeApp {
    screen: Screen,
    /// Saved houses for the start screen (rescanned when it's shown).
    houses: Option<Vec<HouseEntry>>,
    updater: Updater,
    /// The person asked to check, so say what we found.
    update_by_user: bool,
    /// Started with `--update-now`: install as soon as an update is found.
    install_when_found: bool,
    logo: Option<egui::TextureHandle>,
    doc: Document,
    settings: Settings,
    selection: Option<EntityRef>,
    prev_selection: Option<EntityRef>,
    active_level: LevelId,
    tab: MainTab,
    /// What the canvas shows (plan or 3D) under any open sheet.
    last_canvas: MainTab,
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

/// Attachment types that are safe to open with the system's default app.
/// Anything else (executables, scripts, installers, HTML...) can only be saved.
const OPENABLE_EXTENSIONS: &[&str] = &[
    "pdf", "png", "jpg", "jpeg", "gif", "webp", "bmp", "heic", "tif", "tiff", "txt", "md", "csv", "rtf", "doc",
    "docx", "xls", "xlsx", "odt", "ods", "mp4", "mov", "m4v", "mp3", "m4a", "wav",
];

/// A filename that can't escape its directory or be empty/special.
fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '\0') || c.is_control() { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "attachment".into()
    } else {
        cleaned
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
        theme::install(&cc.egui_ctx, settings.appearance);
        let mut toasts = Toasts::default();
        let sample = initial.as_deref().is_some_and(|p| p == Path::new("--sample"));
        // `--update-now`: check and install the latest release straight away.
        let update_now = initial.as_deref().is_some_and(|p| p == Path::new("--update-now"));
        let initial = if sample || update_now { None } else { initial };
        let mut screen = if sample || initial.is_some() { Screen::Editor } else { Screen::Welcome };
        let start = if sample { abode_core::sample::sample_house() } else { blank_project() };
        let doc = initial
            .and_then(|p| match Document::open(&p) {
                Ok(d) => Some(d),
                Err(e) => {
                    toasts.error(format!("Couldn't open {}: {e:#}", p.display()));
                    screen = Screen::Welcome;
                    None
                }
            })
            .unwrap_or_else(|| Document::new_with(start).expect("couldn't create a temporary working file"));
        let active_level = doc.project.first_house_level();
        let dialogs = Dialogs { settings: settings.author.trim().is_empty(), ..Default::default() };
        let updater = Updater::default();
        let screenshot = std::env::var("ABODE_SCREENSHOT").is_ok();
        if (!settings.no_auto_update && !screenshot) || update_now {
            updater.check(&cc.egui_ctx);
        }
        let mut app = Self {
            screen,
            houses: None,
            updater,
            update_by_user: update_now,
            install_when_found: update_now,
            logo: None,
            doc,
            settings,
            selection: None,
            prev_selection: None,
            active_level,
            tab: MainTab::Plan,
            last_canvas: MainTab::Plan,
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
        self.screen = Screen::Editor;
        self.selection = None;
        self.prev_selection = None;
        self.active_level = self.doc.project.first_house_level();
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
                let mut dialog = rfd::FileDialog::new().set_directory(welcome::default_folder());
                // On macOS the type filter uses a deprecated API that greys out
                // `.abode` files (they have no registered type), so it's left off
                // there and the extension is checked below instead.
                if !cfg!(target_os = "macos") {
                    dialog = dialog.add_filter("Abode house file", &[FILE_EXTENSION]);
                }
                if let Some(path) = dialog.pick_file() {
                    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(FILE_EXTENSION)) {
                        self.open_path(&path);
                    } else {
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        self.toasts.error(format!("{name} isn't an Abode house file. Choose a file ending in .{FILE_EXTENSION}."));
                    }
                }
            }
            Pending::OpenPath(p) => self.open_path(&p),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Pending::Home => {
                self.new_doc(blank_project());
                self.screen = Screen::Welcome;
                self.houses = None;
            }
            Pending::Update(release) => self.updater.install(ctx, release),
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
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.toasts.info(format!("Opened {name}"));
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
                .set_directory({
                    let dir = welcome::default_folder();
                    let _ = std::fs::create_dir_all(&dir);
                    dir
                })
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
            let safe = safe_filename(&meta.filename);
            // House files get shared with strangers, so never hand the OS
            // something it might execute (.app, .command, .exe, scripts...).
            let ext = Path::new(&safe)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if !ext.as_deref().is_some_and(|e| OPENABLE_EXTENSIONS.contains(&e)) {
                anyhow::bail!(
                    "for safety, “{}” can't be opened directly. Use 💾 Save a copy… and check it first",
                    meta.filename
                );
            }
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
        if let EntityRef::Building(b) = r {
            let l = p.ground_level(Some(b))?;
            let pts: Vec<Vec2> = l.nodes.values().map(|n| n.pos).collect();
            return abode_core::geometry::bounds(&pts).map(|(lo, hi)| (lo + hi) / 2.0);
        }
        let l = &p.levels[p.level_index_of(r)?];
        match r {
            EntityRef::RoofSection(id) => Some(label_point(&l.roofs.get(&id)?.footprint())),
            EntityRef::Feature(id) => {
                let f = l.features.get(&id)?;
                match f.kind.geom() {
                    FeatureGeom::Area => Some(label_point(&f.points)),
                    _ => abode_core::geometry::point_along(&f.points, f.length() / 2.0).map(|(p, _)| p),
                }
            }
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
                        self.active_level = self.doc.project.first_house_level();
                    }
                    self.toasts.info("Restored. Undo (⌘Z) if that wasn't what you wanted.");
                }
                Err(e) => self.toasts.error(format!("Couldn't load that version: {e:#}")),
            },
            Action::Home => self.guard(ctx, Pending::Home),
            Action::CheckUpdates => {
                self.update_by_user = true;
                self.updater.check(ctx);
            }
            Action::InstallUpdate(r) => self.guard(ctx, Pending::Update(r)),
            Action::EditRoof(l) => {
                self.active_level = l;
                self.plan.layer = Layer::Roof;
                self.tab = MainTab::Plan;
                self.last_canvas = MainTab::Plan;
            }
            Action::Reveal(r) => {
                self.selection = Some(r);
                // Showing something means leaving any sheet for the canvas + inspector.
                if !matches!(self.tab, MainTab::Plan | MainTab::View3d) {
                    self.tab = self.last_canvas;
                }
                let p = &self.doc.project;
                let on_roof = match r {
                    EntityRef::RoofSection(_) => true,
                    EntityRef::Object(id) => p.find_object(id).is_some_and(|(_, o)| o.roof.is_some()),
                    _ => false,
                };
                if matches!(r, EntityRef::Level(_)) {
                    // keep the current layer
                } else if on_roof {
                    self.plan.layer = Layer::Roof;
                } else {
                    self.plan.layer = Layer::Floor;
                }
                if let EntityRef::Building(_) = r {
                    if let Some(site) = p.site_id() {
                        self.active_level = site;
                        if let Some(pos) = self.entity_position(r) {
                            self.plan.reveal(pos);
                        }
                    }
                } else if let Some(li) = self.doc.project.level_index_of(r) {
                    self.active_level = self.doc.project.levels[li].id;
                    if let Some(p) = self.entity_position(r) {
                        self.plan.reveal(p);
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

    /// A centred modal card. Returns true if dismissed (Esc / click outside).
    fn modal(ctx: &egui::Context, id: &str, i: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) -> bool {
        egui::Modal::new(egui::Id::new(id))
            .frame(theme::panel().inner_margin(egui::Margin::same(24)).corner_radius(egui::CornerRadius::same(20)))
            .backdrop_color(egui::Color32::from_black_alpha(70))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.horizontal(|ui| {
                    theme::icon_tile(ui, i, c::forest(), c::mint_soft());
                    ui.label(egui::RichText::new(title).family(theme::serif()).size(26.0).color(c::ink()));
                });
                ui.add_space(8.0);
                add(ui);
            })
            .should_close()
    }

    /// Enter pressed while no text box has the keyboard: confirms a dialog.
    fn enter_confirms(ctx: &egui::Context) -> bool {
        !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Enter))
    }

    /// Right-aligned button row. Add the primary button first.
    fn footer(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
    }

    fn dialogs_ui(&mut self, ctx: &egui::Context) {
        // First-run / settings
        if self.dialogs.settings {
            let mut close = false;
            let Settings { author, appearance, no_auto_update, .. } = &mut self.settings;
            let mut check_now = false;
            let dismissed = Self::modal(ctx, "settings", icon::GEAR_SIX, "Settings", |ui| {
                ui.label(theme::muted(
                    "Your name. It's shown next to changes in the timeline, which helps when a contractor or the next owner adds to the file.",
                ));
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(author).hint_text("Your name").desired_width(f32::INFINITY).margin(egui::vec2(10.0, 8.0)));
                ui.add_space(10.0);
                ui.label(theme::strong("Appearance"));
                let before = *appearance;
                theme::segmented(
                    ui,
                    appearance,
                    &[
                        (theme::Appearance::System, "Match system"),
                        (theme::Appearance::Light, "Light"),
                        (theme::Appearance::Dark, "Dark"),
                    ],
                );
                if *appearance != before {
                    appearance.apply(ui.ctx());
                }
                ui.add_space(10.0);
                ui.label(theme::strong("Updates"));
                ui.horizontal(|ui| {
                    let mut auto = !*no_auto_update;
                    if theme::toggle(ui, &mut auto, "Check for updates when Abode opens").changed() {
                        *no_auto_update = !auto;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::soft_button(ui, "Check now").clicked() {
                            check_now = true;
                        }
                    });
                });
                ui.label(theme::muted(format!("You have Abode {}.", crate::updater::VERSION)));
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, "Done").clicked() {
                        close = true;
                    }
                });
            });
            if close || dismissed {
                self.dialogs.settings = false;
            }
            if check_now {
                self.actions.push(Action::CheckUpdates);
            }
        }

        // Unlock room shape
        if let Some(rid) = self.dialogs.unlock_room {
            let mut decision = None;
            let dismissed = Self::modal(ctx, "unlock", icon::LOCK_OPEN, "Unlock room shape?", |ui| {
                ui.label("Room shapes rarely change unless you renovate, so they're locked once drawn.");
                ui.label(theme::muted(
                    "Unlocking lets you drag corners and change dimensions. Rooms that share walls with this one move too. Changes are recorded in the timeline.",
                ));
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, "Unlock").clicked() {
                        decision = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if dismissed {
                decision = Some(false);
            } else if decision.is_none() && Self::enter_confirms(ctx) {
                decision = Some(true);
            }
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
            let dismissed = Self::modal(ctx, "delete", icon::TRASH, &format!("Delete {name}?"), |ui| {
                ui.label(theme::muted(match target {
                    EntityRef::Room(_) => "Its items, doors, windows, notes and history go too. You can undo this.",
                    EntityRef::Level(_) => "Everything on this level goes too. You can undo this.",
                    EntityRef::Building(_) => "Its floors, rooms, items, notes and history go too. You can undo this.",
                    _ => "You can undo this.",
                }));
                Self::footer(ui, |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Delete").color(egui::Color32::WHITE).family(theme::medium()))
                                .fill(c::danger())
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(egui::CornerRadius::same(10))
                                .min_size(egui::vec2(0.0, 34.0)),
                        )
                        .clicked()
                    {
                        decision = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if dismissed {
                decision = Some(false);
            } else if decision.is_none() && Self::enter_confirms(ctx) {
                decision = Some(true);
            }
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
            let dismissed = Self::modal(ctx, "unsaved", icon::FLOPPY_DISK, "Unsaved changes", |ui| {
                ui.label(format!("Save changes to “{name}” first?"));
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, "Save").clicked() {
                        decision = Some(0);
                    }
                    if theme::secondary_button(ui, "Don't save").clicked() {
                        decision = Some(1);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(2);
                    }
                });
            });
            if dismissed {
                decision = Some(2);
            } else if decision.is_none() && Self::enter_confirms(ctx) {
                decision = Some(0);
            }
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
            let dismissed = Self::modal(ctx, "export", icon::EXPORT, "Share with a new owner", |ui| {
                ui.label("Saves a separate copy of the house file to give to a buyer, tenant or contractor.");
                ui.add_space(6.0);
                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(theme::caps("Left out of the copy"));
                    for line in [
                        "Notes, service records and custom fields marked private",
                        "Files attached only to private notes",
                        "Saved versions (restore points)",
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(icon::LOCK).color(c::muted()));
                            ui.label(line);
                        });
                    }
                });
                ui.label(theme::muted("Your own file isn't changed."));
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, format!("{}  Choose where to save…", icon::EXPORT)).clicked() {
                        decision = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if dismissed {
                decision = Some(false);
            } else if decision.is_none() && Self::enter_confirms(ctx) {
                decision = Some(true);
            }
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
            let dismissed = Self::modal(ctx, "save-note", icon::NOTE_PENCIL, "Save with a note", |ui| {
                ui.label(theme::muted("Describe what changed. It appears under Saved versions in the Timeline."));
                let r = ui.add(
                    egui::TextEdit::singleline(&mut msg)
                        .hint_text("e.g. Added basement and furnace details")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(10.0, 8.0)),
                );
                r.request_focus();
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    decision = Some(true);
                }
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, "Save").clicked() {
                        decision = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if dismissed {
                decision = Some(false);
            }
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
            let dismissed = Self::modal(ctx, "restore", icon::ARROW_COUNTER_CLOCKWISE, "Restore this version?", |ui| {
                ui.label(format!("Go back to the version saved {label}?"));
                ui.label(theme::muted("The timeline is kept, and you can undo this."));
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, "Restore").clicked() {
                        decision = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if dismissed {
                decision = Some(false);
            }
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
            let dismissed = Self::modal(ctx, "calibrate", icon::RULER, "Calibrate blueprint", |ui| {
                ui.label("How far apart are the two points you clicked, in real life?");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut cal.text)
                        .hint_text("e.g. 12' 6\" or 3.8 m")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(10.0, 8.0)),
                );
                r.request_focus();
                if let Some(e) = &cal.error {
                    ui.colored_label(c::coral_dark(), e);
                }
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    decision = Some(true);
                }
                Self::footer(ui, |ui| {
                    if theme::primary_button(ui, "Apply").clicked() {
                        decision = Some(true);
                    }
                    if theme::secondary_button(ui, "Cancel").clicked() {
                        decision = Some(false);
                    }
                });
            });
            if dismissed {
                decision = Some(false);
            }
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
                            self.toasts.info("Blueprint scaled. Use Move blueprint to line it up, then draw rooms over it.");
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
            if let Ok(t) = std::env::var("ABODE_REMINDERS_TAB") {
                crate::ui::reminders::set_tab(ctx, &t);
            }
            if std::env::var("ABODE_DIALOG").as_deref() == Ok("export") {
                self.dialogs.export_info = true;
            }
            if std::env::var("ABODE_SCREEN").as_deref() == Ok("welcome") {
                self.screen = Screen::Welcome;
            }
            if std::env::var("ABODE_DARK").is_ok() {
                theme::Appearance::Dark.apply(ctx);
            }
            if let Ok(name) = std::env::var("ABODE_LEVEL") {
                // ABODE_MOVE=down,down: move that level down the stack twice.
                if let Ok(moves) = std::env::var("ABODE_MOVE") {
                    let p = &mut self.doc.project;
                    if let Some(id) = p.levels.iter().find(|l| l.name == name).map(|l| l.id) {
                        for m in moves.split(',') {
                            p.move_level(id, m.trim() == "up");
                        }
                    }
                }
                let p = &self.doc.project;
                let found = if name == "outside" { p.site_id() } else { p.levels.iter().find(|l| l.name == name).map(|l| l.id) };
                if let Some(l) = found {
                    self.active_level = l;
                }
            }
            if let Ok(v) = std::env::var("ABODE_CAMERA") {
                let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if let [yaw, pitch] = n[..] {
                    self.view3d.set_angle(yaw, pitch);
                }
            }
            if let Ok(v) = std::env::var("ABODE_3D") {
                self.view3d.show_roofs = !v.contains("noroof");
                self.view3d.show_outside = !v.contains("nooutside");
                self.view3d.show_items = !v.contains("noitems");
            }
            if std::env::var("ABODE_LAYER").as_deref() == Ok("roof") {
                self.plan.layer = Layer::Roof;
            }
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
                            .chain(l.roofs.values().filter(|r| r.name == name).map(|r| EntityRef::RoofSection(r.id)))
                            .chain(l.features.values().filter(|f| f.name == name).map(|f| EntityRef::Feature(f.id)))
                    })
                    .chain(p.buildings.iter().filter(|b| b.name == name).map(|b| EntityRef::Building(b.id)))
                    .chain(p.levels.iter().filter(|l| l.name == name).map(|l| EntityRef::Level(l.id)))
                    .next()
                    .or((name == "house").then_some(EntityRef::General))
                    .or((name == "roof").then_some(EntityRef::Roof));
                // ABODE_UNLOCK: unlock the selected room's shape. ABODE_WALL: select its first wall.
                if std::env::var("ABODE_UNITS").as_deref() == Ok("imperial") {
                    self.doc.project.info.units = abode_core::units::UnitSystem::Imperial;
                }
                let mut found = found;
                if let Some(EntityRef::Room(rid)) = found {
                    let p = &mut self.doc.project;
                    if let Some(li) = p.level_index_of(EntityRef::Room(rid)) {
                        let level = &mut p.levels[li];
                        if std::env::var("ABODE_UNLOCK").is_ok() {
                            level.rooms.get_mut(&rid).unwrap().shape_locked = false;
                        }
                        if std::env::var("ABODE_WALL").is_ok() {
                            found = level.walls_of_room(&level.rooms[&rid]).first().map(|w| EntityRef::Wall(*w));
                        }
                    }
                }
                // ABODE_CUSTOM: give the selected item a couple of custom fields.
                if let (Some(EntityRef::Object(oid)), Ok(_)) = (found, std::env::var("ABODE_CUSTOM")) {
                    for l in &mut self.doc.project.levels {
                        if let Some(o) = l.objects.get_mut(&oid) {
                            for (k, v) in [("Test", "This"), ("Filter size", "16 x 25 x 1 inch, changed every three months")] {
                                o.details.custom_fields.push(abode_core::CustomField { key: k.into(), value: v.into(), private: false });
                            }
                        }
                    }
                }
                self.selection = found;
                self.prev_selection = found;
                // ABODE_FOCUS=<section title>: open and scroll to that section.
                if let Ok(t) = std::env::var("ABODE_FOCUS") {
                    theme::focus_section(ctx, &[t.as_str()]);
                }
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

impl AbodeApp {
    fn set_title(&mut self, ctx: &egui::Context) {
        let title = match self.screen {
            Screen::Welcome => "Abode".to_string(),
            Screen::Editor => format!("{}{} — Abode", if self.doc.is_dirty() { "• " } else { "" }, self.doc.display_name()),
        };
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn welcome(&mut self, ctx: &egui::Context) {
        if self.logo.is_none() {
            if let Ok(img) = image::load_from_memory(include_bytes!("../assets/icon/icon.png")) {
                let img = img.resize(256, 256, image::imageops::FilterType::Lanczos3).to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                self.logo = Some(ctx.load_texture("abode-logo", color, egui::TextureOptions::LINEAR));
            }
        }
        if self.houses.is_none() {
            self.houses = Some(welcome::scan(&self.settings.recent, &self.settings.hidden));
        }
        let status = self.updater.status();
        let houses = self.houses.clone().unwrap_or_default();
        let mut action = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(c::canvas()))
            .show(ctx, |ui| action = welcome::welcome_ui(ui, &houses, &status, self.logo.as_ref()));
        match action {
            Some(WelcomeAction::New) => self.new_doc(blank_project()),
            Some(WelcomeAction::Sample) => self.new_doc(abode_core::sample::sample_house()),
            Some(WelcomeAction::Open) => self.proceed(ctx, Pending::Open),
            Some(WelcomeAction::OpenPath(p)) => self.open_path(&p),
            Some(WelcomeAction::Forget(p)) => {
                self.settings.recent.retain(|x| x != &p);
                self.settings.hidden.push(p);
                self.houses = None;
            }
            Some(WelcomeAction::CheckUpdates) => self.actions.push(Action::CheckUpdates),
            Some(WelcomeAction::InstallUpdate(r)) => self.actions.push(Action::InstallUpdate(r)),
            Some(WelcomeAction::DismissUpdate) => self.updater.dismiss(),
            Some(WelcomeAction::Settings) => self.dialogs.settings = true,
            None => {}
        }
        if self.screen == Screen::Editor {
            self.tab = MainTab::Plan;
            self.last_canvas = MainTab::Plan;
        }
    }

    /// Progress while an update installs; restart when it's done; errors.
    fn update_ui(&mut self, ctx: &egui::Context) {
        match self.updater.status() {
            UpdateStatus::Installing { release, progress, step } => {
                egui::Modal::new(egui::Id::new("updating"))
                    .frame(theme::panel().inner_margin(egui::Margin::same(24)).corner_radius(egui::CornerRadius::same(20)))
                    .backdrop_color(egui::Color32::from_black_alpha(70))
                    .show(ctx, |ui| {
                        ui.set_width(380.0);
                        ui.horizontal(|ui| {
                            theme::icon_tile(ui, icon::ARROW_CIRCLE_UP, c::forest(), c::mint_soft());
                            ui.label(egui::RichText::new(format!("Updating to {}", release.version)).family(theme::serif()).size(26.0).color(c::ink()));
                        });
                        ui.add_space(10.0);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 4.0, c::field());
                        let t = progress.unwrap_or_else(|| (ui.input(|i| i.time) as f32 * 0.6).fract());
                        let fill = if progress.is_some() {
                            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * t, rect.height()))
                        } else {
                            let w = rect.width() * 0.3;
                            egui::Rect::from_min_size(rect.min + egui::vec2((rect.width() - w) * t, 0.0), egui::vec2(w, rect.height()))
                        };
                        ui.painter().rect_filled(fill, 4.0, c::forest());
                        ui.add_space(6.0);
                        let pct = progress.map(|p| format!(" · {:.0}%", p * 100.0)).unwrap_or_default();
                        ui.label(theme::muted(format!("{step}{pct}. Abode will restart when it's done.")));
                    });
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            UpdateStatus::Ready => {
                self.updater.relaunch();
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            UpdateStatus::Failed { message, page } => {
                if let Some(page) = page {
                    let mut decision = None;
                    let dismissed = Self::modal(ctx, "update-failed", icon::WARNING_CIRCLE, "Update didn't finish", |ui| {
                        ui.label(&message);
                        ui.label(theme::muted("Your houses aren't affected. You can download the new version from the release page instead."));
                        Self::footer(ui, |ui| {
                            if theme::primary_button(ui, format!("{}  Open the download page", icon::ARROW_SQUARE_OUT)).clicked() {
                                decision = Some(true);
                            }
                            if theme::secondary_button(ui, "Close").clicked() {
                                decision = Some(false);
                            }
                        });
                    });
                    if decision == Some(true) {
                        let _ = open::that(&page);
                    }
                    if decision.is_some() || dismissed {
                        self.updater.dismiss();
                    }
                } else {
                    if self.update_by_user {
                        self.toasts.error(message);
                    }
                    self.update_by_user = false;
                    self.updater.dismiss();
                }
            }
            UpdateStatus::UpToDate if self.update_by_user => {
                self.toasts.info(format!("You have the latest version of Abode ({}).", crate::updater::VERSION));
                self.update_by_user = false;
            }
            UpdateStatus::Available(r) if self.install_when_found => {
                self.install_when_found = false;
                self.update_by_user = false;
                self.actions.push(Action::InstallUpdate(r));
            }
            UpdateStatus::Available(_) if self.update_by_user => self.update_by_user = false,
            _ => {}
        }
    }
}

impl eframe::App for AbodeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::sync(ctx);
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
        self.update_ui(ctx);
        if self.screen == Screen::Welcome {
            self.welcome(ctx);
            self.dialogs_ui(ctx);
            let screen = ctx.screen_rect();
            self.toasts.show(ctx, egui::pos2(screen.center().x, screen.bottom() - 40.0));
            for a in std::mem::take(&mut self.actions) {
                self.run(ctx, a);
            }
            self.set_title(ctx);
            return;
        }
        if matches!(self.tab, MainTab::Plan | MainTab::View3d) {
            self.last_canvas = self.tab;
        }

        let screen = ctx.screen_rect();
        let m = theme::MARGIN;
        let sheet_open = matches!(self.tab, MainTab::Timeline | MainTab::Reminders);
        let insets = Insets {
            left: m + theme::LEFT_W,
            // Sheets take the full width; the inspector comes back when you close them.
            right: if self.selection.is_some() && !sheet_open { m + theme::RIGHT_W } else { 0.0 },
            top: 0.0,
            bottom: 0.0,
            overlays: !sheet_open,
        };

        {
            let author = self.author();
            let (name, dirty, saved) = (self.doc.display_name(), self.doc.is_dirty(), self.doc.path.is_some());
            let (can_undo, can_redo) = (self.doc.can_undo(), self.doc.can_redo());
            let AbodeApp {
                doc,
                textures,
                selection,
                active_level,
                actions,
                toasts,
                dialogs,
                tab,
                last_canvas,
                plan,
                view3d,
                timeline,
                settings,
                updater,
                ..
            } = self;
            let update = match updater.status() {
                UpdateStatus::Available(r) => Some(r),
                _ => None,
            };
            let file = nav::FileInfo { name, dirty, saved, recent: &settings.recent, can_undo, can_redo, update };
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

            // The canvas fills the window; everything else floats above it.
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(c::canvas()))
                .show(ctx, |ui| match *last_canvas {
                    MainTab::View3d => view3d::view3d_ui(ui, project, &mut env, view3d, insets),
                    _ => plan::plan_ui(ui, project, &mut env, plan, insets),
                });

            egui::Area::new(egui::Id::new("left-panel"))
                .order(egui::Order::Middle)
                .fixed_pos(egui::pos2(m, m))
                .show(ctx, |ui| {
                    theme::panel().show(ui, |ui| {
                        ui.set_width(theme::LEFT_W - 30.0);
                        ui.set_height(screen.height() - 2.0 * m - 30.0);
                        nav::nav_ui(ui, project, &mut env, tab, last_canvas, &file);
                    });
                });

            if env.selection.is_some() && !sheet_open {
                egui::Area::new(egui::Id::new("inspector-panel"))
                    .order(egui::Order::Middle)
                    .fixed_pos(egui::pos2(screen.right() - m - theme::RIGHT_W, m))
                    .show(ctx, |ui| {
                        ui.set_max_width(theme::RIGHT_W);
                        theme::panel().show(ui, |ui| {
                            ui.set_width(theme::RIGHT_W - 30.0);
                            ui.set_height(screen.height() - 2.0 * m - 30.0);
                            inspector::inspector_ui(ui, project, &mut env);
                            theme::clear_focus(ctx);
                        });
                    });
            }

            if matches!(*tab, MainTab::Timeline | MainTab::Reminders) {
                let left = m + theme::LEFT_W + m;
                let right = screen.right() - insets.right - m;
                let width = (right - left).max(420.0);
                egui::Area::new(egui::Id::new("sheet"))
                    .order(egui::Order::Middle)
                    .fixed_pos(egui::pos2(left, m))
                    .show(ctx, |ui| {
                        theme::panel().inner_margin(egui::Margin::same(24)).show(ui, |ui| {
                            ui.set_width(width - 50.0);
                            ui.set_height(screen.height() - 2.0 * m - 50.0);
                            let (title, i) = if *tab == MainTab::Timeline {
                                ("Timeline", icon::CLOCK_COUNTER_CLOCKWISE)
                            } else {
                                ("Reminders", icon::BELL)
                            };
                            ui.horizontal(|ui| {
                                theme::icon_tile(ui, i, c::forest(), c::mint_soft());
                                ui.label(egui::RichText::new(title).family(theme::serif()).size(32.0).color(c::ink()));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if theme::icon_button(ui, icon::X, "Close", false).clicked() {
                                        *tab = *last_canvas;
                                    }
                                });
                            });
                            ui.add_space(8.0);
                            match *tab {
                                MainTab::Timeline => timeline::timeline_ui(ui, project, &mut env, timeline, &author),
                                _ => crate::ui::reminders::reminders_ui(ui, project, &mut env),
                            }
                        });
                    });
            }
        }

        self.dialogs_ui(ctx);
        let area = insets.apply(screen);
        self.toasts.show(ctx, egui::pos2(area.center().x, screen.bottom() - m - 64.0));
        for a in std::mem::take(&mut self.actions) {
            self.run(ctx, a);
        }
        self.auto_lock();

        // Commit once edits settle: not mid-typing, not mid-drag.
        if !ctx.wants_keyboard_input() && !ctx.input(|i| i.pointer.any_down()) {
            let author = self.author();
            self.doc.commit(&author);
        }

        self.set_title(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_are_sanitised() {
        assert_eq!(safe_filename("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(safe_filename(".."), "attachment");
        assert_eq!(safe_filename("manual.pdf. . "), "manual.pdf");
        assert_eq!(safe_filename(""), "attachment");
    }

    #[test]
    fn executables_are_not_openable() {
        for bad in ["exe", "app", "command", "sh", "bat", "html", "js", "dmg", "pkg", "scpt"] {
            assert!(!OPENABLE_EXTENSIONS.contains(&bad), "{bad}");
        }
    }
}
