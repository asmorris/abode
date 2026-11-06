//! A small example house, for trying the app and for tests.

use crate::geometry::{Vec2, rect_points};
use crate::model::*;
use chrono::{Local, NaiveDate};

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
    let mut paint = Paint::new(PaintTarget::Room(bedroom));
    paint.color_name = "Hale Navy".into();
    paint.color_code = "HC-154".into();
    paint.swatch = Some([66, 76, 92]);
    p.paints.push(paint);
    let _ = (kitchen, bath, utility);

    // ---- Roofs: the main gable roof, and a lower shed roof over the deck.
    let main_roof = p.add_roof(l, &rect_points(Vec2::ZERO, 9.0, 8.7), RoofShape::Gable).unwrap();
    {
        let r = p.levels[0].roofs.get_mut(&main_roof).unwrap();
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
    p.add_roof_object(l, "Chimney", ObjectCategory::Roof, rect(0.7, 0.7), Vec2::new(1.2, 1.8));
    p.add_roof_object(l, "Skylight", ObjectCategory::Roof, rect(0.6, 1.2), Vec2::new(6.6, 6.4));
    p.add_roof_object(l, "Front gutter", ObjectCategory::Roof, rect(9.4, 0.15), Vec2::new(4.5, -0.22));
    p.add_roof_object(l, "Downspout", ObjectCategory::Roof, rect(0.12, 0.12), Vec2::new(9.12, -0.22));
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
        o.details.model = "RAV4".into();
        o.details.purchased = d(2021, 5, 14);
        o.details.price = "$36,450".into();
        o.details.vendor = "Lakeside Toyota".into();
        let v = o.vehicle.as_mut().unwrap();
        v.year = "2021".into();
        v.trim = "XLE AWD".into();
        v.color = "Lunar Rock".into();
        v.vin = "2T3P1RFV5MW000000".into();
        v.plate = "CKRT 214".into();
        v.fuel = Fuel::Gasoline;
        v.engine = "2.5L 4-cylinder".into();
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
    let mut r = Reminder::new(EntityRef::Object(car), "Swap to winter tires", NaiveDate::from_ymd_opt(2026, 11, 15).unwrap());
    r.repeat = Some(Recurrence { every: 1, unit: RecurrenceUnit::Years });
    p.reminders.push(r);

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

#[cfg(test)]
mod tests {
    #[test]
    fn sample_builds() {
        let p = super::sample_house();
        assert_eq!(p.levels[0].rooms.len(), 6);
        assert_eq!(p.levels[0].objects.len(), 11);
        assert_eq!(p.levels[0].roofs.len(), 2);
        assert_eq!(p.buildings.len(), 1);
        assert!(p.site().unwrap().features.len() >= 9);
        assert_eq!(p.levels[0].openings.len(), 10);
    }
}
