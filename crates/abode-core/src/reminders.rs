//! Reminder scheduling.

use crate::ids::*;
use crate::model::*;
use chrono::{Days, Months, NaiveDate};

/// How soon counts as "due soon".
pub const DUE_SOON_DAYS: i64 = 14;
/// How close (km) counts as "due soon" for odometer reminders.
pub const DUE_SOON_KM: f64 = 800.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueStatus {
    Done,
    Overdue(i64),
    DueSoon(i64),
    Upcoming(i64),
    /// Snoozed: comes back in this many days.
    Snoozed(i64),
    /// Odometer reminders: this many km past due.
    OverKm(i64),
    /// Odometer reminders: due within this many km.
    SoonKm(i64),
    /// Odometer reminders: this many km to go.
    LaterKm(i64),
}

impl DueStatus {
    /// 3 = overdue, 2 = due soon, 1 = upcoming, 0 = done/snoozed.
    pub fn urgency(self) -> u8 {
        match self {
            DueStatus::Overdue(_) | DueStatus::OverKm(_) => 3,
            DueStatus::DueSoon(_) | DueStatus::SoonKm(_) => 2,
            DueStatus::Upcoming(_) | DueStatus::LaterKm(_) => 1,
            DueStatus::Done | DueStatus::Snoozed(_) => 0,
        }
    }

    pub fn is_overdue(self) -> bool {
        self.urgency() == 3
    }
}

impl Recurrence {
    pub fn advance(&self, from: NaiveDate) -> NaiveDate {
        let n = self.every.max(1);
        let next = match self.unit {
            RecurrenceUnit::Days => from.checked_add_days(Days::new(n as u64)),
            RecurrenceUnit::Weeks => from.checked_add_days(Days::new(7 * n as u64)),
            RecurrenceUnit::Months => from.checked_add_months(Months::new(n)),
            RecurrenceUnit::Years => from.checked_add_months(Months::new(12 * n)),
        };
        next.unwrap_or(from)
    }

    pub fn describe(&self) -> String {
        if self.every <= 1 {
            let unit = match self.unit {
                RecurrenceUnit::Days => "day",
                RecurrenceUnit::Weeks => "week",
                RecurrenceUnit::Months => "month",
                RecurrenceUnit::Years => "year",
            };
            format!("Every {unit}")
        } else {
            format!("Every {} {}", self.every, self.unit.label())
        }
    }
}

impl Reminder {
    /// Whether it comes back after it's done (by date or by distance).
    pub fn repeats(&self) -> bool {
        (self.due_by.uses_date() && self.repeat.is_some()) || (self.due_by.uses_distance() && self.every_km.is_some())
    }

    pub fn is_done(&self) -> bool {
        !self.repeats() && self.completed.is_some()
    }

    /// Km until an odometer reminder is due (negative once it's past), given
    /// the vehicle's current reading.
    pub fn km_left(&self, odometer: Option<f64>) -> Option<f64> {
        if !self.due_by.uses_distance() {
            return None;
        }
        Some(self.due_km? - odometer?)
    }

    /// [`Reminder::status`], counting the odometer too for vehicle
    /// reminders: whichever of date and distance is more pressing.
    pub fn status_at(&self, today: NaiveDate, odometer: Option<f64>) -> DueStatus {
        let by_date = self.status(today);
        if matches!(by_date, DueStatus::Done | DueStatus::Snoozed(_)) || !self.due_by.uses_distance() {
            return by_date;
        }
        let by_km = self.km_left(odometer).map(|left| {
            if left <= 0.0 {
                DueStatus::OverKm((-left).round() as i64)
            } else if left <= DUE_SOON_KM {
                DueStatus::SoonKm(left.round() as i64)
            } else {
                DueStatus::LaterKm(left.round() as i64)
            }
        });
        match (self.due_by, by_km) {
            (DueBy::Distance, Some(k)) => k,
            // No reading yet: nothing to measure against.
            (DueBy::Distance, None) => DueStatus::LaterKm(0),
            (_, Some(k)) if k.urgency() > by_date.urgency() => k,
            _ => by_date,
        }
    }

    /// Snoozed until a date still to come.
    pub fn is_snoozed(&self, today: NaiveDate) -> bool {
        !self.is_done() && self.snoozed_until.is_some_and(|s| s > today)
    }

    /// When it's actually due: the due date, or the end of a snooze that
    /// pushed it later.
    pub fn effective_due(&self) -> NaiveDate {
        self.snoozed_until.map_or(self.due, |s| s.max(self.due))
    }

    /// When it was last finished, for the Completed list: a one-off's
    /// completion date, or a repeating one's most recent.
    pub fn finished_on(&self) -> Option<NaiveDate> {
        self.completed.or(self.last_done)
    }

    /// Status by date alone. See [`Reminder::status_at`] for vehicles.
    pub fn status(&self, today: NaiveDate) -> DueStatus {
        if self.is_done() {
            return DueStatus::Done;
        }
        if self.is_snoozed(today) {
            return DueStatus::Snoozed((self.effective_due() - today).num_days());
        }
        let days = (self.effective_due() - today).num_days();
        if days < 0 {
            DueStatus::Overdue(-days)
        } else if days <= DUE_SOON_DAYS {
            DueStatus::DueSoon(days)
        } else {
            DueStatus::Upcoming(days)
        }
    }
}

impl Project {
    /// Mark a reminder done on `on`: logs a service record against the same
    /// thing and schedules the next occurrence (if it repeats).
    ///
    /// For a vehicle the service record gets its odometer reading, and an
    /// odometer reminder that repeats comes due again that far on from it.
    pub fn complete_reminder(&mut self, id: ReminderId, on: NaiveDate) -> Option<ServiceId> {
        let target = self.reminders.iter().find(|r| r.id == id)?.target;
        let odometer = self.odometer_for(target);
        let r = self.reminders.iter_mut().find(|r| r.id == id)?;
        r.last_done = Some(on);
        r.snoozed_until = None;
        if !r.repeats() {
            r.completed = Some(on);
        }
        if let (true, Some(rep)) = (r.due_by.uses_date(), r.repeat) {
            r.due = rep.advance(on);
        }
        if r.due_by.uses_distance() {
            match r.every_km {
                Some(every) => r.due_km = Some(odometer.or(r.due_km).unwrap_or(0.0) + every),
                // Only the date repeats: the distance was a one-off.
                None if r.repeats() => r.due_km = None,
                None => {}
            }
        }
        let mut s = ServiceRecord::new(r.target, on);
        s.odometer = odometer;
        s.description = r.title.clone();
        if !r.notes.trim().is_empty() {
            s.description = format!("{}\n{}", r.title, r.notes);
        }
        let sid = s.id;
        self.service_records.push(s);
        Some(sid)
    }

    /// Put a reminder off until `until`.
    pub fn snooze_reminder(&mut self, id: ReminderId, until: NaiveDate) {
        if let Some(r) = self.reminders.iter_mut().find(|r| r.id == id) {
            r.snoozed_until = Some(until);
        }
    }

    pub fn unsnooze_reminder(&mut self, id: ReminderId) {
        if let Some(r) = self.reminders.iter_mut().find(|r| r.id == id) {
            r.snoozed_until = None;
        }
    }

    /// Undo marking a one-off reminder done (its service record stays).
    pub fn reopen_reminder(&mut self, id: ReminderId) {
        if let Some(r) = self.reminders.iter_mut().find(|r| r.id == id) {
            r.completed = None;
        }
    }

    /// A reminder's status, using the odometer if it's about a vehicle.
    pub fn reminder_status(&self, r: &Reminder, today: NaiveDate) -> DueStatus {
        r.status_at(today, self.odometer_for(r.target))
    }

    pub fn overdue_count(&self, today: NaiveDate) -> usize {
        self.reminders.iter().filter(|r| self.reminder_status(r, today).is_overdue()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn monthly_advance_clamps_month_end() {
        let r = Recurrence { every: 1, unit: RecurrenceUnit::Months };
        assert_eq!(r.advance(d(2026, 1, 31)), d(2026, 2, 28));
    }

    #[test]
    fn completing_repeating_reminder_reschedules_and_logs() {
        let mut p = Project::new("T");
        let mut r = Reminder::new(EntityRef::General, "Change furnace filter", d(2026, 1, 1));
        r.repeat = Some(Recurrence { every: 3, unit: RecurrenceUnit::Months });
        let id = r.id;
        p.reminders.push(r);
        assert_eq!(p.reminders[0].status(d(2026, 1, 10)), DueStatus::Overdue(9));
        p.complete_reminder(id, d(2026, 1, 10)).unwrap();
        assert_eq!(p.reminders[0].due, d(2026, 4, 10));
        assert_eq!(p.service_records.len(), 1);
        assert!(!p.reminders[0].is_done());
    }

    #[test]
    fn snoozed_reminder_is_not_overdue_until_it_wakes() {
        let mut p = Project::new("T");
        let r = Reminder::new(EntityRef::Roof, "Clean gutters", d(2026, 10, 1));
        let id = r.id;
        p.reminders.push(r);
        assert_eq!(p.overdue_count(d(2026, 10, 10)), 1);
        p.snooze_reminder(id, d(2026, 10, 17));
        assert_eq!(p.overdue_count(d(2026, 10, 10)), 0);
        assert_eq!(p.reminders[0].status(d(2026, 10, 10)), DueStatus::Snoozed(7));
        // After the snooze it's due again, counted from the snooze date.
        assert_eq!(p.reminders[0].status(d(2026, 10, 20)), DueStatus::Overdue(3));
        // Finishing it clears the snooze.
        p.complete_reminder(id, d(2026, 10, 20));
        assert_eq!(p.reminders[0].snoozed_until, None);
        assert_eq!(p.reminders[0].finished_on(), Some(d(2026, 10, 20)));
        p.reopen_reminder(id);
        assert!(!p.reminders[0].is_done());
    }

    #[test]
    fn odometer_reminder_comes_due_by_distance() {
        let mut p = Project::new("T");
        let site = p.site_id().unwrap();
        let shape = Shape::new(ShapeKind::Rectangle, 1.8, 4.6);
        let car = p.add_object(site, "Car", ObjectCategory::Vehicle, shape, crate::Vec2::new(0.0, 0.0)).unwrap();
        p.levels.iter_mut().find_map(|l| l.objects.get_mut(&car)).unwrap().vehicle.as_mut().unwrap().odometer = Some(41500.0);
        let mut r = Reminder::new(EntityRef::Object(car), "Oil change", d(2026, 9, 1));
        r.due_by = DueBy::Either;
        r.repeat = Some(Recurrence { every: 6, unit: RecurrenceUnit::Months });
        r.due_km = Some(42000.0);
        r.every_km = Some(8000.0);
        let id = r.id;
        p.reminders.push(r);
        // Due soon by distance beats "upcoming" by date.
        assert_eq!(p.reminder_status(&p.reminders[0], d(2026, 6, 1)), DueStatus::SoonKm(500));
        // Past the date: overdue by date.
        assert_eq!(p.reminder_status(&p.reminders[0], d(2026, 9, 5)), DueStatus::Overdue(4));
        p.complete_reminder(id, d(2026, 6, 1));
        let r = &p.reminders[0];
        assert_eq!(r.due_km, Some(49500.0));
        assert_eq!(r.due, d(2026, 12, 1));
        assert_eq!(p.service_records[0].odometer, Some(41500.0));
        assert!(!r.is_done());
        // Distance-only, one-off: done once completed.
        let mut once = Reminder::new(EntityRef::Object(car), "Timing belt", d(2026, 1, 1));
        once.due_by = DueBy::Distance;
        once.due_km = Some(40000.0);
        let oid = once.id;
        p.reminders.push(once);
        assert_eq!(p.reminder_status(&p.reminders[1], d(2026, 6, 1)), DueStatus::OverKm(1500));
        p.complete_reminder(oid, d(2026, 6, 1));
        assert!(p.reminders[1].is_done());
    }

    #[test]
    fn one_off_reminder_is_done_after_completion() {
        let mut p = Project::new("T");
        let r = Reminder::new(EntityRef::Roof, "Roof inspection", d(2026, 5, 1));
        let id = r.id;
        p.reminders.push(r);
        p.complete_reminder(id, d(2026, 5, 2));
        assert_eq!(p.reminders[0].status(d(2026, 6, 1)), DueStatus::Done);
    }
}
