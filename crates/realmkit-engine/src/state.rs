//! The deterministic playthrough state and the save that carries it.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerState {
    pub location: Id,
    pub inventory: BTreeMap<Id, u64>,
}

/// Fighting progress; exists only in worlds with a combat block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatState {
    pub xp: u64,
    pub level: usize,
    /// Characters defeated for good.
    pub defeated: BTreeSet<Id>,
    /// Stat points spent per stat; unspent points are derived from the level.
    pub allocation: BTreeMap<Stat, u32>,
    /// Learned techniques by ID.
    pub techniques: BTreeMap<Id, TechniqueState>,
    /// Individual pieces of equipment by instance ID.
    pub gear: BTreeMap<u64, Gear>,
    /// The next instance ID; IDs are never reused.
    pub next_gear: u64,
    pub stance: Stance,
}

/// Where the player's HP and MP live: exactly one owner at a time, so a save
/// never holds two copies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Stance {
    Exploring(Vitals),
    Fighting(Encounter),
    /// Leading a mass battle; it holds the player's HP and MP until it ends.
    Battle(BattleState),
}

/// A mass battle between the player's side (0) and an army (1), paused
/// between rounds for the player's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleState {
    pub army: Id,
    /// Armies that joined the player, in the order they did.
    pub allies: Vec<Id>,
    /// Rounds fought so far.
    pub round: u32,
    /// The player's HP and MP; at 0 HP the player is knocked out of the fight.
    pub hp: u32,
    pub mp: u32,
    /// Side 0: the roster's healthy squads (lines as authored, levels from
    /// the highest), then each ally's troops. Side 1: the army's troops.
    pub sides: [BattleSide; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleSide {
    pub stacks: Vec<BattleStack>,
    pub morale: u32,
    /// Morale-loss progress below one point, in units of the starting size.
    pub morale_remainder: u64,
    /// Heads at the start, the player included on side 0.
    pub start_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleStack {
    pub line: Id,
    pub level: usize,
    pub count: u64,
    /// Damage taken short of one more loss.
    pub remainder: u64,
}

/// One piece of equipment: which item it is, and whether it is worn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gear {
    pub item: Id,
    pub equipped: bool,
    /// Improvement tier; 0 is the item as defined.
    pub tier: usize,
    /// The one enchantment laid on it, for good.
    pub enchantment: Option<Id>,
}

/// A learned technique's 1-based rank and its technique XP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniqueState {
    pub rank: usize,
    pub xp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vitals {
    pub hp: u32,
    pub mp: u32,
}

/// A fight on the paused initiative timeline. It owns every participant's
/// vitals until it ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encounter {
    /// Current timeline time, in ticks.
    pub now: u64,
    /// The player first, then opponents in engage order; ties break by
    /// `(next_time, side, position)`.
    pub participants: Vec<Participant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    /// The timeline pauses for a command.
    Player,
    /// The engine resolves an authored policy immediately.
    Policy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub character: Id,
    /// 0 is the player's side.
    pub side: u32,
    pub control: Control,
    pub hp: u32,
    pub mp: u32,
    pub rage: u32,
    /// Regeneration progress toward the next MP point, so rounding loses nothing.
    pub mp_remainder: u64,
    /// Damage-rage progress toward the next rage point.
    pub rage_remainder: u64,
    pub next_time: u64,
    /// Stopped fighting, alive, in a yielding group; out of the schedule.
    pub yielded: bool,
}

impl Participant {
    /// Still taking turns: alive and not yielded.
    pub fn fighting(&self) -> bool {
        self.hp > 0 && !self.yielded
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueState {
    pub npc: Id,
    pub node: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameState {
    pub player: PlayerState,
    pub combat: Option<CombatState>,
    pub quests: BTreeMap<Id, QuestStatus>,
    pub flags: BTreeSet<Id>,
    pub dialogue: Option<DialogueState>,
    pub turn: u64,
    /// Present only in worlds with random content.
    pub rng: Option<RngState>,
    /// The current minute of world time; present only in worlds with a clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
    /// Where each character who moves is now.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub whereabouts: BTreeMap<Id, Id>,
    /// Currency and prices; present only in worlds with an economy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economy: Option<EconomyState>,
    /// Soldiers and recruiting pools; present only in worlds with troops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retinue: Option<RetinueState>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetinueState {
    /// Soldiers by line, then 1-based level. A squad is never empty.
    pub roster: BTreeMap<Id, BTreeMap<usize, Squad>>,
    /// Recruits left at each recruiting place, by line.
    pub pools: BTreeMap<Id, BTreeMap<Id, u64>>,
}

/// The soldiers of one line at one level, who share their XP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Squad {
    pub healthy: u64,
    pub wounded: u64,
    pub xp: u64,
}

impl RetinueState {
    /// Healthy and wounded soldiers in every squad.
    pub fn heads(&self) -> u64 {
        self.roster
            .values()
            .flat_map(|levels| levels.values())
            .map(Squad::heads)
            .sum()
    }
}

impl Squad {
    pub fn heads(&self) -> u64 {
        self.healthy + self.wounded
    }
    /// Each soldier's part of the XP, rounded down.
    pub fn share(&self) -> u64 {
        self.xp.checked_div(self.heads()).unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomyState {
    pub currency: u64,
    /// Each market's price index for every good, by location then item, in
    /// thousandths of the base price.
    pub prices: BTreeMap<Id, BTreeMap<Id, u32>>,
}

pub const SAVE_FORMAT_VERSION: u32 = 14;
/// Format 1 has one implicit player route; saves name it explicitly.
pub const DEFAULT_ROUTE: &str = "default";

/// Storage-neutral save: deterministic mutable state plus the exact package it
/// belongs to. Clients decide where it is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSnapshot {
    pub save_format_version: u32,
    pub package_id: Id,
    pub package_revision: String,
    pub player_route_id: Id,
    pub state: GameState,
}
