//! A small example house, for trying the app and for tests.

use crate::geometry::{Vec2, rect_points};
use crate::model::*;
use crate::garden::{GardenSettings, Harvest, Layout, Method, Planting, SeedPacket};
use crate::site::FeatureKind;
use chrono::{Datelike, Local, NaiveDate};

pub fn sample_house() -> Project {
    let mut p = Project::new("Sample House");
    p.info.address = "123 Example Street".into();
    p.info.year_built = Some(1994);
    let l = p.levels[0].id;

    let room = |p: &mut Project, name: &str, kind: &str, x: f64, y: f64, w: f64, h: f64| {
        let id = p.add_room(l, name, &rect_points(Vec2::new(x, y), w, h)).expect("sample room");
        let r = p.levels[0].rooms.get_mut(&id).unwrap();
        r.kind = kind.into();
        r.shape_locked = true;
        id
    };
    let living = room(&mut p, "Living room", "Living room", 0.0, 0.0, 5.0, 4.0);
    let kitchen = room(&mut p, "Kitchen", "Kitchen", 5.0, 0.0, 4.0, 4.0);
    let _hall = room(&mut p, "Hallway", "Hallway", 0.0, 4.0, 9.0, 1.2);
    let bedroom = room(&mut p, "Bedroom", "Bedroom", 0.0, 5.2, 4.5, 3.5);
    let bath = room(&mut p, "Bathroom", "Bathroom", 4.5, 5.2, 2.5, 2.5);
    let utility = room(&mut p, "Utility", "Utility / mechanical", 7.0, 5.2, 2.0, 3.5);

    let door = |p: &mut Project, at: Vec2, style: DoorStyle| {
        let (wall, offset, _) = p.levels[0].nearest_wall(at, 0.2).expect("wall for door");
        p.add_opening(l, wall, OpeningKind::Door(style), offset).unwrap()
    };
    let window = |p: &mut Project, at: Vec2| {
        let (wall, offset, _) = p.levels[0].nearest_wall(at, 0.2).expect("wall for window");
        p.add_opening(l, wall, OpeningKind::Window(WindowStyle::DoubleHung), offset).unwrap()
    };
    let front = door(&mut p, Vec2::new(2.5, 0.0), DoorStyle::Hinged);
    p.levels[0].openings.get_mut(&front).unwrap().label = "Front door".into();
    door(&mut p, Vec2::new(2.0, 4.0), DoorStyle::Double);
    door(&mut p, Vec2::new(6.5, 4.0), DoorStyle::Hinged);
    door(&mut p, Vec2::new(2.0, 5.2), DoorStyle::Hinged);
    door(&mut p, Vec2::new(5.5, 5.2), DoorStyle::Pocket);
    door(&mut p, Vec2::new(8.0, 5.2), DoorStyle::Hinged);
    window(&mut p, Vec2::new(0.0, 2.0));
    window(&mut p, Vec2::new(7.0, 0.0));
    window(&mut p, Vec2::new(2.25, 8.7));
    window(&mut p, Vec2::new(5.75, 7.7));

    // Stairs up the side of the living room, to the landing upstairs.
    p.add_object(l, "Stairs", ObjectCategory::Fixture, Shape::new(ShapeKind::Rectangle, 0.9, 3.0), Vec2::new(4.5, 2.2));
    let up = sample_upstairs(&mut p);

    let rect = |w, d| Shape::new(ShapeKind::Rectangle, w, d);
    let fridge = p
        .add_object(l, "Fridge", ObjectCategory::Appliance, rect(0.9, 0.75), Vec2::new(8.4, 0.5))
        .unwrap();
    p.add_object(l, "Stove", ObjectCategory::Appliance, rect(0.76, 0.66), Vec2::new(6.5, 0.45));
    let furnace = p
        .add_object(l, "Furnace", ObjectCategory::Hvac, rect(0.56, 0.74), Vec2::new(8.4, 6.0))
        .unwrap();
    p.add_object(
        l,
        "Water heater",
        ObjectCategory::Plumbing,
        Shape::new(ShapeKind::Circle, 0.56, 0.56),
        Vec2::new(7.5, 6.0),
    );
    p.add_object(l, "Washer", ObjectCategory::Appliance, rect(0.69, 0.71), Vec2::new(7.45, 8.2));
    p.add_object(l, "Dryer", ObjectCategory::Appliance, rect(0.69, 0.71), Vec2::new(8.5, 8.2));
    p.add_object(l, "Bathtub", ObjectCategory::Plumbing, rect(1.52, 0.76), Vec2::new(5.75, 7.25));
    let vanity = p.add_object(l, "Vanity", ObjectCategory::Plumbing, rect(0.9, 0.5), Vec2::new(6.5, 5.5)).unwrap();
    {
        let hw = |kind: &str, n: u32, brand: &str, model: &str, part: &str, finish: &str, size: &str, vendor: &str| Hardware {
            kind: kind.into(),
            quantity: Some(n),
            brand: brand.into(),
            model: model.into(),
            part_number: part.into(),
            finish: finish.into(),
            size: size.into(),
            vendor: vendor.into(),
            ..Default::default()
        };
        let v = &mut p.levels[0].objects.get_mut(&vanity).unwrap().details;
        v.make = "Glacier Bay".into();
        v.model = "Bannister 36\" vanity".into();
        v.hardware = vec![
            hw("Cabinet pull", 4, "Amerock", "Bar Pulls", "BP36550FB", "Matte black", "128 mm centre to centre", "Home Depot"),
            hw("Cabinet hinge", 4, "Blum", "Compact 33 soft-close", "33.3600", "Nickel", "Full overlay", "Lee Valley"),
            hw("Faucet", 1, "Moen", "Align", "6190BL", "Matte black", "Single hole", "Home Depot"),
        ];
        let b = p.levels[0].rooms.get_mut(&bath).unwrap();
        b.hardware = vec![
            hw("Towel bar", 1, "Moen", "Align", "YB0424BL", "Matte black", "24 in", "Home Depot"),
            hw("Toilet paper holder", 1, "Moen", "Align", "YB0408BL", "Matte black", "", "Home Depot"),
            hw("Robe hook", 2, "Moen", "Align", "YB0403BL", "Matte black", "", "Home Depot"),
        ];
        let d = &mut p.levels[0].openings.get_mut(&front).unwrap().details;
        d.hardware = vec![
            Hardware { notes: "Keyed alike with the back door. Spare keys in the kitchen drawer.".into(), ..hw("Door handle", 1, "Schlage", "Camelot handleset", "F60 V CAM 619", "Satin nickel", "", "Lowe's") },
            hw("Door hinge", 3, "National Hardware", "Residential hinge", "N830-191", "Satin nickel", "3.5 in, ⅝ in radius", "Lowe's"),
        ];
    }

    {
        let f = &mut p.levels[0].objects.get_mut(&furnace).unwrap().details;
        f.make = "Carrier".into();
        f.model = "59SC5".into();
        f.installed = NaiveDate::from_ymd_opt(2018, 10, 2);
        f.warranty_expires = NaiveDate::from_ymd_opt(2028, 10, 2);
    }
    {
        let f = &mut p.levels[0].objects.get_mut(&fridge).unwrap().details;
        f.make = "LG".into();
        f.purchased = NaiveDate::from_ymd_opt(2021, 3, 14);
    }

    let mut n = Note::new(EntityRef::Object(furnace));
    n.title = "Filter size".into();
    n.body = "16 x 25 x 1, MERV 11. Spare filters are on the shelf above the dryer.".into();
    p.notes.push(n);
    let mut n = Note::new(EntityRef::General);
    n.title = "Water shut-off".into();
    n.body = "Main shut-off valve is in the utility room, behind the water heater.".into();
    p.notes.push(n);

    let today = Local::now().date_naive();
    let mut r = Reminder::new(EntityRef::Object(furnace), "Replace furnace filter", today);
    r.repeat = Some(Recurrence { every: 3, unit: RecurrenceUnit::Months });
    p.reminders.push(r);
    let mut s = ServiceRecord::new(EntityRef::Object(furnace), NaiveDate::from_ymd_opt(2025, 10, 15).unwrap());
    s.kind = ServiceKind::Inspection;
    s.performed_by = "Comfort Heating Co.".into();
    s.description = "Annual tune-up, cleaned burners".into();
    s.cost = "$129".into();
    p.service_records.push(s);

    let mut paint = Paint::new(PaintTarget::Room(living));
    paint.brand = "Benjamin Moore".into();
    paint.color_name = "Pale Oak".into();
    paint.color_code = "OC-20".into();
    paint.swatch = Some([221, 212, 197]);
    paint.leftover_location = "Garage shelf".into();
    p.paints.push(paint);
    let mut paint = Paint::for_surface(PaintTarget::Room(living), PaintSurface::Ceiling);
    paint.brand = "Benjamin Moore".into();
    paint.color_name = "Ceiling White".into();
    paint.color_code = "CW-1".into();
    paint.swatch = Some([244, 243, 237]);
    paint.leftover_location = "Garage shelf".into();
    p.paints.push(paint);
    let mut paint = Paint::new(PaintTarget::Room(bedroom));
    paint.color_name = "Hale Navy".into();
    paint.color_code = "HC-154".into();
    paint.swatch = Some([66, 76, 92]);
    p.paints.push(paint);
    let _ = (kitchen, bath, utility);

    // ---- Roofs: the main gable roof, and a lower shed roof over the deck.
    let main_roof = p.add_roof(up, &rect_points(Vec2::ZERO, 9.0, 8.7), RoofShape::Gable).unwrap();
    {
        let r = p.level_mut(up).unwrap().roofs.get_mut(&main_roof).unwrap();
        r.name = "Main roof".into();
        r.pitch = 6.0 / 12.0;
        r.direction = 0.0;
        r.layers = vec![RoofLayer {
            material: "Asphalt shingles (architectural)".into(),
            color: "Weathered wood".into(),
            swatch: Some([112, 104, 96]),
            installed: NaiveDate::from_ymd_opt(2015, 6, 1),
            installed_by: "Summit Roofing".into(),
            warranty_expires: NaiveDate::from_ymd_opt(2045, 6, 1),
            warranty_details: "30-year manufacturer warranty, 10 years on workmanship".into(),
            ..Default::default()
        }];
        r.last_inspected = NaiveDate::from_ymd_opt(2024, 5, 20);
    }
    let porch_roof = p
        .add_roof(l, &rect_points(Vec2::new(0.0, 8.7), 4.5, 2.6), RoofShape::Shed)
        .unwrap();
    {
        let r = p.levels[0].roofs.get_mut(&porch_roof).unwrap();
        r.name = "Deck roof".into();
        r.pitch = 2.0 / 12.0;
        r.direction = std::f64::consts::FRAC_PI_2; // slopes away from the house
        r.overhang = 0.15;
        r.eave_height = Some(2.0);
        r.layers = vec![
            RoofLayer {
                material: "Rolled roofing".into(),
                installed: NaiveDate::from_ymd_opt(2005, 7, 1),
                notes: "Original layer, left in place when the metal went on.".into(),
                ..Default::default()
            },
            RoofLayer {
                material: "Metal (standing seam)".into(),
                color: "Charcoal".into(),
                swatch: Some([70, 74, 78]),
                installed: NaiveDate::from_ymd_opt(2019, 8, 12),
                installed_by: "Northside Metal Roofing".into(),
                ..Default::default()
            },
        ];
    }
    let rect = |w, d| Shape::new(ShapeKind::Rectangle, w, d);
    p.add_roof_object(up, "Chimney", ObjectCategory::Roof, rect(0.7, 0.7), Vec2::new(1.2, 1.8));
    p.add_roof_object(up, "Skylight", ObjectCategory::Roof, rect(0.6, 1.2), Vec2::new(6.6, 6.4));
    p.add_roof_object(up, "Front gutter", ObjectCategory::Roof, rect(9.4, 0.15), Vec2::new(4.5, -0.22));
    p.add_roof_object(up, "Downspout", ObjectCategory::Roof, rect(0.12, 0.12), Vec2::new(9.12, -0.22));
    let mut s = ServiceRecord::new(EntityRef::RoofSection(main_roof), NaiveDate::from_ymd_opt(2024, 5, 20).unwrap());
    s.kind = ServiceKind::Inspection;
    s.performed_by = "Summit Roofing".into();
    s.description = "Spring inspection, resealed flashing around the chimney".into();
    p.service_records.push(s);
    let mut r = Reminder::new(EntityRef::RoofSection(main_roof), "Clean the gutters", today);
    r.repeat = Some(Recurrence { every: 6, unit: RecurrenceUnit::Months });
    p.reminders.push(r);

    // ---- Outside: the lot, deck, patio, driveway, fence, trees and a shed.
    let site = p.site_id().unwrap();
    let feature = |p: &mut Project, kind: FeatureKind, pts: Vec<Vec2>, name: &str, material: &str| {
        let id = p.add_feature(kind, pts).expect("sample feature");
        let f = p.levels.iter_mut().find(|l| l.is_site()).unwrap().features.get_mut(&id).unwrap();
        f.name = name.into();
        f.material = material.into();
        id
    };
    feature(&mut p, FeatureKind::PropertyLine, rect_points(Vec2::new(-6.0, -8.0), 28.0, 28.0), "Lot 14", "");
    feature(&mut p, FeatureKind::Lawn, rect_points(Vec2::new(-5.5, 12.0), 27.0, 7.5), "Back lawn", "Grass");
    let deck = feature(&mut p, FeatureKind::Deck, rect_points(Vec2::new(0.0, 8.7), 4.5, 2.6), "Back deck", "Pressure-treated pine");
    {
        let f = p.levels.iter_mut().find(|l| l.is_site()).unwrap().features.get_mut(&deck).unwrap();
        f.height = 0.7;
        f.railing_material = "Aluminium, black".into();
        f.stairs = "4 steps down to the lawn, east side".into();
        f.details.installed = NaiveDate::from_ymd_opt(2005, 6, 1);
    }
    feature(&mut p, FeatureKind::Patio, rect_points(Vec2::new(5.0, 8.7), 4.0, 3.0), "Patio", "Pavers");
    feature(&mut p, FeatureKind::Driveway, rect_points(Vec2::new(9.8, -8.0), 3.6, 8.6), "Driveway", "Asphalt");
    feature(&mut p, FeatureKind::Path, rect_points(Vec2::new(2.0, -8.0), 1.2, 8.0), "Front walk", "Concrete");
    feature(&mut p, FeatureKind::GardenBed, rect_points(Vec2::new(-1.5, -1.6), 3.2, 1.2), "Front bed", "Stone edging");
    let fence = feature(
        &mut p,
        FeatureKind::Fence,
        vec![Vec2::new(-6.0, 4.0), Vec2::new(-6.0, 20.0), Vec2::new(22.0, 20.0), Vec2::new(22.0, 4.0), Vec2::new(13.6, 4.0)],
        "Back fence",
        "Wood (privacy)",
    );
    {
        let f = p.levels.iter_mut().find(|l| l.is_site()).unwrap().features.get_mut(&fence).unwrap();
        f.shared_with_neighbour = true;
        f.neighbour = "Neighbours to the east (no. 125)".into();
        let mut g = Gate::new(56.0);
        g.label = "Side gate".into();
        g.width = 1.2;
        f.gates.push(g);
        f.details.installed = NaiveDate::from_ymd_opt(2012, 9, 1);
    }
    let tree = |p: &mut Project, at: Vec2, name: &str, size: f64| {
        let id = feature(p, FeatureKind::Tree, vec![at], name, "");
        p.levels.iter_mut().find(|l| l.is_site()).unwrap().features.get_mut(&id).unwrap().size = size;
    };
    tree(&mut p, Vec2::new(-3.0, 15.0), "Sugar maple", 7.0);
    tree(&mut p, Vec2::new(20.0, 10.5), "Blue spruce", 3.5);
    tree(&mut p, Vec2::new(-3.5, -4.5), "Paper birch", 4.0);
    sample_garden(&mut p, today);
    sample_pets(&mut p, today);
    p.add_object(site, "BBQ", ObjectCategory::Outdoor, rect(1.3, 0.6), Vec2::new(3.4, 10.6));
    p.add_object(site, "Patio table", ObjectCategory::Furniture, Shape::new(ShapeKind::Circle, 1.2, 1.2), Vec2::new(7.0, 10.2));
    let ac = p.add_object(site, "Air conditioner", ObjectCategory::Hvac, rect(0.8, 0.8), Vec2::new(9.7, 6.4)).unwrap();
    p.add_object(site, "Hose tap", ObjectCategory::Plumbing, Shape::new(ShapeKind::Circle, 0.2, 0.2), Vec2::new(-0.25, 6.0));

    let mut paint = Paint::new(PaintTarget::Feature(deck));
    paint.surface = PaintSurface::Decking;
    paint.finish = PaintFinish::Stain;
    paint.brand = "Cabot".into();
    paint.color_name = "Semi-transparent Cedar".into();
    paint.swatch = Some([176, 112, 72]);
    paint.applied = NaiveDate::from_ymd_opt(2024, 6, 8);
    paint.leftover_location = "Shed, top shelf".into();
    p.paints.push(paint);
    let mut r = Reminder::new(EntityRef::Feature(deck), "Re-stain the deck", NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
    r.repeat = Some(Recurrence { every: 2, unit: RecurrenceUnit::Years });
    p.reminders.push(r);
    let mut n = Note::new(EntityRef::Feature(fence));
    n.title = "Shared fence".into();
    n.body = "The east side is shared with no. 125; we split the cost of the 2012 rebuild.".into();
    p.notes.push(n);
    let mut r = Reminder::new(EntityRef::Object(ac), "Clear leaves from the AC unit", today);
    r.repeat = Some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
    p.reminders.push(r);

    // ---- A car under the carport on the driveway.
    feature(&mut p, FeatureKind::Carport, rect_points(Vec2::new(9.8, -5.2), 3.6, 5.4), "Carport", "Aluminium");
    let car = p.add_object(site, "SUV / crossover", ObjectCategory::Vehicle, rect(1.9, 4.6), Vec2::new(11.6, -2.5)).unwrap();
    {
        use crate::vehicle::*;
        let o = p.levels.iter_mut().find_map(|l| l.objects.get_mut(&car)).unwrap();
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day);
        o.details.make = "Toyota".into();
        o.details.model = "RAV4 Prime".into();
        o.details.purchased = d(2021, 5, 14);
        o.details.price = "$36,450".into();
        o.details.vendor = "Lakeside Toyota".into();
        let v = o.vehicle.as_mut().unwrap();
        v.year = "2021".into();
        v.trim = "XSE".into();
        v.color = "Lunar Rock".into();
        v.vin = "2T3P1RFV5MW000000".into();
        v.plate = "CKRT 214".into();
        v.fuel = Fuel::PlugInHybrid;
        v.battery = "18.1 kWh".into();
        v.range = "68 km on electric".into();
        v.connector = "J1772".into();
        v.cable = "Toyota charging cable, in the cargo floor".into();
        v.engine = "2.5L 4-cylinder + 2 electric motors".into();
        v.transmission = "Automatic".into();
        v.drivetrain = "AWD".into();
        v.odometer = Some(58400.0);
        v.odometer_date = Some(today);
        v.purchase_km = Some(12.0);
        v.ownership = Ownership::Financed;
        v.lender = "Toyota Financial".into();
        v.payment = "$512 / month".into();
        v.term_ends = d(2027, 5, 14);
        v.registration_expires = today.checked_add_days(chrono::Days::new(45));
        v.insurer = "Northwind Mutual".into();
        v.insurance_renews = d(2027, 3, 1);
        v.keys = "2 fobs; spare in the hall drawer".into();
        v.warranties = vec![
            VehicleWarranty { name: "Basic (bumper-to-bumper)".into(), provider: "Toyota".into(), starts: d(2021, 5, 14), expires: d(2024, 5, 14), until_km: Some(60000.0), ..Default::default() },
            VehicleWarranty { name: "Powertrain".into(), provider: "Toyota".into(), starts: d(2021, 5, 14), expires: d(2026, 5, 14), until_km: Some(100000.0), ..Default::default() },
            VehicleWarranty { name: "Corrosion / rust".into(), provider: "Toyota".into(), starts: d(2021, 5, 14), expires: d(2026, 5, 14), ..Default::default() },
            VehicleWarranty { name: "Extended warranty".into(), provider: "Toyota Extra Care".into(), contract: "XC-55120".into(), expires: d(2028, 5, 14), until_km: Some(160000.0), deductible: "$100".into(), ..Default::default() },
        ];
        v.tires = vec![
            TireSet { season: TireSeason::AllSeason, brand: "Michelin".into(), model: "CrossClimate 2".into(), size: "225/65R17 102H".into(), installed: d(2025, 4, 12), installed_km: Some(41200.0), mounted: true, warranty_km: Some(80000.0), ..Default::default() },
            TireSet { season: TireSeason::Winter, brand: "Bridgestone".into(), model: "Blizzak DM-V2".into(), size: "225/65R17 102S".into(), stored_at: "Garage loft".into(), rims: "Own steel rims".into(), ..Default::default() },
        ];
        v.parts = vec![
            VehiclePart { name: "Engine oil".into(), spec: "0W-16 full synthetic".into(), capacity: "4.8 qt".into(), ..Default::default() },
            VehiclePart { name: "Oil filter".into(), part_number: "04152-YZZA6".into(), ..Default::default() },
            VehiclePart { name: "Wiper blades".into(), spec: "26\" driver, 16\" passenger, 12\" rear".into(), ..Default::default() },
            VehiclePart { name: "Cabin air filter".into(), part_number: "87139-0E040".into(), ..Default::default() },
        ];
    }
    let charger = p.add_object(site, "EV charger", ObjectCategory::Electrical, rect(0.35, 0.2), Vec2::new(13.2, -4.6)).unwrap();
    if let Some(o) = p.levels.iter_mut().find_map(|l| l.objects.get_mut(&charger)) {
        o.details.make = "Grizzl-E".into();
        o.details.model = "Classic".into();
        o.details.installed = NaiveDate::from_ymd_opt(2024, 9, 3);
        for (k, v) in [("Level", "Level 2"), ("Power", "40 A (9.6 kW)"), ("Connector", "J1772"), ("Breaker", "50 A double-pole, panel slots 17-19")] {
            if let Some(f) = o.details.custom_fields.iter_mut().find(|f| f.key == k) {
                f.value = v.into();
            }
        }
    }
    if let Some(v) = p.levels.iter_mut().find_map(|l| l.objects.get_mut(&car)).and_then(|o| o.vehicle.as_mut()) {
        v.charger = Some(charger);
    }
    let mut s = ServiceRecord::new(EntityRef::Object(car), NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
    s.performed_by = "Lakeside Toyota".into();
    s.description = "Oil & filter change, tire rotation, multi-point inspection".into();
    s.cost = "$129".into();
    s.odometer = Some(51000.0);
    p.service_records.push(s);
    let mut r = Reminder::new(EntityRef::Object(car), "Oil & filter change", NaiveDate::from_ymd_opt(2026, 10, 20).and_then(|d| d.checked_add_months(chrono::Months::new(6))).unwrap());
    r.due_by = DueBy::Either;
    r.repeat = Some(Recurrence { every: 6, unit: RecurrenceUnit::Months });
    r.due_km = Some(59000.0);
    r.every_km = Some(8000.0);
    p.reminders.push(r);
    p.add_tire_swap_reminders(car, today);

    let (shed, shed_level) = p
        .add_building("Shed", "Shed", &rect_points(Vec2::new(15.5, 14.0), 3.0, 2.4))
        .expect("sample shed");
    if let Some(b) = p.building_mut(shed) {
        b.details.installed = NaiveDate::from_ymd_opt(2016, 5, 1);
        b.details.vendor = "Backyard Buildings Co.".into();
    }
    {
        let (wall, offset, _) = p.level(shed_level).unwrap().nearest_wall(Vec2::new(17.0, 14.0), 0.2).unwrap();
        p.add_opening(shed_level, wall, OpeningKind::Door(DoorStyle::Double), offset);
        let (wall, offset, _) = p.level(shed_level).unwrap().nearest_wall(Vec2::new(18.5, 15.2), 0.2).unwrap();
        p.add_opening(shed_level, wall, OpeningKind::Window(WindowStyle::Fixed), offset);
    }
    let mower = p
        .add_object(shed_level, "Lawn mower", ObjectCategory::Outdoor, rect(0.55, 1.0), Vec2::new(16.2, 15.4))
        .unwrap();
    if let Some(o) = p.levels.iter_mut().find_map(|l| l.objects.get_mut(&mower)) {
        o.details.make = "Honda".into();
        o.details.model = "HRN216".into();
    }
    let mut r = Reminder::new(EntityRef::Object(mower), "Change the mower oil and sharpen the blade", NaiveDate::from_ymd_opt(2027, 4, 1).unwrap());
    r.repeat = Some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
    p.reminders.push(r);
    p.add_roof(shed_level, &rect_points(Vec2::new(15.5, 14.0), 3.0, 2.4), RoofShape::Gable);
    p.changelog.clear();
    let entry = |summary: &str, details: &str, date: Option<NaiveDate>| {
        let mut e = ChangeEntry::manual("Previous owner", summary, date);
        e.details = details.into();
        e
    };
    p.changelog.push(entry("Moved in", "Bought the house from the original owners.", NaiveDate::from_ymd_opt(2012, 8, 1)));
    let mut roof = entry("New roof", "Full tear-off of the main roof, 30-year architectural shingles.", NaiveDate::from_ymd_opt(2015, 6, 1));
    roof.targets.push(EntityRef::Roof);
    roof.targets.push(EntityRef::RoofSection(main_roof));
    p.changelog.push(roof);
    let mut f = entry("Replaced the furnace", "Old unit failed in October; replaced with a high-efficiency Carrier.", NaiveDate::from_ymd_opt(2018, 10, 2));
    f.targets.push(EntityRef::Object(furnace));
    p.changelog.push(f);
    p.changelog.push(entry("Repainted the living room and bedroom", "", NaiveDate::from_ymd_opt(2023, 4, 15)));
    p
}

/// A vegetable garden out back with this year's plan and last year's (so
/// rotation hints show), strawberries in the front bed, and some seeds.
/// The upper floor, over the whole house: a landing at the top of the
/// stairs, a studio with an angled wall, a library with a round reading
/// room set into it, and bedrooms and a bathroom along the back.
fn sample_upstairs(p: &mut Project) -> crate::ids::LevelId {
    let up = p.add_level(None, "Upper floor");
    let pts = |v: &[(f64, f64)]| v.iter().map(|(x, y)| Vec2::new(*x, *y)).collect::<Vec<_>>();
    let room = |p: &mut Project, name: &str, kind: &str, outline: Vec<Vec2>| {
        let id = p.add_room(up, name, &outline).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let r = p.level_mut(up).unwrap().rooms.get_mut(&id).unwrap();
        r.kind = kind.into();
        r.shape_locked = true;
        id
    };
    room(p, "Studio", "Office", pts(&[(0.0, 0.0), (3.6, 0.0), (3.6, 1.5), (2.8, 3.2), (2.8, 4.0), (0.0, 4.0)]));
    room(
        p,
        "Landing",
        "Hallway",
        pts(&[(3.6, 0.0), (5.4, 0.0), (5.4, 4.0), (9.0, 4.0), (9.0, 5.2), (0.0, 5.2), (0.0, 4.0), (2.8, 4.0), (2.8, 3.2), (3.6, 1.5)]),
    );
    room(p, "Library", "Library", rect_points(Vec2::new(5.4, 0.0), 3.6, 4.0));
    // Drawn inside the library and just over its wall, so it takes a flat
    // side there, where its door opens onto the landing.
    let reading = p.add_round_room(up, "Reading room", Vec2::new(7.2, 2.6), 1.6).expect("sample round room");
    p.level_mut(up).unwrap().rooms.get_mut(&reading).unwrap().shape_locked = true;
    // Along the back, over the rooms below (leaving the gap behind the bathroom open).
    room(p, "Main bedroom", "Bedroom", rect_points(Vec2::new(0.0, 5.2), 4.5, 3.5));
    room(p, "Upstairs bath", "Bathroom", rect_points(Vec2::new(4.5, 5.2), 2.5, 2.5));
    room(p, "Guest room", "Bedroom", rect_points(Vec2::new(7.0, 5.2), 2.0, 3.5));

    let opening = |p: &mut Project, at: Vec2, kind: OpeningKind| {
        let (wall, offset, _) = p.level(up).unwrap().nearest_wall(at, 0.2).expect("wall for opening");
        p.add_opening(up, wall, kind, offset).unwrap();
    };
    for at in [(3.2, 2.35), (7.2, 4.0), (5.4, 0.7), (2.25, 5.2), (5.75, 5.2), (8.0, 5.2)] {
        opening(p, Vec2::new(at.0, at.1), OpeningKind::Door(DoorStyle::Hinged));
    }
    for at in [(0.0, 2.0), (1.8, 0.0), (7.2, 0.0), (2.25, 8.7), (5.75, 7.7), (9.0, 7.0)] {
        opening(p, Vec2::new(at.0, at.1), OpeningKind::Window(WindowStyle::DoubleHung));
    }
    up
}

fn sample_garden(p: &mut Project, today: NaiveDate) {
    let y = today.year();
    let d = |year: i32, m: u32, day: u32| NaiveDate::from_ymd_opt(year, m, day);
    let past = |date: Option<NaiveDate>| date.filter(|x| *x <= today);
    p.garden = GardenSettings { last_frost: d(2000, 5, 10), first_frost: d(2000, 10, 10), zone: "6a".into() };
    let Some(bed) = p.add_feature(FeatureKind::GardenBed, rect_points(Vec2::new(4.0, 13.2), 6.0, 3.4)) else { return };
    let front = p.site().and_then(|l| l.features.values().find(|f| f.name == "Front bed").map(|f| f.id));
    if let Some(f) = p.levels.iter_mut().find_map(|l| l.features.get_mut(&bed)) {
        f.name = "Vegetable garden".into();
        f.material = "Raised cedar".into();
        f.height = 0.3;
    }

    let mut seed = |crop: &str, variety: &str, source: &str, packed: i32, left: &str| {
        let mut s = SeedPacket::new(crop);
        s.variety = variety.into();
        s.source = source.into();
        s.packed_for = Some(packed);
        s.amount = left.into();
        let id = s.id;
        p.seeds.push(s);
        id
    };
    let sungold = seed("Tomato", "Sungold", "Johnny's Selected Seeds", y - 1, "about 15 seeds");
    let nantes = seed("Carrot", "Scarlet Nantes", "West Coast Seeds", y - 3, "half a packet");
    let genovese = seed("Basil", "Genovese", "Botanical Interests", y, "most of a packet");
    let provider = seed("Bush bean", "Provider", "Saved from last year", y - 1, "a jar, about 200");
    seed("Lettuce", "Buttercrunch", "Hardware store", y - 1, "");
    if let Some(s) = p.seeds.iter_mut().find(|s| s.variety == "Buttercrunch") {
        s.used_up = true;
    }

    let row = |year: i32, crop: &str, variety: &str, yy: f64, x0: f64, x1: f64| {
        let mut pl = Planting::new(bed, year, Layout::Row, vec![Vec2::new(x0, yy), Vec2::new(x1, yy)], crop);
        pl.variety = variety.into();
        pl
    };
    // This year
    let mut tomato = row(y, "Tomato", "Sungold", 13.6, 4.4, 9.6);
    tomato.seed = Some(sungold);
    tomato.started = past(d(y, 3, 20));
    tomato.planted = past(d(y, 5, 22));
    for (m, day, kg) in [(8, 4, 1.2), (8, 18, 2.6), (9, 2, 3.1)] {
        if let Some(date) = past(d(y, m, day)) {
            tomato.harvests.push(Harvest { date, amount: Some(kg), unit: "kg".into(), notes: String::new() });
        }
    }
    let tomato_id = tomato.id;
    let mut basil = row(y, "Basil", "Genovese", 14.1, 4.4, 7.0);
    basil.seed = Some(genovese);
    basil.method = Method::StartedIndoors;
    basil.started = past(d(y, 4, 5));
    basil.planted = past(d(y, 5, 28));
    let mut carrots = row(y, "Carrot", "Scarlet Nantes", 14.6, 4.4, 9.6);
    carrots.seed = Some(nantes);
    carrots.planted = past(d(y, 4, 25));
    let mut lettuce = Planting::new(bed, y, Layout::Patch, rect_points(Vec2::new(4.4, 15.0), 1.6, 0.8), "Lettuce");
    lettuce.variety = "Buttercrunch".into();
    lettuce.planted = past(d(y, 4, 15));
    lettuce.removed = past(d(y, 7, 1));
    let mut beans = row(y, "Bush bean", "Provider", 15.4, 6.4, 9.6);
    beans.seed = Some(provider);
    beans.planted = past(d(y, 5, 25));
    // Planted last fall, comes out this summer.
    let mut garlic = Planting::new(bed, y - 1, Layout::Patch, rect_points(Vec2::new(4.4, 15.95), 5.2, 0.45), "Garlic");
    garlic.variety = "Music".into();
    garlic.planted = d(y - 1, 10, 20);
    if let Some(date) = past(d(y, 7, 15)) {
        garlic.removed = Some(date);
        garlic.harvests.push(Harvest { date, amount: Some(60.0), unit: "heads".into(), notes: "Cured in the shed for 3 weeks".into() });
    }
    for (i, x) in [7.6, 8.4, 9.2].into_iter().enumerate() {
        let mut m = Planting::new(bed, y, Layout::Plant, vec![Vec2::new(x, 14.1)], "Marigold");
        m.method = Method::Bought;
        m.planted = past(d(y, 5, 24 + i as u32));
        p.plantings.push(m);
    }
    // Last year: potatoes where the tomatoes are now.
    let mut potato = row(y - 1, "Potato", "Yukon Gold", 13.6, 4.4, 9.6);
    potato.planted = d(y - 1, 5, 1);
    potato.removed = d(y - 1, 8, 20);
    potato.harvests.push(Harvest { date: NaiveDate::from_ymd_opt(y - 1, 8, 20).unwrap(), amount: Some(14.0), unit: "kg".into(), notes: "Good year".into() });
    let mut old_beans = row(y - 1, "Bush bean", "Provider", 14.6, 4.4, 9.6);
    old_beans.planted = d(y - 1, 5, 28);
    old_beans.removed = d(y - 1, 9, 10);
    let mut zucchini = Planting::new(bed, y - 1, Layout::Plant, vec![Vec2::new(8.6, 15.6)], "Zucchini");
    zucchini.size = 1.0;
    zucchini.planted = d(y - 1, 6, 1);
    zucchini.removed = d(y - 1, 9, 25);
    p.plantings.extend([tomato, basil, carrots, lettuce, beans, garlic, potato, old_beans, zucchini]);

    // Strawberries in the front bed, there for a few years now.
    if let Some(front) = front {
        let mut s = Planting::new(front, y - 2, Layout::Patch, rect_points(Vec2::new(-1.35, -1.45), 2.2, 0.9), "Strawberry");
        s.variety = "Honeoye".into();
        s.planted = d(y - 2, 5, 5);
        p.plantings.push(s);
        let mut chives = Planting::new(front, y - 2, Layout::Plant, vec![Vec2::new(1.25, -1.0)], "Chives");
        chives.planted = d(y - 2, 5, 5);
        p.plantings.push(chives);
    }

    let mut water = Reminder::new(EntityRef::Feature(bed), "Water deeply", today);
    water.repeat = Some(Recurrence { every: 3, unit: RecurrenceUnit::Days });
    water.notes = "Soaker hose on the timer by the tap: 30 minutes.".into();
    p.reminders.push(water);
    let mut seeds = Reminder::new(EntityRef::Feature(bed), "Order seeds", crate::garden::next_on(today, 1, 15));
    seeds.repeat = Some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
    p.reminders.push(seeds);
    let mut n = Note::new(EntityRef::Planting(tomato_id));
    n.title = "Staking".into();
    n.body = "Florida weave between the T-posts worked better than cages. Prune suckers below the first flower.".into();
    p.notes.push(n);
}

fn sample_pets(p: &mut Project, today: NaiveDate) {
    use crate::pet::*;
    let days = |n: i64| today + chrono::Duration::days(n);
    let months_ago = |n: u32| today.checked_sub_months(chrono::Months::new(n)).unwrap_or(today);
    let years_ago = |n: u32| months_ago(12 * n);
    let clinic = Vet {
        clinic: "Maple Grove Animal Hospital".into(),
        vet: "Dr. Priya Raman".into(),
        phone: "(555) 201-4410".into(),
        email: "frontdesk@maplegrovevet.example".into(),
        address: "88 Maple Grove Rd".into(),
        notes: "Mon–Sat 8–6. Client #20431. Online portal for records.".into(),
    };
    let er = Vet {
        clinic: "Lakeside 24-Hour Emergency Vet".into(),
        phone: "(555) 330-9111".into(),
        address: "1200 Lakeside Ave".into(),
        notes: "Open 24/7. Call ahead so they're ready.".into(),
        ..Default::default()
    };

    // A dog with a full record.
    let dog = p.add_pet("Biscuit", Species::Dog);
    {
        let b = p.pet_mut(dog).unwrap();
        b.breed = "Golden Retriever".into();
        b.sex = Sex::Female;
        b.fixed = true;
        b.birthday = Some(years_ago(5) - chrono::Duration::days(40));
        b.adopted = Some(years_ago(5) + chrono::Duration::days(30));
        b.adopted_from = "Sunny Acres Goldens (breeder)".into();
        b.markings = "Light gold, white patch on chest".into();
        b.microchip = "985 141 000 123 456".into();
        b.microchip_registry = "PetLink".into();
        b.licence = "City tag #4471".into();
        b.licence_renews = Some(days(52));
        b.food = "Purina Pro Plan Large Breed Adult, salmon".into();
        b.feeding = "1½ cups at 7am and 6pm".into();
        b.treats = "Carrots, freeze-dried liver. No rawhide.".into();
        b.allergies = "Chicken (itchy ears)".into();
        b.conditions = "Mild hip dysplasia: keep her lean".into();
        b.temperament = "Loves everyone. Pulls on the leash when she sees squirrels.".into();
        b.grooming = "Brush twice a week; bath monthly".into();
        b.care_notes = "Walks at 7am, noon and 6pm. Leash and poop bags on the hook by the back door. She'll beg — don't fall for it.".into();
        b.vet = clinic.clone();
        b.emergency_vet = er.clone();
        b.insurance = PetInsurance {
            provider: "Trupanion".into(),
            policy: "TP-5521890".into(),
            phone: "(555) 600-2020".into(),
            coverage: "90% of eligible costs, accidents & illness".into(),
            deductible: "$250 per condition".into(),
            renews: Some(days(120)),
        };
        b.vaccinations = vec![
            Vaccination { name: "Rabies".into(), given: Some(years_ago(2) + chrono::Duration::days(20)), due: Some(years_ago(2) + chrono::Duration::days(20)).and_then(|d| d.checked_add_months(chrono::Months::new(36))), given_by: "Dr. Raman".into(), lot: "Tag R-22841".into(), ..Default::default() },
            Vaccination { name: "DHPP (distemper, parvo)".into(), given: Some(days(18 - 365 * 3)), due: Some(days(18)), given_by: "Dr. Raman".into(), ..Default::default() },
            Vaccination { name: "Bordetella (kennel cough)".into(), given: Some(days(-380)), due: Some(days(-15)), given_by: "Maple Grove".into(), notes: "Needed for boarding at Happy Tails".into(), ..Default::default() },
            Vaccination { name: "Leptospirosis".into(), given: Some(days(-200)), due: Some(days(165)), given_by: "Dr. Raman".into(), ..Default::default() },
        ];
        b.medications = vec![
            Medication { name: "Simparica Trio".into(), dose: "1 chewable (22–44 kg)".into(), how_often: "Once a month".into(), purpose: "Fleas, ticks and heartworm".into(), prescribed_by: "Dr. Raman".into(), started: Some(years_ago(2)), refills: "6-pack, 3 left".into(), ..Default::default() },
            Medication { name: "Carprofen".into(), dose: "75 mg".into(), how_often: "As needed".into(), purpose: "Hip pain after long hikes".into(), prescribed_by: "Dr. Raman".into(), started: Some(months_ago(8)), notes: "With food. Max 1 a day.".into(), ..Default::default() },
        ];
        b.weights = [(30, 31.2), (24, 32.0), (18, 31.6), (12, 30.9), (6, 30.4), (1, 30.1)].iter().map(|(m, kg)| WeightEntry { date: months_ago(*m), kg: *kg }).collect();
    }
    for i in [0usize, 1, 2, 3] {
        p.set_vaccine_reminder(dog, i, true);
    }
    p.add_medication_reminder(dog, 0, days(3));
    // A checkup booked at the vet.
    let mut visit = Reminder::new(EntityRef::Pet(dog), "Annual checkup + DHPP booster", days(11));
    visit.at = chrono::NaiveTime::from_hms_opt(10, 30, 0);
    visit.place = "Maple Grove Animal Hospital".into();
    visit.notes = "Bring a stool sample. Ask about her hips.".into();
    p.reminders.push(visit);
    let mut nails = Reminder::new(EntityRef::Pet(dog), "Nail trim", days(5));
    nails.repeat = Some(Recurrence { every: 4, unit: RecurrenceUnit::Weeks });
    p.reminders.push(nails);
    let visit = |date, kind: &str, by: &str, cost: &str, what: &str| {
        let mut s = ServiceRecord::new(EntityRef::Pet(dog), date);
        s.kind = ServiceKind::Other;
        s.kind_name = kind.into();
        s.performed_by = by.into();
        s.cost = cost.into();
        s.description = what.into();
        s
    };
    p.service_records.push(visit(months_ago(11), "Checkup", "Dr. Raman", "$185", "Annual exam. Healthy; down 0.7 kg, keep it up. Started Simparica Trio refill."));
    p.service_records.push(visit(months_ago(8), "Illness", "Dr. Raman", "$240", "Limping after a long hike. X-rays: mild hip dysplasia. Carprofen as needed."));
    p.service_records.push(visit(months_ago(3), "Grooming", "Paws & Suds", "$90", "Bath, de-shed, nail trim."));
    let mut n = Note::new(EntityRef::Pet(dog));
    n.title = "Boarding".into();
    n.body = "Happy Tails Lodge, (555) 410-8800. Needs Bordetella within the last year. She does best in a run near the window.".into();
    p.notes.push(n);

    // A cat, with less filled in.
    let cat = p.add_pet("Mochi", Species::Cat);
    {
        let m = p.pet_mut(cat).unwrap();
        m.breed = "Domestic shorthair".into();
        m.sex = Sex::Male;
        m.fixed = true;
        m.birthday = Some(years_ago(3));
        m.birthday_estimated = true;
        m.adopted = Some(years_ago(3) + chrono::Duration::days(75));
        m.adopted_from = "City Animal Shelter".into();
        m.markings = "Grey tabby, white socks".into();
        m.microchip = "900 164 001 987 654".into();
        m.food = "Wet: Fancy Feast pâté, ½ can twice a day. Dry: free-feed".into();
        m.temperament = "Shy with strangers. Hides under the guest bed.".into();
        m.vet = clinic;
        m.emergency_vet = er;
        m.vaccinations = vec![
            Vaccination { name: "Rabies".into(), given: Some(months_ago(10)), due: months_ago(10).checked_add_months(chrono::Months::new(12)), given_by: "Dr. Raman".into(), ..Default::default() },
            Vaccination { name: "FVRCP (distemper)".into(), given: Some(months_ago(10)), due: months_ago(10).checked_add_months(chrono::Months::new(36)), given_by: "Dr. Raman".into(), ..Default::default() },
        ];
        m.weights = [(12, 4.6), (6, 4.9), (1, 5.1)].iter().map(|(mo, kg)| WeightEntry { date: months_ago(*mo), kg: *kg }).collect();
    }
    p.set_vaccine_reminder(cat, 0, true);
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_builds() {
        let p = super::sample_house();
        assert_eq!(p.levels[0].rooms.len(), 6);
        assert_eq!(p.levels[0].objects.len(), 9);
        assert_eq!(p.levels[0].roofs.len(), 1);
        let up = p.levels.iter().find(|l| l.name == "Upper floor").unwrap();
        assert_eq!(up.rooms.len(), 7);
        assert_eq!(up.roofs.len(), 1);
        assert_eq!(up.objects.len(), 4, "the main roof's chimney, skylight, gutter and downspout");
        let reading = up.rooms.values().find(|r| r.name == "Reading room").unwrap();
        assert!(up.room_has_curves(reading));
        assert!(up.rooms.keys().all(|r| !up.room_overlaps_others(*r)));
        let main = p.levels[0].objects.values().find(|o| o.is_stairs()).unwrap();
        assert_eq!(p.stair_info(p.levels[0].id, main).unwrap().to, Some(up.id));
        assert_eq!(p.buildings.len(), 1);
        assert!(p.site().unwrap().features.len() >= 9);
        assert_eq!(p.levels[0].openings.len(), 10);
    }
}
