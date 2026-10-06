//! World time, the roads that take it, and what happens on a schedule.

use crate::*;

/// The latest minute world time may reach, about 1,900 years from the epoch.
pub const WORLD_TIME_BOUND: u64 = 1_000_000_000;
/// The longest road, wait or rest, in minutes: 30 days.
pub const DURATION_BOUND: u64 = 43_200;
pub const MINUTES_PER_DAY: u64 = 1_440;

/// Optional world time: minutes from an authored epoch, advanced only by
/// travel, waiting and resting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorldTime {
    /// The minute a new playthrough starts at.
    pub start: u64,
    /// How the time is shown: `{day}` (from 1), `{hour}` and `{minute}`
    /// (two digits each); with a calendar also `{year}`, `{month}` and
    /// `{day_of_month}` (two digits each).
    pub clock: TextTemplate,
    /// Waiting is possible, and the menu offers this many minutes at a time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<u64>,
    /// Resting at a safe location also passes this many minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<u64>,
    /// Pins minute 0 to a Gregorian midnight, so the time has a date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<Calendar>,
}

/// The proleptic Gregorian calendar, from the date at minute 0. A date is
/// always derived from the minute, never saved beside it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Calendar {
    pub epoch: GregorianDate,
}

/// A day in the proleptic Gregorian calendar; `month` and `day` count from 1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GregorianDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl GregorianDate {
    fn to_date(self) -> Option<::time::Date> {
        let month = ::time::Month::try_from(self.month).ok()?;
        ::time::Date::from_calendar_date(self.year, month, self.day).ok()
    }

    fn from_date(date: ::time::Date) -> Self {
        Self {
            year: date.year(),
            month: date.month().into(),
            day: date.day(),
        }
    }
}

impl Calendar {
    /// The date at `minute`; `None` if the epoch is no real date or the
    /// date falls outside the years -9999 to 9999. Validation rules out both
    /// up to `WORLD_TIME_BOUND`.
    pub fn date(&self, minute: u64) -> Option<GregorianDate> {
        let days = i32::try_from(minute / MINUTES_PER_DAY).ok()?;
        let epoch = self.epoch.to_date()?.to_julian_day();
        let date = ::time::Date::from_julian_day(epoch.checked_add(days)?).ok()?;
        Some(GregorianDate::from_date(date))
    }

    /// The first midnight strictly after `minute`.
    pub fn next_day(&self, minute: u64) -> Option<u64> {
        (minute / MINUTES_PER_DAY + 1).checked_mul(MINUTES_PER_DAY)
    }

    /// The first first-of-a-month midnight strictly after `minute`. Every
    /// month boundary a step crosses is found by repeating this from the
    /// boundary before it.
    pub fn next_month(&self, minute: u64) -> Option<u64> {
        let today = self.date(minute)?.to_date()?;
        let year = match today.month() {
            ::time::Month::December => today.year().checked_add(1)?,
            _ => today.year(),
        };
        let first = ::time::Date::from_calendar_date(year, today.month().next(), 1).ok()?;
        let days = first.to_julian_day() - self.epoch.to_date()?.to_julian_day();
        u64::try_from(days).ok()?.checked_mul(MINUTES_PER_DAY)
    }
}

/// An undirected road between two locations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Road {
    pub id: Id,
    pub between: [Id; 2],
    /// Travel time; a road without one takes no time.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub minutes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    /// Shown when `requires` does not hold; needed exactly when it is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_text: Option<String>,
}

impl Road {
    /// The other end, seen from `from`; `None` if the road does not touch it.
    pub fn leads(&self, from: &str) -> Option<&Id> {
        match &self.between {
            [a, b] if a == from => Some(b),
            [a, b] if b == from => Some(a),
            _ => None,
        }
    }
}

/// When something happens: first at minute `at`, then every `every` minutes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// The first occurrence, after the start of play.
    pub at: u64,
    /// The period of a recurring schedule; a one-shot one has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<u64>,
}

impl Schedule {
    /// The first occurrence at or after `minute`, or strictly after it.
    pub fn next(&self, minute: u64, inclusive: bool) -> Option<u64> {
        let floor = if inclusive {
            minute
        } else {
            minute.checked_add(1)?
        };
        if self.at >= floor {
            return Some(self.at);
        }
        let every = self.every?;
        let periods = (floor - self.at).div_ceil(every);
        self.at.checked_add(periods.checked_mul(every)?)
    }
}

/// Authored effects that happen on a schedule, while their condition holds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorldEvent {
    pub id: Id,
    pub schedule: Schedule,
    /// Checked at each occurrence; an occurrence where it fails does nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    pub effects: Vec<Effect>,
}

/// A character who moves among locations on a schedule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Moves {
    /// Where it may be; each occurrence draws one of them.
    pub among: Vec<Id>,
    pub schedule: Schedule,
}

/// The placeholders every clock may use.
pub(crate) const CLOCK_FIELDS: [&str; 3] = ["day", "hour", "minute"];
/// The placeholders only a clock with a calendar may use.
pub(crate) const CALENDAR_FIELDS: [&str; 3] = ["year", "month", "day_of_month"];

/// "Day 3, 08:05" or "742-02-03 08:05" from a clock template. `{day}` is
/// always the elapsed day from 1, calendar or not.
pub fn clock_values(time: &WorldTime, minute: u64) -> Vec<(&'static str, String)> {
    let day = minute / MINUTES_PER_DAY + 1;
    let of_day = minute % MINUTES_PER_DAY;
    let mut values = vec![
        ("day", day.to_string()),
        ("hour", format!("{:02}", of_day / 60)),
        ("minute", format!("{:02}", of_day % 60)),
    ];
    if let Some(date) = time.calendar.and_then(|c| c.date(minute)) {
        values.extend([
            ("year", date.year.to_string()),
            ("month", format!("{:02}", date.month)),
            ("day_of_month", format!("{:02}", date.day)),
        ]);
    }
    values
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}
