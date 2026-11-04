//! Combat weapon proficiency: which weapons share fundamentals, by the
//! world's own names. Distinct from the point-bought `Proficiency` of trades
//! and from techniques: this is what use of a weapon teaches the hands, not a
//! move or a school.

use crate::*;

/// The world's weapon families and handling disciplines, and what rank in
/// them does. Absent, no weapon trains and no attack's timing changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponProficiency {
    pub disciplines: Vec<WeaponDiscipline>,
    pub families: Vec<WeaponFamily>,
    /// Rank from XP: the highest entry whose `xp` is reached, else 0.
    pub ranks: Vec<ProficiencyRank>,
    /// Percent of an attack's competence that is family rank; the rest is
    /// handling rank.
    pub family_percent: u32,
    /// Percent taken off an attack's wind-up at competence 100, proportionally.
    pub windup_percent: u32,
    /// Percent taken off an attack's recovery at competence 100, proportionally.
    pub recovery_percent: u32,
    pub xp: WeaponXp,
}

/// Motor coordination common to a way of holding a weapon, such as one-handed
/// or polearm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponDiscipline {
    pub id: Id,
    pub name: String,
}

/// Weapons that share mechanics, and so one rank.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponFamily {
    pub id: Id,
    pub name: String,
    /// Percent of this family's direct rank each named family may borrow.
    /// Directional; a family not named borrows nothing from this one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub affinities: BTreeMap<Id, u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProficiencyRank {
    /// 1 to 100.
    pub rank: u32,
    /// Cumulative XP.
    pub xp: u64,
}

/// What meaningful use teaches. Each amount falls off against weaker foes as
/// defeat XP does; inert targets teach nothing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponXp {
    /// For a weapon attack that does damage to a foe.
    pub hit: u32,
    /// For a parry or a raised guard that stops a foe's blow.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub defense: u32,
    /// The most XP one foe can ever teach, across every fight with it, so
    /// neither restarting a fight nor reloading makes more.
    pub target_budget: u32,
}

/// What a wielded weapon trains. Pieces with one must share a slot, so at most
/// one is ever the weapon in hand.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WeaponProfile {
    pub family: Id,
    /// Weights over disciplines, totalling 100.
    pub handling: BTreeMap<Id, u32>,
}

/// Ranks a skill asks of the wielded weapon's family, beyond knowing the move.
/// Borrowed rank counts toward `effective` only.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillMastery {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub direct: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub effective: u32,
}

/// Highest rank on the proficiency scale.
pub const PROFICIENCY_RANK_BOUND: u32 = 100;
/// Most an attack's wind-up or recovery can be cut, in percent.
pub const ATTACK_CUT_BOUND: u32 = 50;
/// Bound on authored proficiency XP amounts and budgets.
pub const WEAPON_XP_BOUND: u32 = 1_000_000;
/// Bound on the XP a rank asks.
pub const RANK_XP_BOUND: u64 = 1_000_000_000_000;
