//! Export the house as plain files anyone can open without Abode: a PDF
//! house manual, Markdown pages, SVG plans, the attached files, and all the
//! data as JSON.
//!
//! The pages are built as [`doc::Page`]s first, and both the Markdown and
//! the PDF are made from them, so they always say the same thing.

pub mod doc;
pub mod pdf;
pub mod svg;

use crate::garden::{Layout, Planting};
use crate::geometry;
use crate::ids::*;
use crate::model::*;
use crate::roof::format_pitch;
use crate::site::{Feature, FeatureGeom, FeatureKind};
use crate::units::{UnitSystem, format_area, format_length};
use crate::vehicle::{Fuel, Vehicle, format_distance};
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use doc::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Everything, private notes too (a copy for yourself), or a copy for
    /// someone else with private things left out.
    pub include_private: bool,
    pub today: NaiveDate,
}

/// One file of the export, at a path relative to the export folder.
#[derive(Clone, Debug)]
pub struct ExportFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// An attached file's details and contents.
pub type ReadAttachment<'a> = &'a dyn Fn(AttachmentId) -> Option<(AttachmentMeta, Vec<u8>)>;

/// The PDF's name in the export folder.
pub const MANUAL: &str = "House manual.pdf";

/// Build every file of the export.
pub fn export(project: &Project, opts: Options, read: ReadAttachment) -> Result<Vec<ExportFile>, String> {
    let p = if opts.include_private { project.clone() } else { project.sanitized_for_sharing() };
    let mut out = Vec::new();

    // Attached files, under names people can recognise.
    let mut files = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for id in p.referenced_attachments() {
        let Some((meta, bytes)) = read(id) else { continue };
        let path = unique(&mut taken, "files", &file_name(&meta.filename));
        out.push(ExportFile { path: path.clone(), bytes });
        files.insert(id, (path, meta));
    }

    let mut cx = Cx { p: &p, units: p.info.units, opts, files, plans: Vec::new(), slugs: BTreeSet::new() };
    let pages = cx.pages();
    let drawings: BTreeMap<String, svg::Drawing> = std::mem::take(&mut cx.plans).into_iter().collect();
    for (path, d) in &drawings {
        out.push(ExportFile { path: path.clone(), bytes: d.to_svg().into_bytes() });
    }
    for page in &pages {
        out.push(ExportFile { path: page.path.clone(), bytes: to_markdown(page).into_bytes() });
    }
    out.push(ExportFile { path: "house.json".into(), bytes: cx.json() });
    let photos: BTreeMap<String, Vec<u8>> = out
        .iter()
        .filter(|f| cx.files.values().any(|(path, meta)| *path == f.path && meta.is_image()))
        .map(|f| (f.path.clone(), f.bytes.clone()))
        .collect();
    let title = p.entity_name(EntityRef::General);
    let settings = pdf::Settings { title: &title, letter: p.info.units == UnitSystem::Imperial };
    out.push(ExportFile { path: MANUAL.into(), bytes: pdf::render(&pages, &drawings, &photos, settings)? });
    Ok(out)
}

/// The export folder's name: "Maple House (for a new owner)".
pub fn folder_name(project: &Project, include_private: bool) -> String {
    let name = file_name(&project.entity_name(EntityRef::General));
    if include_private { format!("{name} (everything)") } else { format!("{name} (for a new owner)") }
}

/// A file name that's safe on every system.
fn file_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if c.is_control() || r#"/\:*?"<>|"#.contains(c) { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    if cleaned.is_empty() { "file".into() } else { cleaned }
}

/// `dir/name`, numbered if that's taken ("photo (2).jpg").
fn unique(taken: &mut BTreeSet<String>, dir: &str, name: &str) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let mut n = 1;
    loop {
        let candidate = if n == 1 { format!("{dir}/{name}") } else { format!("{dir}/{stem} ({n}){ext}") };
        if taken.insert(candidate.to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

/// A short lowercase name for a page or plan file ("main-floor").
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "page".into() } else { out }
}

fn date(d: NaiveDate) -> String {
    d.format("%b %-d, %Y").to_string()
}

fn opt_date(d: Option<NaiveDate>) -> String {
    d.map(date).unwrap_or_default()
}

fn yes(b: bool) -> String {
    if b { "Yes".into() } else { String::new() }
}

struct Cx<'a> {
    p: &'a Project,
    units: UnitSystem,
    opts: Options,
    /// Attachment → (path in the export, details).
    files: BTreeMap<AttachmentId, (String, AttachmentMeta)>,
    /// Plan drawings: (path of the SVG, drawing).
    plans: Vec<(String, svg::Drawing)>,
    slugs: BTreeSet<String>,
}

/// A page we'll write, with what goes in the contents list.
struct Planned {
    path: String,
    title: String,
    blurb: String,
}

impl Cx<'_> {
    fn len(&self, m: f64) -> String {
        format_length(m, self.units)
    }

    fn area(&self, m2: f64) -> String {
        format_area(m2, self.units)
    }

    fn page_path(&mut self, name: &str) -> String {
        let base = slug(name);
        let mut s = base.clone();
        let mut n = 2;
        while !self.slugs.insert(s.clone()) {
            s = format!("{base}-{n}");
            n += 1;
        }
        format!("pages/{s}.md")
    }

    /// Save a plan drawing and return the block that shows it.
    fn plan(&mut self, name: &str, drawing: Option<svg::Drawing>, alt: &str) -> Option<Block> {
        let svg = drawing?;
        let path = format!("plans/{}.svg", slug(name));
        let path = if self.plans.iter().any(|(p, _)| *p == path) { format!("plans/{}-{}.svg", slug(name), self.plans.len()) } else { path };
        self.plans.push((path.clone(), svg));
        Some(Block::Image { src: path, alt: alt.to_string() })
    }

    fn pages(&mut self) -> Vec<Page> {
        let p = self.p;
        let mut pages = Vec::new();
        let mut contents: Vec<Planned> = Vec::new();
        let add = |pages: &mut Vec<Page>, contents: &mut Vec<Planned>, page: Page, blurb: String| {
            contents.push(Planned { path: page.path.clone(), title: page.title.clone(), blurb });
            pages.push(page);
        };

        for l in p.floor_stack(None).into_iter().rev() {
            let page = self.floor_page(l);
            let n = l.rooms.len();
            add(&mut pages, &mut contents, page, format!("{n} room{}", if n == 1 { "" } else { "s" }));
        }
        for b in &p.buildings {
            let page = self.building_page(b);
            let kind = b.kind.trim();
            let blurb = if kind.is_empty() || kind.eq_ignore_ascii_case(&b.display_name()) { "Outbuilding".into() } else { kind.to_string() };
            add(&mut pages, &mut contents, page, blurb);
        }
        if p.site().is_some_and(|s| !s.features.is_empty() || !s.objects.is_empty()) {
            let page = self.outside_page();
            add(&mut pages, &mut contents, page, "The yard, decks, fences, trees and more".into());
        }
        let gardens: Vec<&Feature> = p.site().map(|s| s.features.values().filter(|f| f.kind == FeatureKind::GardenBed).collect()).unwrap_or_default();
        for g in gardens {
            if p.garden_years(g.id).is_empty() {
                continue;
            }
            let page = self.garden_page(g);
            add(&mut pages, &mut contents, page, "What was planted where, and when".into());
        }
        if !p.seeds.is_empty() {
            let page = self.seeds_page();
            add(&mut pages, &mut contents, page, "Seed packets".into());
        }
        let has_roof = p.levels.iter().any(|l| !l.roofs.is_empty()) || !p.roof.material.trim().is_empty() || p.roof.installed.is_some();
        if has_roof {
            let page = self.roof_page();
            add(&mut pages, &mut contents, page, "Roof sections, layers and who installed them".into());
        }
        if !p.paints.is_empty() {
            let page = self.paint_page();
            add(&mut pages, &mut contents, page, "Every paint colour, and where the leftovers are".into());
        }
        if !p.all_hardware().is_empty() {
            let page = self.hardware_page();
            add(&mut pages, &mut contents, page, "Pulls, hinges, handles and fittings, and where they are".into());
        }
        if !p.vehicles().is_empty() {
            let page = self.vehicles_page();
            add(&mut pages, &mut contents, page, "Cars and other vehicles".into());
        }
        if !p.pets.is_empty() {
            let page = self.pets_page();
            let n = p.pets.len();
            add(&mut pages, &mut contents, page, format!("{n} pet{}: vet, vaccines, medications and care", if n == 1 { "" } else { "s" }));
        }
        if !p.reminders.is_empty() {
            let page = self.reminders_page();
            add(&mut pages, &mut contents, page, "Upcoming maintenance".into());
        }
        if !p.service_records.is_empty() || !p.changelog.is_empty() {
            let page = self.history_page();
            add(&mut pages, &mut contents, page, "Work done on the house, and changes over time".into());
        }
        let readme = self.readme(&contents);
        pages.insert(0, readme);
        pages
    }

    // -----------------------------------------------------------------------
    // Shared pieces

    fn custom(&self, fields: &[CustomField]) -> Vec<(String, String)> {
        fields
            .iter()
            .filter(|f| !f.value.trim().is_empty() || !f.key.trim().is_empty())
            .map(|f| {
                let key = if f.key.trim().is_empty() { "Field".to_string() } else { f.key.trim().to_string() };
                let key = if f.private { format!("{key} (private)") } else { key };
                (key, f.value.clone())
            })
            .collect()
    }

    /// "Label: value" fields plus custom fields, with blanks dropped.
    fn fields_with(&self, pairs: Vec<(&str, String)>, custom: &[CustomField]) -> Option<Block> {
        let mut v: Vec<(String, String)> = pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        v.extend(self.custom(custom));
        v.retain(|(_, v)| !v.trim().is_empty());
        (!v.is_empty()).then(|| Block::Fields(v.into_iter().map(|(k, v)| (k, v.trim().to_string())).collect()))
    }

    fn details(&self, d: &ItemDetails) -> Vec<(&'static str, String)> {
        vec![
            ("Make", d.make.clone()),
            ("Model", d.model.clone()),
            ("Serial number", d.serial.clone()),
            ("Purchased", opt_date(d.purchased)),
            ("Installed", opt_date(d.installed)),
            ("Price", d.price.clone()),
            ("Bought from", d.vendor.clone()),
            ("Warranty until", opt_date(d.warranty_expires)),
            ("Warranty details", d.warranty_details.clone()),
        ]
    }

    fn attachments(&self, ids: &[AttachmentId]) -> Vec<Block> {
        let mut out = Vec::new();
        let mut others = Vec::new();
        for id in ids {
            let Some((path, meta)) = self.files.get(id) else { continue };
            if meta.is_image() {
                out.push(Block::Image { src: path.clone(), alt: meta.filename.clone() });
            } else {
                others.push(vec![link(meta.filename.clone(), path.clone())]);
            }
        }
        if !others.is_empty() {
            out.push(Block::Bullets(others));
        }
        out
    }

    /// Notes, service history and reminders about one thing.
    fn extras(&self, r: EntityRef, h: u8) -> Vec<Block> {
        let p = self.p;
        let mut out = Vec::new();
        let notes: Vec<&Note> = p.notes_for(r).collect();
        if !notes.is_empty() {
            out.push(Block::Heading(h, "Notes".into()));
            for n in notes {
                let title = if n.title.trim().is_empty() { "Note".to_string() } else { n.title.trim().to_string() };
                let mut line = vec![Inline::Bold(title), text(format!(" · {}", date(n.updated.with_timezone(&Local).date_naive())))];
                if n.private {
                    line.push(Inline::Italic(" (private)".into()));
                }
                out.push(Block::Para(line));
                if !n.body.trim().is_empty() {
                    out.push(Block::Lines(n.body.clone()));
                }
                out.extend(self.attachments(&n.attachments));
            }
        }
        let mut service: Vec<&ServiceRecord> = p.service_for(r).collect();
        if !service.is_empty() {
            service.sort_by_key(|s| std::cmp::Reverse(s.date));
            let title = if matches!(r, EntityRef::Pet(_)) { "Vet visits and care" } else { "Service history" };
            out.push(Block::Heading(h, title.into()));
            out.push(self.service_table(&service, false));
            for s in service {
                if !s.attachments.is_empty() {
                    out.push(Block::Para(vec![Inline::Italic(format!("Files for {} on {}:", s.kind_label().to_lowercase(), date(s.date)))]));
                    out.extend(self.attachments(&s.attachments));
                }
            }
        }
        let reminders: Vec<&Reminder> = p.reminders_for(r).filter(|r| !r.is_done()).collect();
        if !reminders.is_empty() {
            out.push(Block::Heading(h, "Reminders".into()));
            out.push(self.reminder_table(&reminders, false));
        }
        out
    }

    fn service_table(&self, records: &[&ServiceRecord], with_target: bool) -> Block {
        let mut head = vec!["Date".to_string()];
        if with_target {
            head.push("For".into());
        }
        head.extend(["Type", "Done by", "Cost", "What was done"].map(String::from));
        let rows = records
            .iter()
            .map(|s| {
                let mut row = vec![date(s.date)];
                if with_target {
                    row.push(self.p.entity_name(s.target));
                }
                let mut what = s.description.trim().to_string();
                if let Some(km) = s.odometer {
                    what = format!("{what} (at {})", format_distance(km, self.units)).trim().to_string();
                }
                if s.private {
                    what = format!("{what} (private)").trim().to_string();
                }
                row.extend([s.kind_label(), s.performed_by.clone(), s.cost.clone(), what]);
                row
            })
            .collect();
        Block::Table { head, rows }
    }

    fn reminder_due(&self, r: &Reminder) -> String {
        let mut due = Vec::new();
        if r.due_by.uses_date() {
            due.push(date(r.effective_due()));
        }
        if let (true, Some(km)) = (r.due_by.uses_distance(), r.due_km) {
            due.push(format!("at {}", format_distance(km, self.units)));
        }
        due.join(" or ")
    }

    fn reminder_repeat(&self, r: &Reminder) -> String {
        let mut rep = Vec::new();
        if let (true, Some(x)) = (r.due_by.uses_date(), r.repeat) {
            rep.push(x.describe());
        }
        if let (true, Some(km)) = (r.due_by.uses_distance(), r.every_km) {
            rep.push(format!("every {}", format_distance(km, self.units)));
        }
        rep.join(" or ")
    }

    fn reminder_table(&self, reminders: &[&Reminder], with_target: bool) -> Block {
        let mut head = vec!["Reminder".to_string()];
        if with_target {
            head.push("For".into());
        }
        head.extend(["Due", "Repeats", "Notes"].map(String::from));
        let rows = reminders
            .iter()
            .map(|r| {
                let mut row = vec![if r.title.trim().is_empty() { "Reminder".into() } else { r.title.trim().to_string() }];
                if with_target {
                    row.push(self.p.entity_name(r.target));
                }
                row.extend([self.reminder_due(r), self.reminder_repeat(r), r.notes.clone()]);
                row
            })
            .collect();
        Block::Table { head, rows }
    }

    fn paint_line(&self, paint: &Paint) -> String {
        let surface = custom_label(paint.surface == PaintSurface::Other, &paint.surface_name, paint.surface.label());
        let finish = custom_label(paint.finish == PaintFinish::Other, &paint.finish_name, paint.finish.label());
        let colour = [paint.brand.trim(), paint.color_name.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
        let mut s = format!("{surface}: {}", if colour.is_empty() { "paint" } else { &colour });
        if !paint.color_code.trim().is_empty() {
            s.push_str(&format!(" ({})", paint.color_code.trim()));
        }
        s.push_str(&format!(", {}", finish.to_lowercase()));
        if let Some(d) = paint.applied {
            s.push_str(&format!(", painted {}", date(d)));
        }
        if !paint.leftover_location.trim().is_empty() {
            s.push_str(&format!(". Leftover: {}", paint.leftover_location.trim()));
        }
        s
    }

    /// "Paint" and a line for each paint matching `f`.
    fn paints_where(&self, f: impl Fn(&PaintTarget) -> bool) -> Vec<Block> {
        let lines: Vec<Vec<Inline>> = self.p.paints.iter().filter(|x| f(&x.target)).map(|x| vec![text(self.paint_line(x))]).collect();
        if lines.is_empty() {
            return Vec::new();
        }
        vec![Block::Para(vec![Inline::Bold("Paint".into())]), Block::Bullets(lines)]
    }

    /// An item: what it is, its details, and its history.
    fn item(&self, level: &Level, o: &PlacedObject, h: u8) -> Vec<Block> {
        let mut out = vec![Block::Heading(h, o.display_name())];
        let kind = if o.is_stairs() { "Stairs".to_string() } else { o.category_label() };
        let mut pairs = vec![("Type", kind), ("Size", format!("{} × {}", self.len(o.shape.width), self.len(o.shape.depth)))];
        if let Some(info) = self.p.stair_info(level.id, o) {
            let to = info.to.map(|l| self.p.entity_name(EntityRef::Level(l))).unwrap_or_default();
            let dir = o.stairs.as_ref().map(|s| s.direction.label().to_lowercase()).unwrap_or_default();
            pairs.push(("Stairs", format!("{} steps {dir} {}", info.steps, if to.is_empty() { String::new() } else { format!("to {to}") }).trim().to_string()));
        }
        pairs.extend(self.details(&o.details));
        out.extend(self.fields_with(pairs, &o.details.custom_fields));
        out.extend(hardware_table(&o.details.hardware));
        out.extend(self.extras(EntityRef::Object(o.id), h + 1));
        out
    }

    fn opening(&self, level: &Level, o: &Opening, h: u8) -> Vec<Block> {
        let name = self.p.entity_name(EntityRef::Opening(o.id));
        let mut out = vec![Block::Heading(h, name)];
        let mut pairs = vec![("Style", o.style_label()), ("Size", format!("{} × {}", self.len(o.width), self.len(o.height)))];
        if !o.kind.is_door() && o.sill_height > 0.0 {
            pairs.push(("Sill height", self.len(o.sill_height)));
        }
        pairs.extend(self.details(&o.details));
        out.extend(self.fields_with(pairs, &o.details.custom_fields));
        out.extend(hardware_table(&o.details.hardware));
        out.extend(self.extras(EntityRef::Opening(o.id), h + 1));
        let _ = level;
        out
    }

    /// Whether there's anything to say about a thing beyond its basics.
    fn has_more(&self, r: EntityRef, d: Option<&ItemDetails>, custom: &[CustomField]) -> bool {
        let p = self.p;
        let details = d.is_some_and(|d| {
            !(d.make.trim().is_empty()
                && d.model.trim().is_empty()
                && d.serial.trim().is_empty()
                && d.purchased.is_none()
                && d.installed.is_none()
                && d.price.trim().is_empty()
                && d.vendor.trim().is_empty()
                && d.warranty_expires.is_none()
                && d.warranty_details.trim().is_empty()
                && d.custom_fields.is_empty()
                && d.hardware.is_empty())
        });
        details
            || !custom.is_empty()
            || p.notes_for(r).next().is_some()
            || p.service_for(r).next().is_some()
            || p.reminders_for(r).any(|r| !r.is_done())
    }

    // -----------------------------------------------------------------------
    // Pages

    fn readme(&mut self, contents: &[Planned]) -> Page {
        let p = self.p;
        let name = p.entity_name(EntityRef::General);
        let mut page = Page::new("README.md", name.clone());
        let (floor_area, _) = p.house_floor_area();
        let pairs = vec![
            ("Address", p.info.address.clone()),
            ("Built", p.info.year_built.map(|y| y.to_string()).unwrap_or_default()),
            ("Floor area", if floor_area > 0.0 { self.area(floor_area) } else { String::new() }),
        ];
        page.extend(self.fields_with(pairs, &p.info.custom_fields));
        let mut about = format!("Exported from Abode on {}. ", date(self.opts.today));
        if self.opts.include_private {
            about.push_str("This copy has everything, including notes and details marked private. Keep it somewhere safe.");
        } else {
            about.push_str("Anything marked private was left out, along with vehicles, pets and seed packets, which leave with their owner.");
        }
        page.push(para(about));
        if !contents.is_empty() {
            page.push(Block::Heading(2, "Contents".into()));
            page.push(Block::Bullets(
                contents
                    .iter()
                    .map(|c| {
                        let mut line = vec![link(c.title.clone(), c.path.clone())];
                        if !c.blurb.is_empty() {
                            line.push(text(format!(" · {}", c.blurb)));
                        }
                        line
                    })
                    .collect(),
            ));
        }
        page.extend(self.extras(EntityRef::General, 2));
        if !self.files.is_empty() {
            page.push(Block::Heading(2, "Files".into()));
            page.push(para("Receipts, manuals, photos and other files that were attached, in the files folder:"));
            let mut list: Vec<(&String, &AttachmentMeta)> = self.files.values().map(|(p, m)| (p, m)).collect();
            list.sort_by_key(|(p, _)| p.to_lowercase());
            page.push(Block::Bullets(list.into_iter().map(|(path, m)| vec![link(m.filename.clone(), path.clone())]).collect()));
        }
        page.push(Block::Heading(2, "About these files".into()));
        page.push(Block::Bullets(vec![
            vec![link(MANUAL, MANUAL), text(" has all of this in one document, for reading on screen or printing.")],
            vec![text("The pages are Markdown: plain text you can open in any text editor, or read formatted in a Markdown viewer.")],
            vec![text("The plans folder has drawings of each floor, the yard and the roof, as SVG images that open in any web browser.")],
            vec![text("house.json has all the information in a form other software can read.")],
            vec![text("To keep using Abode, open the .abode house file instead; this export is a copy for reading.")],
        ]));
        page
    }

    fn floor_blocks(&mut self, l: &Level, h: u8) -> Vec<Block> {
        let p = self.p;
        let title = p.entity_name(EntityRef::Level(l.id));
        let mut out = Vec::new();
        out.extend(self.plan(&title, svg::floor(l, &title, self.units), &format!("Plan of {title}")));
        let area: f64 = l.rooms.values().map(|r| geometry::area(&l.room_outline(r))).sum();
        let pairs = vec![
            ("Floor area", if area > 0.0 { self.area(area) } else { String::new() }),
            ("Ceiling height", self.len(l.wall_height)),
        ];
        out.extend(fields(pairs));
        out.extend(self.extras(EntityRef::Level(l.id), h));

        let mut rooms: Vec<&Room> = l.rooms.values().collect();
        rooms.sort_by_key(|r| r.display_name().to_lowercase());
        if !rooms.is_empty() {
            out.push(Block::Heading(h, "Rooms".into()));
        }
        for room in rooms {
            out.push(Block::Heading(h + 1, room.display_name()));
            let poly = l.room_outline(room);
            let size = l.rect_dims(room).map(|(_, w, d)| format!("{} × {}", self.len(w), self.len(d))).unwrap_or_default();
            let pairs = vec![
                ("Type", room.kind.clone()),
                ("Area", self.area(geometry::area(&poly))),
                ("Size", size),
                ("Ceiling height", room.ceiling_height.map(|c| self.len(c)).unwrap_or_default()),
                ("Floor", room.floor_finish.clone()),
            ];
            out.extend(self.fields_with(pairs, &room.custom_fields));
            out.extend(hardware_table(&room.hardware));
            out.extend(self.paints_where(|t| matches!(t, PaintTarget::Room(r) if *r == room.id) || matches!(t, PaintTarget::WallFace { room: Some(r), .. } if *r == room.id)));
            out.extend(self.extras(EntityRef::Room(room.id), h + 2));
            let mut items: Vec<&PlacedObject> = l.objects.values().filter(|o| o.room == Some(room.id) && o.vehicle.is_none() && o.roof.is_none()).collect();
            items.sort_by_key(|o| o.display_name().to_lowercase());
            for o in items {
                out.extend(self.item(l, o, h + 2));
            }
        }

        let mut loose: Vec<&PlacedObject> = l.objects.values().filter(|o| o.room.is_none() && o.roof.is_none() && o.feature.is_none() && o.vehicle.is_none()).collect();
        if !loose.is_empty() {
            loose.sort_by_key(|o| o.display_name().to_lowercase());
            out.push(Block::Heading(h, "Other items".into()));
            for o in loose {
                out.extend(self.item(l, o, h + 1));
            }
        }
        out.extend(self.vehicles_here(l, h));

        let mut openings: Vec<&Opening> = l.openings.values().collect();
        if !openings.is_empty() {
            openings.sort_by_key(|o| (!o.kind.is_door(), p.entity_name(EntityRef::Opening(o.id)).to_lowercase()));
            out.push(Block::Heading(h, "Doors and windows".into()));
            out.push(Block::Table {
                head: ["Name", "Style", "Width", "Height"].map(String::from).to_vec(),
                rows: openings.iter().map(|o| vec![p.entity_name(EntityRef::Opening(o.id)), o.style_label(), self.len(o.width), self.len(o.height)]).collect(),
            });
            for o in openings {
                if self.has_more(EntityRef::Opening(o.id), Some(&o.details), &[]) {
                    out.extend(self.opening(l, o, h + 1));
                }
            }
        }

        let mut walls: Vec<&Wall> = l
            .walls
            .values()
            .filter(|w| {
                !w.label.trim().is_empty() || self.has_more(EntityRef::Wall(w.id), None, &w.custom_fields) || p.paints.iter().any(|x| matches!(x.target, PaintTarget::WallFace { wall, .. } if wall == w.id))
            })
            .collect();
        if !walls.is_empty() {
            walls.sort_by_key(|w| p.entity_name(EntityRef::Wall(w.id)).to_lowercase());
            out.push(Block::Heading(h, "Walls".into()));
            for w in walls {
                out.push(Block::Heading(h + 1, p.entity_name(EntityRef::Wall(w.id))));
                out.extend(self.fields_with(vec![("Length", self.len(l.wall_length(w))), ("Thickness", self.len(w.thickness))], &w.custom_fields));
                out.extend(self.paints_where(|t| matches!(t, PaintTarget::WallFace { wall, .. } if *wall == w.id)));
                out.extend(self.extras(EntityRef::Wall(w.id), h + 2));
            }
        }
        out
    }

    fn vehicles_here(&self, l: &Level, h: u8) -> Vec<Block> {
        let cars: Vec<&PlacedObject> = l.objects.values().filter(|o| o.vehicle.is_some()).collect();
        if cars.is_empty() {
            return Vec::new();
        }
        vec![
            Block::Heading(h, "Vehicles parked here".into()),
            Block::Bullets(cars.iter().map(|o| vec![link(o.display_name(), "pages/vehicles.md")]).collect()),
        ]
    }

    fn floor_page(&mut self, l: &Level) -> Page {
        let title = self.p.entity_name(EntityRef::Level(l.id));
        let mut page = Page::new(self.page_path(&title), title);
        let blocks = self.floor_blocks(l, 2);
        page.extend(blocks);
        page
    }

    fn building_page(&mut self, b: &Building) -> Page {
        let p = self.p;
        let mut page = Page::new(self.page_path(&b.display_name()), b.display_name());
        let mut pairs = vec![("Type", b.kind.clone())];
        pairs.extend(self.details(&b.details));
        page.extend(self.fields_with(pairs, &b.details.custom_fields));
        page.extend(hardware_table(&b.details.hardware));
        page.extend(self.paints_where(|t| matches!(t, PaintTarget::Building(x) if *x == b.id)));
        page.extend(self.extras(EntityRef::Building(b.id), 2));
        let levels = p.floor_stack(Some(b.id));
        let many = levels.len() > 1;
        for l in levels.into_iter().rev() {
            if many {
                page.push(Block::Heading(2, l.name.clone()));
            }
            let blocks = self.floor_blocks(l, if many { 3 } else { 2 });
            page.extend(blocks);
        }
        page
    }

    fn outside_page(&mut self) -> Page {
        let p = self.p;
        let site = p.site().expect("checked by the caller");
        let mut page = Page::new(self.page_path("Outside"), "Outside");
        page.extend(self.plan("Outside", svg::site(p, "Outside"), "Plan of the yard"));
        page.extend(self.extras(EntityRef::Level(site.id), 2));
        let mut feats: Vec<&Feature> = site.features.values().collect();
        feats.sort_by_key(|f| (f.kind != FeatureKind::PropertyLine, f.kind_label(), f.display_name().to_lowercase()));
        for f in feats {
            page.push(Block::Heading(2, f.display_name()));
            let size = match f.kind.geom() {
                FeatureGeom::Area => self.area(f.area()),
                FeatureGeom::Line => self.len(f.length()),
                FeatureGeom::Point => if f.size > 0.0 { format!("{} across", self.len(f.size)) } else { String::new() },
            };
            let mut pairs = vec![("Type", f.kind_label()), ("Size", size), ("Material", f.material.clone())];
            if let Some(label) = f.kind.height_label() {
                if f.height > 0.0 {
                    pairs.push((label, self.len(f.height)));
                }
            }
            if f.railing {
                pairs.push(("Railing", if f.railing_material.trim().is_empty() { "Yes".into() } else { f.railing_material.clone() }));
            }
            pairs.push(("Stairs", f.stairs.clone()));
            if f.shared_with_neighbour {
                pairs.push(("Shared with", if f.neighbour.trim().is_empty() { "A neighbour".into() } else { f.neighbour.clone() }));
            }
            if !f.gates.is_empty() {
                let gates: Vec<String> = f.gates.iter().map(|g| if g.label.trim().is_empty() { format!("{} wide", self.len(g.width)) } else { format!("{} ({})", g.label.trim(), self.len(g.width)) }).collect();
                pairs.push(("Gates", gates.join(", ")));
            }
            pairs.extend(self.details(&f.details));
            page.extend(self.fields_with(pairs, &f.details.custom_fields));
            if f.kind == FeatureKind::GardenBed && !p.garden_years(f.id).is_empty() {
                page.push(Block::Para(vec![text("See "), link(format!("{} plantings", f.display_name()), format!("pages/{}.md", self.garden_slug(f))), text(".")]));
            }
            page.extend(self.paints_where(|t| matches!(t, PaintTarget::Feature(x) if *x == f.id)));
            page.extend(self.extras(EntityRef::Feature(f.id), 3));
            let mut items: Vec<&PlacedObject> = site.objects.values().filter(|o| o.feature == Some(f.id) && o.vehicle.is_none()).collect();
            items.sort_by_key(|o| o.display_name().to_lowercase());
            for o in items {
                page.extend(self.item(site, o, 3));
            }
        }
        let mut loose: Vec<&PlacedObject> = site.objects.values().filter(|o| o.feature.is_none() && o.vehicle.is_none()).collect();
        if !loose.is_empty() {
            loose.sort_by_key(|o| o.display_name().to_lowercase());
            page.push(Block::Heading(2, "Other things outside".into()));
            for o in loose {
                page.extend(self.item(site, o, 3));
            }
        }
        page.extend(self.vehicles_here(site, 2));
        page
    }

    /// Garden pages are named up front so the outside page can link to them.
    fn garden_slug(&self, f: &Feature) -> String {
        format!("garden-{}", slug(&f.display_name()))
    }

    fn garden_page(&mut self, g: &Feature) -> Page {
        let p = self.p;
        let path = format!("pages/{}.md", self.garden_slug(g));
        self.slugs.insert(self.garden_slug(g));
        let mut page = Page::new(path, g.display_name());
        let s = &p.garden;
        page.extend(fields(vec![
            ("Size", self.area(g.area())),
            ("Last spring frost", s.last_frost.map(|d| d.format("%b %-d").to_string()).unwrap_or_default()),
            ("First fall frost", s.first_frost.map(|d| d.format("%b %-d").to_string()).unwrap_or_default()),
            ("Hardiness zone", s.zone.clone()),
        ]));
        // Newest first, including years where only perennials carried on.
        // Years with exactly the same plantings share a heading ("2024–2026").
        let first = p.garden_years(g.id).last().copied().unwrap_or(self.opts.today.year());
        let last = self.opts.today.year().max(p.garden_years(g.id).first().copied().unwrap_or(first));
        let mut spans: Vec<(i32, i32, Vec<&Planting>)> = Vec::new();
        for year in (first..=last).rev() {
            let plantings = p.plantings_in(g.id, year);
            if plantings.is_empty() {
                continue;
            }
            let ids: Vec<PlantingId> = plantings.iter().map(|x| x.id).collect();
            match spans.last_mut() {
                Some((from, _, prev)) if prev.iter().map(|x| x.id).collect::<Vec<_>>() == ids && *from == year + 1 => *from = year,
                _ => spans.push((year, year, plantings)),
            }
        }
        let mut detailed = BTreeSet::new();
        for (from, to, plantings) in spans {
            let years = if from == to { from.to_string() } else { format!("{from}–{to}") };
            page.push(Block::Heading(2, years.clone()));
            let title = format!("{} {years}", g.display_name());
            page.extend(self.plan(&title, svg::garden(g, &plantings, &title), &format!("{} in {years}", g.display_name())));
            page.push(Block::Table {
                head: ["Planting", "How", "Started", "Planted", "Taken out", "Harvest"].map(String::from).to_vec(),
                rows: plantings.iter().map(|x| self.planting_row(x)).collect(),
            });
            for x in plantings {
                let more = self.has_more(EntityRef::Planting(x.id), None, &x.custom_fields) || x.harvests.iter().any(|h| !h.notes.trim().is_empty());
                if more && detailed.insert(x.id) {
                    page.extend(self.planting(x, 3));
                }
            }
        }
        page.extend(self.extras(EntityRef::Feature(g.id), 2));
        page
    }

    fn planting_row(&self, x: &Planting) -> Vec<String> {
        let size = match x.layout {
            Layout::Row => format!("{} row", self.len(x.length())),
            Layout::Patch => format!("{} patch", self.area(x.area())),
            Layout::Plant => "1 plant".into(),
        };
        let count = x.count.map(|n| format!(", {n} plants")).unwrap_or_default();
        let harvest: Vec<String> = x.total_harvest().iter().map(|(unit, amount)| format!("{} {unit}", trim_num(*amount))).collect();
        vec![x.display_name(), format!("{} ({size}{count})", x.method.label()), opt_date(x.started), opt_date(x.planted), opt_date(x.removed), harvest.join(", ")]
    }

    fn planting(&self, x: &Planting, h: u8) -> Vec<Block> {
        let mut out = vec![Block::Heading(h, x.display_name())];
        out.extend(self.fields_with(vec![("Variety", x.variety.clone()), ("Days to harvest", x.days_to_harvest.map(|d| d.to_string()).unwrap_or_default())], &x.custom_fields));
        if !x.harvests.is_empty() {
            out.push(Block::Table {
                head: ["Harvested", "Amount", "Notes"].map(String::from).to_vec(),
                rows: x
                    .harvests
                    .iter()
                    .map(|hv| vec![date(hv.date), hv.amount.map(|a| format!("{} {}", trim_num(a), hv.unit)).unwrap_or_default(), hv.notes.clone()])
                    .collect(),
            });
        }
        out.extend(self.extras(EntityRef::Planting(x.id), h + 1));
        out
    }

    fn seeds_page(&mut self) -> Page {
        let mut page = Page::new(self.page_path("Seeds"), "Seeds");
        let mut seeds: Vec<_> = self.p.seeds.iter().collect();
        seeds.sort_by_key(|s| s.display_name().to_lowercase());
        page.push(Block::Table {
            head: ["Seeds", "From", "Packed for", "Bought", "Left", "Germination", "Organic", "Used up", "Notes"].map(String::from).to_vec(),
            rows: seeds
                .iter()
                .map(|s| {
                    vec![
                        s.display_name(),
                        s.source.clone(),
                        s.packed_for.map(|y| y.to_string()).unwrap_or_default(),
                        opt_date(s.bought),
                        s.amount.clone(),
                        s.germination.map(|g| format!("{}%", trim_num(g))).unwrap_or_default(),
                        yes(s.organic),
                        yes(s.used_up),
                        s.notes.clone(),
                    ]
                })
                .collect(),
        });
        page
    }

    fn roof_page(&mut self) -> Page {
        let p = self.p;
        let mut page = Page::new(self.page_path("Roof"), "Roof");
        let r = &p.roof;
        let pairs = vec![
            ("Material", r.material.clone()),
            ("Colour", r.color.clone()),
            ("Pitch", r.pitch.clone()),
            ("Area", r.area_m2.map(|a| self.area(a)).unwrap_or_default()),
            ("Installed", opt_date(r.installed)),
            ("Installed by", r.installed_by.clone()),
            ("Warranty until", opt_date(r.warranty_expires)),
            ("Warranty details", r.warranty_details.clone()),
            ("Last inspected", opt_date(r.last_inspected)),
        ];
        page.extend(self.fields_with(pairs, &r.custom_fields));
        page.extend(self.extras(EntityRef::Roof, 2));
        for l in p.levels.iter().filter(|l| !l.roofs.is_empty()) {
            let title = format!("Roof over {}", p.entity_name(EntityRef::Level(l.id)));
            page.extend(self.plan(&title, svg::roof(l, &title, self.units), &title));
            let mut sections: Vec<_> = l.roofs.values().collect();
            sections.sort_by_key(|s| s.display_name().to_lowercase());
            for s in sections {
                page.push(Block::Heading(2, s.display_name()));
                let pairs = vec![
                    ("Shape", s.shape.label().to_string()),
                    ("Pitch", if s.shape.is_sloped() { format_pitch(s.pitch, self.units) } else { String::new() }),
                    ("Covers", self.area(s.plan_area())),
                    ("Roof area", self.area(s.surface_area())),
                    ("Overhang", if s.overhang > 0.0 { self.len(s.overhang) } else { String::new() }),
                    ("Last inspected", opt_date(s.last_inspected)),
                ];
                page.extend(self.fields_with(pairs, &s.custom_fields));
                let layers: Vec<_> = s
                    .layers
                    .iter()
                    .filter(|x| {
                        !(x.material.trim().is_empty()
                            && x.color.trim().is_empty()
                            && x.installed.is_none()
                            && x.installed_by.trim().is_empty()
                            && x.warranty_expires.is_none()
                            && x.warranty_details.trim().is_empty()
                            && x.notes.trim().is_empty())
                    })
                    .collect();
                if !layers.is_empty() {
                    page.push(Block::Heading(3, "Layers, top first".into()));
                    page.push(Block::Table {
                        head: ["Material", "Colour", "Installed", "Installed by", "Warranty until", "Notes"].map(String::from).to_vec(),
                        rows: layers
                            .iter()
                            .rev()
                            .map(|x| {
                                let notes = [x.warranty_details.trim(), x.notes.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(". ");
                                vec![x.material.clone(), x.color.clone(), opt_date(x.installed), x.installed_by.clone(), opt_date(x.warranty_expires), notes]
                            })
                            .collect(),
                    });
                }
                let mut items: Vec<&PlacedObject> = l.objects.values().filter(|o| o.roof == Some(s.id)).collect();
                items.sort_by_key(|o| o.display_name().to_lowercase());
                for o in items {
                    page.extend(self.item(l, o, 3));
                }
                page.extend(self.extras(EntityRef::RoofSection(s.id), 3));
            }
        }
        page
    }

    fn paint_page(&mut self) -> Page {
        let p = self.p;
        let mut page = Page::new(self.page_path("Paint"), "Paint");
        let mut paints: Vec<&Paint> = p.paints.iter().collect();
        let place = |x: &Paint| match x.target {
            PaintTarget::WallFace { wall, room: Some(r) } => format!("{} ({})", p.entity_name(EntityRef::Room(r)), p.entity_name(EntityRef::Wall(wall))),
            t => p.entity_name(t.entity()),
        };
        paints.sort_by_key(|x| place(x).to_lowercase());
        page.push(Block::Table {
            head: ["Where", "Surface", "Brand", "Colour", "Code", "Finish", "Painted", "Leftover cans", "Notes"].map(String::from).to_vec(),
            rows: paints
                .iter()
                .map(|x| {
                    vec![
                        place(x),
                        custom_label(x.surface == PaintSurface::Other, &x.surface_name, x.surface.label()),
                        x.brand.clone(),
                        x.color_name.clone(),
                        x.color_code.clone(),
                        custom_label(x.finish == PaintFinish::Other, &x.finish_name, x.finish.label()),
                        opt_date(x.applied),
                        x.leftover_location.clone(),
                        x.notes.clone(),
                    ]
                })
                .collect(),
        });
        page
    }

    fn vehicles_page(&mut self) -> Page {
        let p = self.p;
        // Fixed name: floor pages link here.
        self.slugs.insert("vehicles".into());
        let mut page = Page::new("pages/vehicles.md", "Vehicles");
        for (li, id) in p.vehicles() {
            let l = &p.levels[li];
            let Some(o) = l.objects.get(&id) else { continue };
            let Some(v) = o.vehicle.as_deref() else { continue };
            page.push(Block::Heading(2, o.display_name()));
            page.push(para(format!("Parked: {}", p.entity_name(EntityRef::Level(l.id)))));
            page.extend(self.vehicle(o, v));
            page.extend(self.extras(EntityRef::Object(o.id), 3));
        }
        page
    }

    fn vehicle(&self, o: &PlacedObject, v: &Vehicle) -> Vec<Block> {
        let p = self.p;
        let d = &o.details;
        let mut out = Vec::new();
        let dist = |km: Option<f64>| km.map(|k| format_distance(k, self.units)).unwrap_or_default();
        let odometer = match (v.odometer, v.odometer_date) {
            (Some(k), Some(on)) => format!("{} on {}", format_distance(k, self.units), date(on)),
            (Some(k), None) => format_distance(k, self.units),
            _ => String::new(),
        };
        let charger = v.charger.map(|c| p.entity_name(EntityRef::Object(c))).unwrap_or_else(|| v.charger_name.clone());
        out.extend(fields(vec![
            ("Year", v.year.clone()),
            ("Make", d.make.clone()),
            ("Model", d.model.clone()),
            ("Trim", v.trim.clone()),
            ("Body", v.body.clone()),
            ("Colour", v.color.clone()),
            ("VIN", v.vin.clone()),
            ("Plate", [v.plate.trim(), v.plate_region.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", ")),
            ("Fuel", custom_label(v.fuel == Fuel::Other, &v.fuel_name, v.fuel.label())),
            ("Engine", v.engine.clone()),
            ("Transmission", v.transmission.clone()),
            ("Drivetrain", v.drivetrain.clone()),
            ("Driver", v.driver.clone()),
            ("Odometer", odometer),
            ("Keys", v.keys.clone()),
        ]));
        let ownership = fields(vec![
            ("Ownership", v.ownership.label().to_string()),
            ("Purchased", opt_date(d.purchased)),
            ("Price", d.price.clone()),
            ("Bought from", d.vendor.clone()),
            ("Odometer at purchase", dist(v.purchase_km)),
            ("Lender", v.lender.clone()),
            ("Account", v.account.clone()),
            ("Payment", v.payment.clone()),
            ("Term ends", opt_date(v.term_ends)),
            ("Lease distance", dist(v.lease_km)),
        ]);
        if let Some(b) = ownership {
            out.push(Block::Heading(3, "Ownership".into()));
            out.push(b);
        }
        let papers = fields(vec![
            ("Registration expires", opt_date(v.registration_expires)),
            ("Inspection due", opt_date(v.inspection_due)),
            ("Insurer", v.insurer.clone()),
            ("Policy", v.policy.clone()),
            ("Insurance renews", opt_date(v.insurance_renews)),
            ("Insurance phone", v.insurance_phone.clone()),
            ("Roadside assistance", v.roadside_phone.clone()),
            ("Dealer", v.dealer.clone()),
            ("Dealer phone", v.dealer_phone.clone()),
        ]);
        if let Some(b) = papers {
            out.push(Block::Heading(3, "Registration and insurance".into()));
            out.push(b);
        }
        let ev = fields(vec![
            ("Battery", v.battery.clone()),
            ("Battery health", v.battery_health.clone()),
            ("Range", v.range.clone()),
            ("Connector", v.connector.clone()),
            ("Cable", v.cable.clone()),
            ("Charger", charger),
        ]);
        if let Some(b) = ev {
            out.push(Block::Heading(3, "Charging".into()));
            out.push(b);
        }
        if !v.warranties.is_empty() {
            out.push(Block::Heading(3, "Warranties".into()));
            out.push(Block::Table {
                head: ["Warranty", "Provider", "Contract", "Starts", "Ends", "Up to", "Deductible", "Phone", "Details"].map(String::from).to_vec(),
                rows: v
                    .warranties
                    .iter()
                    .map(|w| vec![w.name.clone(), w.provider.clone(), w.contract.clone(), opt_date(w.starts), opt_date(w.expires), dist(w.until_km), w.deductible.clone(), w.phone.clone(), w.details.clone()])
                    .collect(),
            });
        }
        if !v.tires.is_empty() {
            out.push(Block::Heading(3, "Tires".into()));
            out.push(Block::Table {
                head: ["Season", "Tires", "Size", "On the car", "Stored at", "Bought", "Tread", "Notes"].map(String::from).to_vec(),
                rows: v
                    .tires
                    .iter()
                    .map(|t| {
                        let name = [t.brand.trim(), t.model.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
                        vec![t.season.label().to_string(), name, t.size.clone(), yes(t.mounted), t.stored_at.clone(), opt_date(t.purchased), t.tread_depth.clone(), t.notes.clone()]
                    })
                    .collect(),
            });
        }
        if !v.parts.is_empty() {
            out.push(Block::Heading(3, "Parts and fluids".into()));
            out.push(Block::Table {
                head: ["Part", "What it takes", "Part number", "Capacity", "Last replaced", "Notes"].map(String::from).to_vec(),
                rows: v
                    .parts
                    .iter()
                    .map(|x| {
                        let last = match (x.last_replaced, x.last_km) {
                            (Some(d), Some(k)) => format!("{} at {}", date(d), format_distance(k, self.units)),
                            (Some(d), None) => date(d),
                            (None, Some(k)) => format!("at {}", format_distance(k, self.units)),
                            _ => String::new(),
                        };
                        vec![x.name.clone(), x.spec.clone(), x.part_number.clone(), x.capacity.clone(), last, x.notes.clone()]
                    })
                    .collect(),
            });
        }
        if let Some(b) = self.fields_with(Vec::new(), &d.custom_fields) {
            out.push(Block::Heading(3, "Other details".into()));
            out.push(b);
        }
        if !v.documents.is_empty() {
            out.push(Block::Heading(3, "Documents".into()));
            out.extend(self.attachments(&v.documents));
        }
        out
    }

    /// Every piece of hardware in the house, by where it is: an inventory
    /// for buying matching replacements.
    fn hardware_page(&mut self) -> Page {
        let p = self.p;
        let mut page = Page::new(self.page_path("Hardware"), "Hardware");
        page.push(para("Everything fitted to the cabinets, doors and rooms, so a matching replacement is easy to find."));
        let mut all = p.all_hardware();
        all.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.2.kind.to_lowercase().cmp(&b.2.kind.to_lowercase())));
        page.push(Block::Table {
            head: ["Where", "Hardware", "How many", "Brand", "Model", "Part number", "Finish", "Size"].map(String::from).to_vec(),
            rows: all
                .iter()
                .map(|(_, at, x)| {
                    let kind = if x.kind.trim().is_empty() { "Hardware".to_string() } else { x.kind.clone() };
                    vec![at.clone(), kind, x.quantity.map(|n| n.to_string()).unwrap_or_default(), x.brand.clone(), x.model.clone(), x.part_number.clone(), x.finish.clone(), x.size.clone()]
                })
                .collect(),
        });
        page
    }

    fn pets_page(&mut self) -> Page {
        use crate::pet::*;
        let p = self.p;
        let today = self.opts.today;
        let mut page = Page::new(self.page_path("Pets"), "Pets");
        for pet in p.pets_in_order() {
            let mut title = pet.display_name();
            if let Some(d) = pet.passed {
                title = format!("{title} (in memory, {})", date(d));
            }
            page.push(Block::Heading(2, title));
            if let Some(photo) = pet.photo {
                page.extend(self.attachments(&[photo]));
            }
            let weight = pet.latest_weight().map(|w| format!("{} on {}", format_weight(w.kg, self.units), date(w.date))).unwrap_or_default();
            let birthday = match (pet.birthday, pet.birthday_estimated) {
                (Some(b), true) => format!("About {}", date(b)),
                (Some(b), false) => date(b),
                (None, _) => String::new(),
            };
            page.extend(fields(vec![
                ("Species", pet.species_label()),
                ("Breed", pet.breed.clone()),
                ("Sex", if pet.sex == Sex::Unknown { String::new() } else { pet.sex.label().to_string() }),
                ("Spayed / neutered", if pet.fixed { "Yes".into() } else { String::new() }),
                ("Born", birthday),
                ("Age", pet.age(today).unwrap_or_default()),
                ("Adopted", opt_date(pet.adopted)),
                ("Adopted from", pet.adopted_from.clone()),
                ("Colour and markings", pet.markings.clone()),
                ("Weight", weight),
                ("Microchip", [pet.microchip.trim(), pet.microchip_registry.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", ")),
                ("Licence / tag", pet.licence.clone()),
                ("Licence renews", opt_date(pet.licence_renews)),
            ]));
            let care = fields(vec![
                ("Food", pet.food.clone()),
                ("Feeding", pet.feeding.clone()),
                ("Treats", pet.treats.clone()),
                ("Allergies", pet.allergies.clone()),
                ("Conditions", pet.conditions.clone()),
                ("Temperament", pet.temperament.clone()),
                ("Grooming", pet.grooming.clone()),
                ("For the sitter", pet.care_notes.clone()),
            ]);
            if let Some(b) = care {
                page.push(Block::Heading(3, "Care".into()));
                page.push(b);
            }
            for (heading, vet) in [("Vet", &pet.vet), ("Emergency vet", &pet.emergency_vet)] {
                let b = fields(vec![
                    ("Clinic", vet.clinic.clone()),
                    ("Vet", vet.vet.clone()),
                    ("Phone", vet.phone.clone()),
                    ("Email", vet.email.clone()),
                    ("Address", vet.address.clone()),
                    ("Notes", vet.notes.clone()),
                ]);
                if let Some(b) = b {
                    page.push(Block::Heading(3, heading.into()));
                    page.push(b);
                }
            }
            let ins = &pet.insurance;
            let b = fields(vec![
                ("Provider", ins.provider.clone()),
                ("Policy", ins.policy.clone()),
                ("Claims phone", ins.phone.clone()),
                ("Coverage", ins.coverage.clone()),
                ("Deductible", ins.deductible.clone()),
                ("Renews", opt_date(ins.renews)),
            ]);
            if let Some(b) = b {
                page.push(Block::Heading(3, "Insurance".into()));
                page.push(b);
            }
            if !pet.vaccinations.is_empty() {
                let mut vax: Vec<&Vaccination> = pet.vaccinations.iter().collect();
                vax.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(b.given.cmp(&a.given)));
                page.push(Block::Heading(3, "Vaccinations".into()));
                page.push(Block::Table {
                    head: ["Vaccine", "Given", "Next due", "Given by", "Lot / tag", "Notes"].map(String::from).to_vec(),
                    rows: vax.iter().map(|v| vec![v.display_name(), opt_date(v.given), opt_date(v.due), v.given_by.clone(), v.lot.clone(), v.notes.clone()]).collect(),
                });
            }
            if !pet.medications.is_empty() {
                page.push(Block::Heading(3, "Medications".into()));
                page.push(Block::Table {
                    head: ["Medication", "Dose", "How often", "For", "Prescribed by", "Started", "Ends", "Refills", "Notes"].map(String::from).to_vec(),
                    rows: pet
                        .medications
                        .iter()
                        .map(|m| {
                            let ends = match m.ends {
                                Some(e) => date(e),
                                None => "Ongoing".into(),
                            };
                            vec![m.display_name(), m.dose.clone(), m.how_often.clone(), m.purpose.clone(), m.prescribed_by.clone(), opt_date(m.started), ends, m.refills.clone(), m.notes.clone()]
                        })
                        .collect(),
                });
            }
            if pet.weights.len() > 1 {
                let mut w = pet.weights.clone();
                w.sort_by_key(|e| std::cmp::Reverse(e.date));
                page.push(Block::Heading(3, "Weight".into()));
                page.push(Block::Table {
                    head: ["Date", "Weight"].map(String::from).to_vec(),
                    rows: w.iter().map(|e| vec![date(e.date), format_weight(e.kg, self.units)]).collect(),
                });
            }
            if let Some(b) = self.fields_with(Vec::new(), &pet.custom_fields) {
                page.push(Block::Heading(3, "Other details".into()));
                page.push(b);
            }
            if !pet.documents.is_empty() {
                page.push(Block::Heading(3, "Documents".into()));
                page.extend(self.attachments(&pet.documents));
            }
            page.extend(self.extras(EntityRef::Pet(pet.id), 3));
        }
        page
    }

    fn reminders_page(&mut self) -> Page {
        let p = self.p;
        let mut page = Page::new(self.page_path("Reminders"), "Reminders");
        let mut open: Vec<&Reminder> = p.reminders.iter().filter(|r| !r.is_done()).collect();
        open.sort_by_key(|r| r.effective_due());
        if !open.is_empty() {
            page.push(Block::Heading(2, "Coming up".into()));
            page.push(self.reminder_table(&open, true));
        }
        let mut done: Vec<&Reminder> = p.reminders.iter().filter(|r| r.is_done()).collect();
        if !done.is_empty() {
            done.sort_by_key(|r| std::cmp::Reverse(r.finished_on()));
            page.push(Block::Heading(2, "Done".into()));
            page.push(Block::Table {
                head: ["Reminder", "For", "Done"].map(String::from).to_vec(),
                rows: done.iter().map(|r| vec![r.title.clone(), p.entity_name(r.target), opt_date(r.finished_on())]).collect(),
            });
        }
        page
    }

    fn history_page(&mut self) -> Page {
        let p = self.p;
        let mut page = Page::new(self.page_path("History"), "History");
        if !p.service_records.is_empty() {
            let mut records: Vec<&ServiceRecord> = p.service_records.iter().collect();
            records.sort_by_key(|s| std::cmp::Reverse(s.date));
            page.push(Block::Heading(2, "Work done".into()));
            page.push(self.service_table(&records, true));
        }
        if !p.changelog.is_empty() {
            let mut log: Vec<&ChangeEntry> = p.changelog.iter().collect();
            log.sort_by(|a, b| b.effective_date().cmp(&a.effective_date()).then(b.timestamp.cmp(&a.timestamp)));
            page.push(Block::Heading(2, "Changes".into()));
            page.push(Block::Table {
                head: ["Date", "What changed", "Details"].map(String::from).to_vec(),
                rows: log.iter().map(|c| vec![date(c.effective_date()), c.summary.clone(), c.details.clone()]).collect(),
            });
        }
        page
    }

    // -----------------------------------------------------------------------
    // JSON

    fn json(&self) -> Vec<u8> {
        #[derive(Serialize)]
        struct File<'a> {
            id: AttachmentId,
            filename: &'a str,
            mime: &'a str,
            /// Where it is in this export.
            path: &'a str,
        }
        #[derive(Serialize)]
        struct Export<'a> {
            format: &'static str,
            version: u32,
            exported: DateTime<Utc>,
            includes_private: bool,
            files: Vec<File<'a>>,
            house: &'a Project,
        }
        let e = Export {
            format: "abode-export",
            version: 1,
            exported: Utc::now(),
            includes_private: self.opts.include_private,
            files: self.files.iter().map(|(id, (path, m))| File { id: *id, filename: &m.filename, mime: &m.mime, path }).collect(),
            house: self.p,
        };
        serde_json::to_vec_pretty(&e).unwrap_or_default()
    }
}

/// A thing's hardware as a table (nothing if it has none).
fn hardware_table(list: &[Hardware]) -> Vec<Block> {
    if list.is_empty() {
        return Vec::new();
    }
    vec![
        Block::Para(vec![Inline::Bold("Hardware".into())]),
        Block::Table {
            head: ["Hardware", "How many", "Brand", "Model", "Part number", "Finish", "Size", "Bought from", "Price", "Notes"].map(String::from).to_vec(),
            rows: list
                .iter()
                .map(|x| {
                    let kind = if x.kind.trim().is_empty() { "Hardware".to_string() } else { x.kind.clone() };
                    vec![kind, x.quantity.map(|n| n.to_string()).unwrap_or_default(), x.brand.clone(), x.model.clone(), x.part_number.clone(), x.finish.clone(), x.size.clone(), x.vendor.clone(), x.price.clone(), x.notes.clone()]
                })
                .collect(),
        },
    ]
}

/// 2.50 → "2.5", 3.0 → "3".
fn trim_num(n: f64) -> String {
    let s = format!("{n:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(include_private: bool) -> (Project, Vec<ExportFile>) {
        let mut p = crate::sample::sample_house();
        let mut n = Note::new(EntityRef::General);
        n.title = "Spare key".into();
        n.body = "Under the third flower pot".into();
        n.private = true;
        p.notes.push(n);
        let opts = Options { include_private, today: NaiveDate::from_ymd_opt(2026, 11, 6).unwrap() };
        let files = export(&p, opts, &|_| None).unwrap();
        (p, files)
    }

    fn file<'a>(files: &'a [ExportFile], path: &str) -> &'a str {
        let f = files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("no {path}"));
        std::str::from_utf8(&f.bytes).unwrap()
    }

    #[test]
    fn exports_the_sample() {
        let (p, files) = run(false);
        let readme = file(&files, "README.md");
        assert!(readme.starts_with(&format!("# {}", p.info.name)));
        assert!(readme.contains("](pages/"));
        assert!(files.iter().any(|f| f.path.starts_with("plans/") && f.path.ends_with(".svg")));
        assert!(files.iter().any(|f| f.path == "pages/outside.md"));
        assert!(files.iter().any(|f| f.path.starts_with("pages/garden-")));
        let json: serde_json::Value = serde_json::from_slice(&files.iter().find(|f| f.path == "house.json").unwrap().bytes).unwrap();
        assert_eq!(json["format"], "abode-export");
        assert_eq!(json["includes_private"], false);
        // Every page path is unique.
        let paths: BTreeSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths.len(), files.len());
    }

    #[test]
    fn private_notes_only_in_your_own_copy() {
        let (_, shared) = run(false);
        let (_, own) = run(true);
        let all = |files: &[ExportFile]| files.iter().map(|f| String::from_utf8_lossy(&f.bytes).into_owned()).collect::<String>();
        assert!(!all(&shared).contains("third flower pot"));
        assert!(all(&own).contains("third flower pot"));
        assert!(file(&own, "README.md").contains("(private)"));
    }

    #[test]
    fn hardware_inventory_in_both_copies() {
        for private in [false, true] {
            let (_, files) = run(private);
            let page = file(&files, "pages/hardware.md");
            assert!(page.contains("BP36550FB"), "vanity pulls");
            assert!(page.contains("Bathroom · Vanity"));
            assert!(page.contains("Front door"));
        }
        let (_, files) = run(false);
        assert!(file(&files, "pages/main-floor.md").contains("Ceiling White"));
    }

    #[test]
    fn pets_only_in_your_own_copy() {
        let (_, shared) = run(false);
        let (_, own) = run(true);
        assert!(!shared.iter().any(|f| f.path == "pages/pets.md"));
        let all = |files: &[ExportFile]| files.iter().filter(|f| f.path.ends_with(".md") || f.path.ends_with(".json")).map(|f| String::from_utf8_lossy(&f.bytes).into_owned()).collect::<String>();
        assert!(!all(&shared).contains("Biscuit"));
        let pets = file(&own, "pages/pets.md");
        assert!(pets.contains("## Biscuit"));
        assert!(pets.contains("Maple Grove Animal Hospital"));
        assert!(pets.contains("Simparica Trio"));
        assert!(pets.contains("Vet visits and care"));
    }

    #[test]
    fn names_are_safe_and_unique() {
        let mut taken = BTreeSet::new();
        assert_eq!(unique(&mut taken, "files", &file_name("photo.jpg")), "files/photo.jpg");
        assert_eq!(unique(&mut taken, "files", &file_name("Photo.JPG")), "files/Photo (2).JPG");
        assert_eq!(file_name("a/b:c?.pdf"), "a-b-c-.pdf");
        assert_eq!(slug("Main floor · Loft"), "main-floor-loft");
    }
}
