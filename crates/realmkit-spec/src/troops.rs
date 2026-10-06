//! Troops: lines of soldiers who level up, recruiting, upkeep, the armies a
//! player can fight, and the rules of a mass battle.

use crate::*;

/// The most soldiers a roster, a recruiting pool or an army stack holds.
pub const ROSTER_BOUND: u64 = 10_000;

/// Optional troops: soldier lines, a roster limit and upkeep. Needs combat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Troops {
    pub classes: Vec<TroopClass>,
    /// Lines in the order rosters and battles list them.
    pub lines: Vec<TroopLine>,
    /// Healthy and wounded soldiers together; the player is not counted.
    pub limit: u64,
    /// The share of the player side's battle losses wounded, not killed.
    #[serde(default)]
    pub wounded_percent: u32,
    /// Wages and recovery on a schedule; needs world time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upkeep: Option<Upkeep>,
    /// When wages fall due: on the upkeep schedule, or at each month end.
    #[serde(default, skip_serializing_if = "Payroll::is_upkeep")]
    pub payroll: Payroll,
}

/// When the roster's wages fall due. Either way they are paid from the bank
/// first, where the world has one, then from carried currency.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payroll {
    /// On each upkeep, whose `desert_percent` applies when they go unpaid.
    /// Braced so that a stray field is refused like any other.
    Upkeep {},
    /// At each Gregorian month end, for the roster standing then; needs a
    /// calendar. It needs no upkeep: where there is one, it only mends the
    /// wounded.
    Monthly {
        /// The share of each squad that leaves when the month's wages go
        /// unpaid.
        #[serde(default)]
        desert_percent: u32,
    },
}

impl Default for Payroll {
    fn default() -> Self {
        Payroll::Upkeep {}
    }
}

impl Payroll {
    fn is_upkeep(&self) -> bool {
        *self == Payroll::Upkeep {}
    }
}

/// A kind of soldier for matchups: shooting, riding, or neither.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TroopClass {
    pub id: Id,
    pub name: String,
    /// Shoots from behind the frontage, whole.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ranged: bool,
    /// Can flank the enemy's ranged troops.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mounted: bool,
}

/// A ladder of levels soldiers climb by battle XP, then branch from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TroopLine {
    pub id: Id,
    pub class: Id,
    #[serde(default)]
    pub channel: Channel,
    /// Level 1 first; it alone has `xp` 0 and must be named.
    pub levels: Vec<TroopLevel>,
    /// Lines a last-level soldier can be upgraded into, at their level 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<TroopUpgrade>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TroopLevel {
    /// XP each soldier needs to rise into this level from the one before.
    pub xp: u64,
    /// A new name from this level on; without one the previous name holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub stats: Stats,
    /// Currency per soldier on each upkeep; without one the previous wage holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wage: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TroopUpgrade {
    pub to: Id,
    /// Currency per soldier upgraded.
    #[serde(default)]
    pub cost: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Upkeep {
    pub schedule: Schedule,
    /// The share of each stack's wounded who recover on each upkeep.
    #[serde(default)]
    pub recover_percent: u32,
    /// The share of each stack that leaves when wages go unpaid on upkeep;
    /// monthly payroll has its own.
    #[serde(default)]
    pub desert_percent: u32,
}

impl TroopLine {
    /// The soldier name at 1-based `level`: the nearest named level at or below it.
    pub fn name_at(&self, level: usize) -> &str {
        self.levels[..level]
            .iter()
            .rev()
            .find_map(|l| l.name.as_deref())
            .unwrap_or_default()
    }
    /// The wage at 1-based `level`: the nearest authored wage at or below it.
    pub fn wage_at(&self, level: usize) -> u64 {
        self.levels[..level]
            .iter()
            .rev()
            .find_map(|l| l.wage)
            .unwrap_or(0)
    }
}

impl Troops {
    pub fn class(&self, id: &str) -> Option<&TroopClass> {
        self.classes.iter().find(|c| c.id == id)
    }
    pub fn line(&self, id: &str) -> Option<&TroopLine> {
        self.lines.iter().find(|l| l.id == id)
    }
}

/// Optional mass battles between the player's side and authored armies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Battle {
    /// Melee heads per side that fight each round.
    pub frontage: u64,
    /// After this many rounds the weaker side withdraws.
    pub rounds: u32,
    /// Each side's per-round roll, in percent, inclusive.
    pub roll: [u32; 2],
    /// Attacker class, then target class: percent of damage; 100 if absent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub matchups: BTreeMap<Id, BTreeMap<Id, u32>>,
    /// Without morale a side never breaks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morale: Option<MoraleRules>,
    /// Melee damage dealt or taken by a holding side, in percent.
    pub hold_percent: u32,
    /// Damage of mounted troops sent at the enemy's archers, in percent.
    pub flank_percent: u32,
    /// Counts double in a pursuit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pursuit_class: Option<Id>,
    /// The player's share of a victory's XP; the rest goes to the roster.
    #[serde(default)]
    pub player_xp_percent: u32,
}

/// Morale starts at 100 each battle and falls with losses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MoraleRules {
    /// Morale lost per loss, times 1/starting size.
    pub factor: u32,
    /// Damage at zero morale, in percent; 100 at full morale.
    pub floor: u32,
    /// A side below this breaks.
    pub rout: u32,
}

/// Soldiers for hire at a location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recruits {
    pub troops: Vec<RecruitOffer>,
    /// Pools refill on a schedule; needs world time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refill: Option<Refill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecruitOffer {
    pub line: Id,
    /// Currency per recruit.
    #[serde(default)]
    pub price: u64,
    /// The pool's full size, which it starts at.
    pub size: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Refill {
    pub schedule: Schedule,
    /// Recruits each pool gains per refill, up to its size.
    pub amount: u64,
}

/// An army a character leads: engaging it starts a mass battle, unless it
/// is an ally, which joins the player's side instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Army {
    pub troops: Vec<ArmyStack>,
    /// Shared between the player and the roster on victory.
    #[serde(default)]
    pub xp: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loot: Vec<ItemStack>,
    /// Can be fought again after defeat.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeatable: bool,
    /// An ally: never engaged, and joins the player's battles here while this holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joins: Option<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmyStack {
    pub line: Id,
    #[serde(default = "first_level")]
    pub level: usize,
    pub count: u64,
}
