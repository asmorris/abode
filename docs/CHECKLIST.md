# Abode: progress checklist

Status after milestone 1, the redesign, and roofs/outside/dark mode. Organised against the original
spec in [`original-spec.md`](original-spec.md).

Legend: `[x]` done · `[~]` partly done · `[ ]` not started

---

## 1. Done vs. remaining, by the original spec

### 1. 2D/3D modelling of rooms

- [x] **2D floor-plan editor**: rectangle rooms (drag), freeform polygon rooms (click corners)
- [x] **3D view**: read-only extrusion of walls, floors, doors/windows and items, with orbit, pan and zoom
- [~] **Input rooms from an existing blueprint**
  - [x] Import an image (PNG/JPEG/GIF/BMP/WebP) per level as an underlay
  - [x] Calibrate the scale by clicking two points + typing the real distance
  - [x] Move, rotate, fade, show/hide the underlay
  - [x] Pluggable `BlueprintImporter` trait for new formats
  - [ ] PDF import
  - [ ] Automatic room detection from a blueprint
- [x] **Input rooms from your own design**
- [~] **Change lengths/widths of rooms**
  - [x] Width/depth fields for rectangular rooms
  - [x] Drag corners to reshape any room (when unlocked)
  - [x] Lock after creation; "Unlock…" with a warning to change later ("discouraged, not impossible")
  - [x] Items keep their position when a room is resized
  - [ ] Type exact edge lengths for non-rectangular rooms
- [x] **Multiple rooms**
- [x] **Build a house from rooms**: shared walls, auto-splitting where rooms partly touch, multiple levels with elevation (basements via negative elevation)
- [x] **Label rooms** (name + type with suggestions, flooring, ceiling height)
- [x] **Label walls** (label, thickness, height override; shown on plan)
- [x] **Notes on rooms**
- [x] **Roof information**: house-wide roof record (optional overall details, last inspected, custom fields, notes/service/reminders)
- [x] **Roof modelling**
  - [x] Roof sections drawn on each floor's Roof layer (rectangle or any polygon), snapping to walls
  - [x] Shapes: gable, hip, shed, flat; pitch (x/12 or degrees), ridge/slope direction, overhang, eaves height
  - [x] Sections at different heights (e.g. a lower porch roof on lower eaves)
  - [x] Stacked layers per section (material, colour, swatch, installed, by, warranty, notes)
  - [x] Roof items: skylight, chimney, vent, solar panels, gutter, downspout, satellite dish (each with details/notes/service/reminders)
  - [x] Plan: shaded facets, ridge/hip lines, wall line; 3D: surfaces, fascia, gable ends
  - [ ] L-shaped/valley roofs as one section (draw two overlapping sections for now)
  - [ ] Gutters/downspouts drawn along eaves rather than as flat items
- [x] **Outside**
  - [x] An "Outside" plan with the house's ground floor for reference, optional survey underlay
  - [x] Property line (dashed, lot area)
  - [x] Areas: deck (height, railing, stairs), patio, driveway, path, garden bed, pool, lawn
  - [x] Lines: fence (posts, gates, shared with neighbour), retaining wall, hedge
  - [x] Trees (species, canopy, height, planted)
  - [x] Outbuildings (shed, detached garage…) as mini buildings with their own floors, rooms, doors, windows, items and roof
  - [x] Items outside (BBQ, hot tub, AC unit, hose tap…), sitting on a deck/patio when dropped on one
  - [x] Stain/paint/sealant records on decks, fences, outbuilding siding
  - [x] All of it in 3D
  - [ ] Grade/terrain heights (everything outside sits at ground level 0)
- [x] **Paint notes**: per room or per wall face; brand, colour name/code, finish, surface (walls/ceiling/trim…), swatch, date, leftover-can location, notes; swatches shown on plan and in 3D

### 2. Objects in rooms

- [~] **Bank of primitives**
  - [x] Rectangle, circle, ellipse, triangle
  - [x] Presets (fridge, stove, washer, dryer, furnace, water heater, sump pump, panel…)
  - [ ] Custom polygon / freeform object shapes
- [x] **Associated with a room** (auto-assigned by position; dragging into another room re-assigns)
- [x] **Own labels and categories** (appliance, HVAC, plumbing, electrical…)
- [x] **Notes on objects**
- [x] **Images in notes, many per note** (any file type, embedded in the house file; thumbnails for images; open/save copy)
- [x] **Upkeep info**: make, model, serial, purchased, installed, price, vendor, warranty date + details, custom fields
- [x] **Service history** (date, type, who, cost, description, attachments, private flag)
- [x] **Reminders** (one-off or repeating; "mark done" logs a service record and reschedules)

### 3. Windows and doors

- [x] Placed on walls; styles (hinged/double/sliding/pocket/bifold/garage; fixed/casement/hung/slider/awning/bay)
- [x] Width, height, sill height, position, hinge/swing flip
- [x] Same notes, dates, details, service history, reminders as objects

### 4. General notes

- [x] House-level general notes (plus house-level service history and reminders)
- [ ] Search across all notes

### Sharing and files

- [x] **Save / load** (`.abode`, single SQLite file, atomic save, works on a temp copy while open)
- [x] **Recent files**, unsaved-changes prompts on new/open/quit
- [x] **Shareable copy for a buyer**: *Export copy for a new owner* strips private notes, service records, custom fields, their attachments, and save history, then vacuums so deleted data can't be recovered
- [ ] **Paid / hosted sharing service**

### "Maybe" items

- [~] **Timeline of changes (git-like)**
  - [x] Automatic human-readable change log, per-field ("Updated item “Furnace”: model, serial"), merged within 15 min
  - [x] Filter the timeline to the selected room/object
  - [x] Hand-written entries with past dates, optional privacy
  - [x] Snapshot per save; restore any saved version (keeps the log; undoable)
  - [x] Undo/redo
  - [ ] Visual diff between two saved versions
  - [ ] Edit (not just delete) hand-written entries
- [ ] **Contractor access** (limited edits, owner merges)

### "Eventual" items

- [ ] Wiring: lights, switches, outlets, circuits, with room constraints
- [ ] Plumbing, with room constraints

---

## 2. Known gaps and rough edges in what's built

These came up while building; none block normal use, but each is worth a ticket.

**Not yet exercised by hand.** I verified rendering via screenshots and the
core logic via tests, but not live mouse/keyboard interaction. See the
[manual QA list](#4-manual-qa-checklist).

**Geometry**
- [ ] Dragging a corner can make a room outline cross itself. `Level::move_node` doesn't validate. Fix: check `is_simple` for every room using that node and reject or revert the move.
- [ ] Dropping a corner onto another corner doesn't merge them.
- [ ] Deleting a room leaves extra collinear corners in neighbours (the old T-junctions). Harmless, but a "merge collinear walls" pass would tidy it.
- [ ] `set_rect_size` moves shared corners, but not a neighbour's other corners on the same line. A taller neighbour can end up with a slanted wall. Consider moving all collinear nodes on that edge line.
- [ ] Room area/perimeter use wall centre-lines, not interior faces. Add an "interior area" that subtracts half the wall thickness.

**Roofs & outside**
- [ ] Hip roofs on concave (L-shaped) outlines are approximate; the roof maths is exact for convex outlines.
- [ ] Dragging a roof/feature corner can make it self-intersect (no validation yet).
- [ ] Moving the house doesn't move the decks/fences around it (they're independent).
- [ ] The colour-picker popup is still egui's stock one.

**Data/history**
- [ ] Notes/history attached to a *wall* are dropped if that wall disappears (e.g. a room is deleted). Consider re-targeting them to the surviving room, or warning first.
- [ ] Splitting a wall can log a spurious "Updated door: offset" for doors on the second half.
- [ ] Save snapshots are full JSON (minus the change log), capped at 200. Fine for houses, but compress them (zstd) or store deltas if files grow large.
- [ ] Attachments are read fully into memory when opened/thumbnailed. Very large videos could be slow. Stream to disk instead.
- [ ] `FORMAT_VERSION` = 1; the migration hook in `storage::check_format` is a stub.

**UI**
- [ ] Item labels can overflow small shapes.
- [ ] 3D uses painter's-algorithm sorting on egui's 2D painter, so some angles show layering glitches.
- [ ] No drag-and-drop of files onto notes.
- [x] Light and dark themes (Settings → Appearance: match system, light, dark).
- [x] Custom date picker, dropdown carets, toggle switches (no stock egui widgets left except the colour picker popup and sliders).
- [ ] No keyboard shortcut list / help screen.
- [ ] Some emoji glyphs may be missing in egui's font on other platforms. Swap any that render as boxes.

**Engineering**
- [ ] Few tests for the app crate (UI): 4 in `abode-app`, 43 in `abode-core`.
- [x] CI: tests + clippy on macOS, tests on Windows; release workflow on `v*` tags.
- [ ] Not tested on Windows or Linux.
- [x] Packaging: universal Mac .app/.dmg, Windows NSIS installer, icon, `.abode` file association on Windows, auto-updates from GitHub releases.
- [ ] Code signing/notarisation (Mac Developer ID, Windows certificate).
- [ ] `.abode` double-click on Mac (needs handling macOS open-file events).
- [ ] egui is pinned at 0.31 (latest 0.37). Upgrade in one sitting; expect small API changes (menus, `CornerRadius`, etc.).
- [ ] The debug screenshot hook (`ABODE_SCREENSHOT`) is debug-builds only. Keep it for visual regression tests or remove it.

---

## 3. Implementation notes for what remains

### Exact edge lengths for any polygon room
- In the room inspector, list each edge with a `length_edit`. On change, move the edge's *end* node along the edge direction. If that node is shared, warn (it moves the neighbour) or only allow it when unlocked.
- Validate with `is_simple` afterwards; revert on failure.

### PDF blueprints
- Add `PdfImporter: BlueprintImporter` in `abode-core/src/blueprint.rs`. Rasterise page 1 (or let the user pick a page) to PNG and return it as `ImportedBlueprint`. The rest of the pipeline (underlay, calibration) already works.
- Crate options: `pdfium-render` (best fidelity; needs the pdfium library shipped with the app) or `mupdf` bindings (AGPL; mind licensing if commercial). For vector PDFs, extracting line segments directly could later feed auto-detection.

### Automatic room detection
- Pipeline: greyscale → threshold → morphological close (join wall gaps at doors) → flood-fill / contour detection → polygon simplification (Douglas–Peucker) → snap to 90° where close → scale via calibration.
- Present results as *suggested rooms* the user accepts one by one (`Project::add_room` handles the shared walls).
- Crates: `imageproc` (pure Rust) or `opencv` (heavier, more capable). Keep it behind the importer trait so it's optional.

### Custom object shapes
- Add `ShapeKind::Polygon` with `points: Vec<Vec2>` in local coordinates; `Shape::outline` returns them. Hit-testing/drawing already use the outline, but drawing uses `convex_polygon`. Switch to the triangulated `fill_polygon` for concave shapes.

### Visual diff between saved versions
- `diff::diff(old, new)` already produces per-record changes. Load two revisions with `Store::load_revision`, run `diff`, and show the list. On the plan, highlight added (green), removed (red) and changed (amber) rooms/items.

### Contractor workflow (file-based, owner merges)
1. **Export for contractor**: like the new-owner export, plus `meta` keys `mode=contractor`, `base_revision=<id>`, `contractor_name`, `scope=[entity keys]`. Strip private data.
2. **Contractor mode in the app**: when `mode=contractor`, restrict the UI to adding notes, attachments and service records (and maybe details) on scoped entities. No geometry edits. This is a UI guard, not security: with plain files, nothing is truly enforceable without signing or a server.
3. **Import contractor changes**: owner picks the returned file. Three-way diff by record key: base (owner's revision `base_revision`) vs contractor's file vs owner's current. `diff.rs`'s `collect()` already builds keyed JSON records; extend it to a three-way merge. Show each change with Accept/Reject; flag conflicts where both sides changed the same record.
4. Log accepted changes with `author = contractor_name`.
5. Later, with a server, scope becomes real permissions and the import becomes a pull request–like review.

### Sync and hosted/paid sharing
- **Server**: Rust (Axum) + Postgres for rows, S3-compatible storage for attachments. The file's row-per-entity layout maps directly to tables.
- **Sync model**: start with per-record last-writer-wins plus a server-side change feed (`changelog` doubles as audit). Move to field-level merge or CRDTs (e.g. `automerge`) only if live co-editing is needed.
- **Accounts and ownership**: a house has an owner, plus invitees (viewer/contractor). "Transfer ownership" covers the sale: the buyer gets the house minus private data (reuse `sanitized_for_sharing`).
- **Web viewer**: eframe compiles to WASM, so a read-only web build of the same UI is feasible. Move `rusqlite` behind a feature flag (or use `sqlite-wasm`) so `abode-core` builds for wasm.
- **Payments**: Stripe. Free local app, paid hosting/sharing/transfer is a natural split.
- **IDs**: already UUIDv7 everywhere, so there's no remapping on sync.

### Wiring (eventual)
- **Model** (new collections on `Level`):
  - `ElectricalDevice { kind: Outlet|Switch|Light|Fan|Panel|Detector…, mount: Wall{wall, offset, height} | Ceiling{pos} | Floor{pos}, room, circuit: Option<CircuitId>, details }`
  - `Panel` (could reuse the existing "Electrical panel" item) with `Breaker { number, amps, label }`
  - `Circuit { breaker, devices: Vec<DeviceId> }`; `SwitchControl { switch, controls: Vec<DeviceId> }`
- **Constraints as warnings, not hard errors**: wall-mounted devices must be on a wall of their room; ceiling devices inside the room polygon; outlet spacing (e.g. NEC "6 ft / 12 ft" rule, make it locale-configurable); GFCI in kitchens/bathrooms; a switch near the latch side of the door.
- **UI**: a plan "layer" toggle (Structure / Electrical / Plumbing) that dims everything else, standard symbols, a circuit list in the panel's inspector, and a breaker-map printout.
- Wall-mounted devices should follow wall splits the same way openings do (see `Level::split_wall_at`).

### Plumbing (eventual)
- **Model**: fixtures can be existing items with category Plumbing; add `PipeRun { system: Supply{hot|cold}|Drain|Vent|Gas, material, diameter, points: Vec<Vec3> }`, `Valve { kind: Shutoff|…, position, controls: Vec<FixtureId> }`.
- **Constraints**: drains need slope (e.g. ¼" per foot), fixtures need vents, shutoffs per fixture. Highlight the "main shut-off" for emergencies.
- **Needs real 3D**: vertical runs between levels. At this point move the 3D view to wgpu (`egui_wgpu` paint callback, or embed `three-d`/`bevy` render-to-texture) and drop the painter's-algorithm renderer.

### Reminders and notifications
- In-app only today. For OS notifications, use `notify-rust` on launch for overdue items. True background reminders need a login item / tray helper, or calendar export (`.ics` with recurrence rules), which is cheap and works everywhere.

### Mobile
- Keep `abode-core` UI-free (already true). Options: (a) eframe on Android/iOS (works, immature); (b) native Swift/Kotlin UI calling `abode-core` via UniFFI. (b) gives the best camera/photo capture UX, which is the main mobile use case (photographing labels and serial numbers straight into notes).

### Extras worth considering
- "Home manual" PDF export (`typst` or `printpdf`): rooms, appliances with model/serial/warranty, paint list, shut-off locations.
- Search across notes, items, serial numbers.
- Import photos and auto-read model/serial numbers (OCR) on mobile.

---

## 4. Manual QA checklist

Run with `cargo run --release -p abode-app` (or `-- --sample`) and try:

- [ ] Draw a rectangle room; draw a second against it and confirm the shared wall (select it: "Between A and B")
- [ ] Draw a smaller room against a bigger one and confirm the wall splits
- [ ] Draw a polygon room (click, Shift for straight lines, Backspace, Enter / click first point)
- [ ] Edit width/depth of a new room; click away and confirm it locks; Unlock… and drag a corner
- [ ] Place doors/windows (each style), drag along the wall, flip hinge/swing
- [ ] Place items from presets; drag one into another room and check its "In" room changes
- [ ] Add notes with photos and a PDF; open and save-copy them; mark a note private
- [ ] Add service records and a repeating reminder; Mark done and check the next due date + service entry
- [ ] Add room paint and wall-face paint with swatches; check the plan and 3D
- [ ] Add a level, a negative-elevation basement, and switch levels
- [ ] Import a blueprint, calibrate, move it, trace a room over it
- [ ] Switch units to imperial; type `12' 6"` into a length
- [ ] Undo/redo across several kinds of edits
- [ ] Save, close, reopen and confirm everything is intact (including attachments)
- [ ] Save a few times; restore an older version from the Timeline
- [ ] Export for new owner; open the copy and confirm private items and save history are gone
- [ ] Quit with unsaved changes and confirm the prompt

---

## 5. Security notes

- Attachments come from whoever made the file (previous owner, contractor), so treat them as untrusted. Clicking an attachment only opens it with the system app for an allowlist of document/image/media types (`OPENABLE_EXTENSIONS` in `app.rs`). Anything else (executables, scripts, installers, HTML) can only be saved with "Save a copy…".
- Attachment filenames are sanitised before being written to the temp folder (no path separators, no `.`/`..`, no trailing dots/spaces).
- Opening a `.abode` file only reads JSON + blobs via SQLite. Nothing in it is executed.
- Future: when sharing via a server, scan uploads, cap attachment sizes, and sign contractor/owner exports so imports can be verified.
