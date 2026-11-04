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

    p.roof.material = "Asphalt shingles".into();
    p.roof.installed = NaiveDate::from_ymd_opt(2015, 6, 1);
    p.changelog.clear();
    let entry = |summary: &str, details: &str, date: Option<NaiveDate>| {
        let mut e = ChangeEntry::manual("Previous owner", summary, date);
        e.details = details.into();
        e
    };
    p.changelog.push(entry("Moved in", "Bought the house from the original owners.", NaiveDate::from_ymd_opt(2012, 8, 1)));
    let mut roof = entry("New roof", "Full tear-off, 30-year asphalt shingles.", NaiveDate::from_ymd_opt(2015, 6, 1));
    roof.targets.push(EntityRef::Roof);
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
        assert_eq!(p.levels[0].objects.len(), 7);
        assert_eq!(p.levels[0].openings.len(), 10);
    }
}
