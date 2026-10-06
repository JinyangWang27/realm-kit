//! Factions, the war and peace between them, and the standing tracks a world
//! measures the player by.

use crate::*;

/// The furthest a standing value or a change to one reaches from zero, so
/// arithmetic on them stays far inside 32 bits.
pub const STANDING_BOUND: i32 = 1_000_000;

/// A political entity of the world, such as a kingdom, a clan or a guild.
/// It owns nothing and decides nothing by itself; other capabilities refer
/// to it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Faction {
    pub id: Id,
    pub name: String,
}

/// War and peace between factions. Every pair not listed starts at peace;
/// only effects change it afterwards.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Diplomacy {
    /// Pairs at war at the start, each listed once in either order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at_war: Vec<[Id; 2]>,
}

/// Whose standing a track measures the player against.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StandingScope {
    /// One value for the whole world, such as renown.
    #[default]
    Global,
    /// One value for each faction, such as favour at its court.
    Faction,
}

/// A bounded integer the world measures the player by. Changes clamp to
/// `min..=max`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StandingTrack {
    pub id: Id,
    pub name: String,
    #[serde(default, skip_serializing_if = "is_global")]
    pub scope: StandingScope,
    pub min: i32,
    pub max: i32,
    /// The starting value; for each faction, unless `starts` names it.
    #[serde(default)]
    pub start: i32,
    /// Starting values for particular factions on a faction track.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub starts: BTreeMap<Id, i32>,
    /// Names for ranges of the value, in ascending order: each holds from
    /// its `at` up to the next one's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thresholds: Vec<Threshold>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Threshold {
    pub at: i32,
    pub name: String,
}

impl StandingTrack {
    /// The starting value, with `faction` on a faction track.
    pub fn start(&self, faction: Option<&str>) -> i32 {
        faction
            .and_then(|f| self.starts.get(f))
            .copied()
            .unwrap_or(self.start)
    }
    /// The name of the highest threshold `value` has reached, if any.
    pub fn label(&self, value: i32) -> Option<&str> {
        self.thresholds
            .iter()
            .rev()
            .find(|t| value >= t.at)
            .map(|t| t.name.as_str())
    }
}

fn is_global(scope: &StandingScope) -> bool {
    *scope == StandingScope::Global
}

/// A pair of factions in its one canonical order, so `[a, b]` and `[b, a]`
/// name the same relation.
pub fn faction_pair(pair: &[Id; 2]) -> [Id; 2] {
    let [a, b] = pair.clone();
    if a <= b {
        [a, b]
    } else {
        [b, a]
    }
}
