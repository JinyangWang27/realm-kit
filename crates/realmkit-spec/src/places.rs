//! Locations, the exits between them, and the characters placed in them.

use crate::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub id: Id,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub exits: BTreeMap<Direction, Exit>,
    #[serde(default)]
    pub characters: Vec<Id>,
    /// Resting here restores HP and MP. Requires combat.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub safe: bool,
    /// Crafting stations here, such as an anvil. Requires combat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<Id>,
    /// Soldiers for hire here. Requires troops and an economy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recruits: Option<Recruits>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    pub destination: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    pub blocked_text: String,
}

/// Anyone in the world. Talking and fighting are optional components.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: Id,
    pub name: String,
    pub description: String,
    /// The condition for the character to be present where it is placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<CombatProfile>,
    /// Moves among locations on a schedule instead of staying where placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves: Option<Moves>,
    /// Leads an army: an enemy to engage in a mass battle, or an ally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army: Option<Army>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CombatProfile {
    pub stats: Stats,
    pub xp: u64,
    #[serde(default)]
    pub loot: Vec<ItemStack>,
    /// Usable once the profile's level reaches each skill's unlock level.
    #[serde(default)]
    pub skills: Vec<Id>,
    #[serde(default)]
    pub basic_channel: Channel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_crit: Option<Crit>,
    /// Gates the profile's skills and scales the XP it grants.
    #[serde(default = "first_level")]
    pub level: usize,
    /// Engaging any member brings in every present, undefeated member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<Id>,
}
