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
    /// (two digits each).
    pub clock: TextTemplate,
    /// Waiting is possible, and the menu offers this many minutes at a time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<u64>,
    /// Resting at a safe location also passes this many minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<u64>,
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

/// "Day 3, 08:05" from a clock template.
pub fn clock_values(minute: u64) -> [(&'static str, String); 3] {
    let day = minute / MINUTES_PER_DAY + 1;
    let of_day = minute % MINUTES_PER_DAY;
    [
        ("day", day.to_string()),
        ("hour", format!("{:02}", of_day / 60)),
        ("minute", format!("{:02}", of_day % 60)),
    ]
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}
