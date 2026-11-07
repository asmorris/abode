# Abode

Abode is a record of your house: what's in it, what's been done to it, and
what needs doing next. You draw a simple floor plan, then attach details to
each room, wall, window or appliance: model and serial numbers, warranties,
paint colours, receipts, photos, service history and reminders.

Everything is kept in one file on your computer. When you sell the house, you
can export a copy for the new owner, without your private notes.

It runs on Mac and Windows.

## Installing

Download the installer for your computer from the
[latest release](https://github.com/asmorris/abode/releases/latest).
[docs/INSTALL.md](docs/INSTALL.md) has step-by-step instructions. The app
isn't signed by Apple or Microsoft, so the first launch needs one extra click.

## Getting started

The first time Abode opens, it asks for your name, which is used to label
changes in the timeline. Then you can:

- **Open the sample house** to see what a filled-in house looks like.
- **Start from scratch** and draw your own.
- **Open a saved house** from your recent files or `Documents/Abode`.

A quick way in:

1. Draw your rooms on the plan, as rectangles or by clicking corners. If you
   have a blueprint or listing floor plan, import it as an image and trace
   over it.
2. Add doors, windows and items (furnace, water heater, fridge...) to the
   rooms they're in.
3. Click anything to fill in its details on the right.
4. Add reminders for things that come up regularly, like a furnace filter or
   gutter cleaning. The Reminders page shows what's overdue or coming up.

You don't need to measure everything. A rough plan is enough to hang the
information on, and you can refine it later.

## What you can keep track of

- **Rooms and levels**: floors, basements, the roof and the yard, plus
  outbuildings like a shed or detached garage, and the stairs between floors
  (double-click stairs to go to the other floor). There's also a 3D view.
- **Items**: appliances and fixtures, with make, model, serial number,
  purchase and install dates, price, where you bought it, warranty and your
  own custom fields. Pick from the list (it's searchable) or type any name.
  Every long list works this way: if what you want isn't there, type it in.
- **Paint**: brand, colour, finish and where the leftover cans are, for each
  room or wall.
- **Roof**: roof sections, what each layer of roofing is, and who installed
  it, plus skylights, vents, solar panels and gutters.
- **Outside**: property line, decks, patios, driveways, fences, garden beds,
  trees, carports and EV chargers.
- **Gardens**: double-click a garden bed to plan it. Draw rows and patches, or
  place single plants, and say what's growing in each and when it went in.
  Enter your frost dates and Abode works out when to start seeds indoors,
  sow, transplant and harvest, for about 60 common vegetables, herbs, fruit
  and flowers. You can plant anything else too, by typing its name. It warns when a crop family grew in the same spot last year,
  and points out good and bad neighbours. You can also keep a harvest log
  and a list of the seeds you have.
- **Vehicles**: park them in the garage or driveway and keep their
  warranties, tires, parts, insurance, registration and service history.
  Reminders can be due by date or by odometer reading.
- **Notes and files**: photos, manuals, receipts and PDFs, attached to
  anything and stored inside the house file.
- **Service history**: who did what and when. Marking a reminder done adds a
  service record for you.
- **Timeline**: a log of every change, plus your own entries ("Replaced the
  hot water tank"), with past dates allowed.

Measurements can be metric or imperial. Type `12' 6"` or `3.8 m`, whichever
you prefer.

## Your file

Each house is a single `.abode` file. By default it's saved in
`Documents/Abode`, but you can keep it anywhere. Back it up the way you back
up any other document. Every save keeps a snapshot, so you can go back to an
earlier version from the timeline.

You're never locked in. **⋯ → Export as documents…** saves everything as
ordinary files that open without Abode: a readable page for each floor, the
yard, gardens, roof, paint, reminders and history, drawings of the plans,
every attached file, and all the data as JSON for other apps. Choose
**Everything, for me** to keep a full copy for your own records, or **For a
new owner** to leave out anything private.

Abode doesn't upload your house anywhere. The only thing it does online is
check GitHub for a new version when it opens, and you can turn that off in
Settings.

## Handing the house over

Mark notes, service records or custom fields as **Private** to keep them to
yourself. Then use **⋯ → Export copy for a new owner…**. The copy leaves out
everything private, your vehicles, your seed packets and the save history, so
the new owner gets the house without your personal details. What was planted
in the gardens stays, so they know what's in the ground. If they don't use
Abode, give them **Export as documents… → For a new owner** as well (or
instead): the same information as pages anyone can read.

## Updates

When there's a new version, Abode shows an **Update** button on the start
screen and in the sidebar. Click it to install the update and restart. Your
house files aren't changed. You can also check yourself with
**⋯ → Check for updates…**.

## Contributing

Bug reports and ideas are welcome as
[GitHub issues](https://github.com/asmorris/abode/issues). Screenshots help.
If you can, include the steps that led to the problem.

To work on the code:

1. Install Rust from [rustup.rs](https://rustup.rs).
2. Run it with `cargo run --release -p abode-app -- --sample`.
3. Before opening a pull request, run `cargo test --workspace` and
   `cargo clippy --workspace --all-targets`.

Changes go into `main` through pull requests. CI builds and tests each one on
Mac and Windows. [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) covers how the
code is organised, the file format and how releases are made.
