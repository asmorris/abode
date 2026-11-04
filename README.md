# Abode

A home-owner's manual for your house: floor plans, rooms, appliances, paint,
service history, reminders and notes, all in one file you can hand to the next owner.

## Running

```sh
cargo run --release -p abode-app              # new empty house
cargo run --release -p abode-app -- --sample  # a sample house to explore
cargo run --release -p abode-app -- path/to/house.abode
cargo test --workspace
```

The first launch asks for your name (it's shown in the timeline).

## What's in milestone 1

- **2D plan editor**: rectangle and freeform rooms, multiple levels, snapping to
  corners, walls and grid. Walls are **shared**: drawing a room against another
  splits walls so the common section is one wall in both rooms.
- **Shape lock**: a room is editable while you draw it, then locks when you
  select something else. Reshaping later means an explicit *Unlock…*. Items stay
  where they are when a room is resized.
- **Blueprint tracing**: import an image per level, calibrate it by clicking two
  points with a known distance, move/rotate/fade it, and draw rooms on top.
- **Doors and windows** on walls, with style, size, swing, and full details.
- **Items** (appliances, fixtures…) from a bank of shapes and presets. Each
  belongs to the room it's in; dragging it into another room moves it there.
- **Details on everything**: make, model, serial, purchase/install dates, price,
  vendor, warranty, custom fields.
- **Notes with attachments** (photos, PDFs, any file, embedded in the house file),
  **service history**, and **reminders** (repeating, "mark done" logs a service
  record) on the house, roof, levels, rooms, walls, doors, windows and items.
- **Paint** per room or per wall face (brand, colour name/code, finish, swatch,
  date, where the leftover cans are), shown on the plan and in 3D.
- **Roofs**: draw roof sections on each floor's Roof layer: gable, hip, shed or
  flat, with pitch, ridge direction, overhang and eaves height, so a porch roof can
  sit lower than the main roof. Each section keeps its **layers** of roofing
  (material, colour, installer, warranty) and items like skylights, chimneys,
  vents, solar panels and gutters.
- **Outside**: an Outside plan around the house's ground floor: property line,
  decks (height, railing, stairs), patios, driveways, paths, garden beds, pools,
  lawns, fences with gates (and whether they're shared with a neighbour),
  retaining walls, hedges and trees. **Outbuildings** like a shed or detached
  garage are mini buildings with their own floors, doors, windows, items and roof.
  Double-click one to open its plan.
- **House** details and **general notes**.
- **Light and dark** themes (Settings → Appearance).
- **3D view**: walls extruded from the plan, with orbit, pan and zoom.
- **Timeline**: every change is logged automatically ("Updated item “Furnace” on
  Utility: model, serial"), with bursts of edits merged. Write your own entries
  too, with past dates allowed. Each save stores a restorable snapshot.
- **Undo/redo**, metric/imperial (type `12' 6"` or `3.8 m` anywhere).
- **Private** notes, service records and custom fields. *File → Export copy for a
  new owner* strips them, along with save history, and rewrites the file so
  deleted data can't be recovered.

## Design

A full-window drafting canvas with floating white panels: the house tree on the left, the inspector on the right (when something is selected), and tool pills at the top and bottom. Light and dark themes. Colours: forest green for primary actions, soft mint for selection, teal for selections on the plan, coral for highlights and warnings, on a warm off-white grid. Inter for UI text, Instrument Serif for titles, Phosphor line icons. Everything lives in `crates/abode-app/src/theme.rs`.

## Layout

```
crates/
  abode-core/   no UI; reusable by a future web viewer, mobile app or sync server
    model.rs      the data model (Project → Level → nodes/walls/rooms/openings/objects)
    topology.rs   shared-wall floor-plan graph: add/remove rooms, splitting, resizing
    ops.rs        project-level edits that keep references consistent
    diff.rs       automatic change log (record-by-record diff + merging)
    storage.rs    the .abode SQLite file format
    sharing.rs    sanitising for export
    reminders.rs  scheduling
    roof.rs       roof sections: shapes, facets, ridge lines, 3D mesh
    site.rs       outside: decks, fences, trees, outbuildings
    units.rs      metric/imperial formatting and parsing
    geometry.rs   polygons, triangulation, hit testing
    blueprint.rs  importer trait (add PDF / auto-detection importers here)
  abode-app/    egui desktop app
    theme.rs      design system: palette, fonts, panels, buttons, chips, sections
    document.rs   open file + undo/redo + commit-to-changelog
    app.rs        layout (floating panels over the canvas), dialogs, file actions
    ui/           plan editor, 3D, inspector, navigator, timeline, reminders
    assets/fonts/ Inter + Instrument Serif (SIL Open Font License, see *-OFL.txt)
```

### The `.abode` file

A single SQLite database (SQLite is compiled into the app, so there's nothing to install):

| table | contents |
|---|---|
| `meta` | format name/version, project id, timestamps |
| `project`, `levels`, `nodes`, `walls`, `rooms`, `openings`, `objects`, `roofs`, `features`, `buildings`, `notes`, `service_records`, `reminders`, `paints`, `changelog` | one row per entity: `(id, parent, position, data JSON)` |
| `attachments` | embedded files (metadata + blob) |
| `revisions` | a snapshot per save (the latest 200 are kept) |

While a file is open the app works on a private copy in a temp folder. Saving
writes it back atomically, so a crash never corrupts your file. All ids are
UUIDv7, which keeps future syncing and merging collision-free.

## Roadmap ideas

- Contractor workflow: they edit a copy, you review and merge their changes
  (the per-record diff in `diff.rs` is the basis).
- Sync / sharing service (file format is already row-per-entity with UUIDs).
- PDF blueprint import; automatic room detection.
- Wiring and plumbing layers with room-aware constraints (likely moving 3D to wgpu).
- Mobile capture (photos of labels straight into notes).
