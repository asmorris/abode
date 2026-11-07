# Developing Abode

Abode is a Rust desktop app built with [egui](https://github.com/emilk/egui).
It runs on macOS and Windows.

## Running from source

Install Rust with [rustup](https://rustup.rs). The right toolchain is picked
up from `rust-toolchain.toml`.

```sh
cargo run --release -p abode-app              # start screen
cargo run --release -p abode-app -- --sample  # the sample house
cargo run --release -p abode-app -- path/to/house.abode
cargo test --workspace
cargo clippy --workspace --all-targets
```

## Layout

```
crates/
  abode-core/   no UI; the data model and everything that doesn't draw
    model.rs      the data model (Project → Level → nodes/walls/rooms/openings/objects)
    topology.rs   shared-wall floor-plan graph: add/remove rooms, splitting, resizing
    ops.rs        project-level edits that keep references consistent
    diff.rs       automatic change log (record-by-record diff + merging)
    storage.rs    the .abode SQLite file format
    sharing.rs    stripping private data for "Export copy for a new owner"
    export/       "Export as documents": pages built as a format-neutral doc
                  (doc.rs) and written as Markdown, SVG plans (svg.rs),
                  attached files and house.json
    reminders.rs  scheduling (by date and by odometer)
    vehicle.rs    vehicles: warranties, tires, parts, distances
    garden.rs     plantings, seed packets, the crop library, planting calendar,
                  rotation and companion hints
    roof.rs       roof sections: shapes, facets, ridge lines, 3D mesh
    site.rs       outside: decks, fences, carports, trees, outbuildings
    units.rs      metric/imperial formatting and parsing
    geometry.rs   polygons, triangulation, hit testing
    blueprint.rs  importer trait (add PDF / auto-detection importers here)
    sample.rs     the sample house
  abode-app/    egui desktop app
    theme.rs      design system: palette, fonts, panels, buttons, chips, sections
    document.rs   open file + undo/redo + commit-to-changelog
    app.rs        layout (floating panels over the canvas), dialogs, file actions
    updater.rs    checks GitHub releases, downloads and installs updates
    ui/           start screen, plan editor (ui/plan/garden.rs: the garden plan),
                  3D, inspector, navigator, timeline, reminders, vehicles, gardens
    assets/fonts/ Inter + Instrument Serif (SIL Open Font License, see *-OFL.txt)
    assets/icon/  app icon (png, icns, ico)
packaging/      macOS Info.plist, Windows NSIS installer script
scripts/        package-macos.sh, release.sh
```

## Design

A full-window drafting canvas with floating panels: the house tree on the
left, the inspector on the right (when something is selected), and tool pills
at the top and bottom. Forest green for primary actions, mint for selection,
teal for selections on the plan, coral for highlights and warnings, on a warm
off-white grid. Inter for UI text, Instrument Serif for titles, Phosphor
icons. Light and dark themes. All of it lives in
`crates/abode-app/src/theme.rs`.

## The `.abode` file

A single SQLite database (SQLite is compiled into the app):

| table | contents |
|---|---|
| `meta` | format name/version, project id, timestamps |
| `project`, `levels`, `nodes`, `walls`, `rooms`, `openings`, `objects`, `roofs`, `features`, `buildings`, `notes`, `service_records`, `reminders`, `paints`, `changelog`, `plantings`, `seeds` | one row per entity: `(id, parent, position, data JSON)` |
| `attachments` | embedded files (metadata + blob) |
| `revisions` | a snapshot per save (the latest 200 are kept) |

While a file is open the app works on a private copy in a temp folder. Saving
writes it back atomically, so a crash never corrupts the file. Saving also
drops attachments nothing refers to any more. All ids are UUIDv7.

## Debugging with screenshots

The app can render one frame to a PNG and quit, which is handy for checking
layout changes:

```sh
ABODE_SCREENSHOT=/tmp/shot.png ABODE_SELECT="Kitchen" ./target/debug/abode --sample
```

Other variables: `ABODE_TAB` (`plan`, `3d`, `timeline`, `reminders`,
`vehicles`), `ABODE_LEVEL` (a level name or `outside`), `ABODE_UNITS=imperial`,
`ABODE_SCREEN=welcome` (the start screen), `ABODE_FAKE_UPDATE=0.2.0` (show
the update banners as if 0.2.0 were out), `ABODE_GARDEN=<garden bed name>`
(open its garden plan) with `ABODE_YEAR=2025`, `ABODE_CROP_SEARCH=<text>`
(open the crop picker with that typed in), `ABODE_TOOL=item|building` (start
with that tool), `ABODE_PICKER=<a dropdown's current text>` with
`ABODE_PICKER_QUERY=<text>` (open a searchable dropdown with that typed in),
`ABODE_DIALOG=export-docs` or `export-docs-all` (the export dialog), and
`ABODE_FOCUS=<section>` (open and scroll to a panel section).

`ABODE_EXPORT_DOCS=<folder> ./target/debug/abode --sample` exports the sample
(or a file path instead of `--sample`) as documents into that folder and
exits without opening a window; add `ABODE_EXPORT_PRIVATE=1` for the
"everything" copy.

Dropdowns with more than six options (`widgets::SEARCH_OVER`) get a search
box, and every one also takes a typed value ("＋ Use “typed text”").
`search_combo` / `search_list` take a list of `Choice`s. For enum fields,
`widgets::combo_named` stores the typed name in a companion string (e.g.
`PlacedObject::category_name`) with the enum set to its Other variant;
`category_label()`, `kind_label()` and friends show it. Menus near the bottom
of the window should use `widgets::menu_dropdown` / `dropdown`, which open
upwards when there isn't room below.

## Releasing

```sh
scripts/release.sh 0.2.0
```

main only accepts pull requests, so the script bumps the version on a
`release-v0.2.0` branch, opens a pull request, waits for CI, merges it, then
tags the merge commit `v0.2.0`. Needs the GitHub CLI (`gh`), signed in. If it
stops part way, fix the problem and run the same command again.

The tag starts the [Release workflow](../.github/workflows/release.yml), which
builds and publishes:

| file | what |
|---|---|
| `Abode-0.2.0-mac.dmg` | universal Mac app (Apple Silicon + Intel), drag to Applications |
| `Abode-0.2.0-mac.zip` | the same app, used by the in-app updater |
| `Abode-0.2.0-windows-setup.exe` | per-user Windows installer (NSIS): Start menu, desktop shortcut, `.abode` files, uninstaller |

The release notes are written by GitHub from the pull requests merged since
the last release, so give pull requests titles that make sense to someone
using the app. The version-bump pull request is labelled `release` and left
out (see [.github/release.yml](../.github/release.yml)). The app's
"What's new" link opens these notes.

Abode checks `api.github.com/repos/asmorris/abode/releases/latest` on launch
(can be turned off in Settings). **Update** downloads the right file, swaps
the app (Mac) or runs the installer silently (Windows), then restarts.

Build the Mac package locally with `scripts/package-macos.sh`. To test the
updater end to end, build with `ABODE_PRETEND_VERSION=0.0.1` so the app thinks
it's out of date. The app icon is drawn by
`cargo run -p abode-app --example make_icon`.

The apps are only ad-hoc signed, so the first launch needs "Open Anyway"
(Mac) or "Run anyway" (Windows). To sign later, add a Developer ID step to
`scripts/package-macos.sh` and a signing step to the Windows job.
