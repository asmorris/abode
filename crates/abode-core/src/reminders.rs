//! Reminder scheduling.

use crate::ids::*;
use crate::model::*;
use chrono::{Days, Months, NaiveDate};

/// How soon counts as "due soon".
pub const DUE_SOON_DAYS: i64 = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueStatus {
    Done,
    Overdue(i64),
    DueSoon(i64),
    Upcoming(i64),
    /// Snoozed: comes back in this many days.
    Snoozed(i64),
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
    pub fn is_done(&self) -> bool {
        self.repeat.is_none() && self.completed.is_some()
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
    pub fn complete_reminder(&mut self, id: ReminderId, on: NaiveDate) -> Option<ServiceId> {
        let r = self.reminders.iter_mut().find(|r| r.id == id)?;
        r.last_done = Some(on);
        r.snoozed_until = None;
        match r.repeat {
            Some(rep) => r.due = rep.advance(on),
            None => r.completed = Some(on),
        }
        let mut s = ServiceRecord::new(r.target, on);
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

    pub fn overdue_count(&self, today: NaiveDate) -> usize {
        self.reminders
            .iter()
            .filter(|r| matches!(r.status(today), DueStatus::Overdue(_)))
            .count()
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
    fn one_off_reminder_is_done_after_completion() {
        let mut p = Project::new("T");
        let r = Reminder::new(EntityRef::Roof, "Roof inspection", d(2026, 5, 1));
        let id = r.id;
        p.reminders.push(r);
        p.complete_reminder(id, d(2026, 5, 2));
        assert_eq!(p.reminders[0].status(d(2026, 6, 1)), DueStatus::Done);
    }
}
