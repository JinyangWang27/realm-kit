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
    /// Makes the item usable: each use spends one and restores vitals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumable: Option<Consumable>,
}

/// What using one unit restores, each capped at the maximum.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Consumable {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub hp: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mp: u32,
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
    /// What wielding it lets the wearer do, by the world's own names
    /// (`blade`, `bow`, `shield`): a skill that `requires` one is usable only
    /// while something worn, or an empty slot's `bare` hands, provides it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Id>,
    /// Real-time only: makes it a drawn weapon that looses projectiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranged: Option<RangedWeapon>,
    /// What wielding it trains, in a world with weapon proficiency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon: Option<WeaponProfile>,
    /// Damage taken on a channel is multiplied by `num / den`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modifiers: BTreeMap<Channel, Modifier>,
    /// Improvement tiers, in order; a piece rises one tier at a time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<Tier>,
}

/// A weapon drawn and loosed in a real-time fight. Step counts are at speed
/// 100, and scale with the wielder's speed as attack phases do. Everything
/// spatial about the shot (velocity, drop, what it touches) is the host's.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RangedWeapon {
    /// Steps of drawing to reach a full draw.
    pub draw: u32,
    /// Steps of drawing before a shot can be loosed at all.
    pub min_draw: u32,
    /// Power of a full draw, and of the shortest; those between scale evenly.
    pub power: u32,
    pub min_power: u32,
    #[serde(default)]
    pub channel: Channel,
    /// Impact of a full draw's hit; a partial draw's has none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub impact: u32,
    /// Steps committed after loosing, before the wielder is free.
    pub recovery: u32,
    /// Steps a loosed projectile stays live; it can land on none after that.
    pub flight: u32,
}

/// A damage multiplier: 0/1 is immunity, 1/2 resistance, 2/1 vulnerability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Modifier {
    pub num: u32,
    pub den: u32,
}
