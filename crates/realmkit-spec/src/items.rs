//! Items, stacks of them, and what wearing one does.

use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemStack {
    pub item: Id,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: Id,
    pub name: String,
    pub description: String,
    /// Makes the item wearable; each one obtained is an individual piece.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<Equipment>,
}

/// What wearing an item does.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Equipment {
    /// The slots it occupies; a two-handed weapon takes two.
    pub slots: Vec<Id>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bonuses: BTreeMap<Stat, u32>,
    /// Subtracted from speed before the speed cap; penalties add up.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub speed_penalty: u32,
    /// A weapon's channel for the wearer's basic attack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_channel: Option<Channel>,
    /// A weapon's basic-attack time, in percent of a basic action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_time: Option<u32>,
    /// Damage taken on a channel is multiplied by `num / den`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modifiers: BTreeMap<Channel, Modifier>,
}

/// A damage multiplier: 0/1 is immunity, 1/2 resistance, 2/1 vulnerability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Modifier {
    pub num: u32,
    pub den: u32,
}
