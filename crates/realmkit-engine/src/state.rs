//! The deterministic playthrough state and the save that carries it.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerState {
    pub location: Id,
    pub inventory: BTreeMap<Id, u64>,
}

/// The player as a fighting character: progression, gear and vitals, which
/// hold however a fight is run, and the stance, which is how one is being
/// run now. Exists only in worlds with a combat block.
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
    /// The player's HP, MP and Rage: their only copy, whatever the stance.
    pub vitals: Vitals,
    pub stance: Stance,
}

/// How the player is fighting, if at all. A stance never holds the player's
/// vitals; fights read and change [`CombatState::vitals`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Stance {
    Exploring,
    /// A personal fight on the turn-based initiative timeline.
    Fighting(Encounter),
    /// A real-time fight on its own fixed-step clock.
    Realtime(RealtimeEncounter),
    /// Leading a mass battle.
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
    /// Rounds fought so far. At 0 HP the player is knocked out of the
    /// battle, which goes on without them.
    pub round: u32,
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

/// A fighting character's current HP and resources. Maxima are derived
/// from stats, never saved; a resource the character does not use stays 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vitals {
    pub hp: u32,
    pub mp: u32,
    pub rage: u32,
    /// Damage-rage progress toward the next point, in units of maximum HP.
    pub rage_remainder: u64,
}

/// A fight on the paused initiative timeline: the turn-based layer's
/// schedule over characters whose vitals it does not copy.
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
    /// Acts on the player's commands; the timeline pauses for one.
    Player,
    /// The engine acts by the character's authored policy.
    Policy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub character: Id,
    /// 0 is the player's side.
    pub side: u32,
    pub control: Control,
    /// An opponent's vitals, which last only as long as the encounter.
    /// `None` for the player, whose vitals stay in [`CombatState::vitals`].
    pub vitals: Option<Vitals>,
    /// Timeline regeneration progress toward the next MP point, so rounding
    /// loses nothing; it belongs to encounter time and ends with it.
    pub mp_remainder: u64,
    pub next_time: u64,
    /// Stopped fighting, alive, in a yielding group; out of the schedule.
    pub yielded: bool,
}

impl Encounter {
    /// Participant `i`'s vitals: an opponent's own, or the player's.
    pub fn vitals<'a>(&'a self, i: usize, player: &'a Vitals) -> &'a Vitals {
        self.participants[i].vitals.as_ref().unwrap_or(player)
    }
    /// Whether participant `i` still takes turns: alive and not yielded.
    pub fn fighting(&self, i: usize, player: &Vitals) -> bool {
        self.vitals(i, player).hp > 0 && !self.participants[i].yielded
    }
}

/// A fight on the real-time layer's clock: the steps it has run and what
/// each fighter is doing. Fighters on different teams are hostile; fighters
/// can join while it runs. Like an encounter, it copies no vitals but those
/// of the characters who are neither the player nor a named companion,
/// which last only as long as the fight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealtimeEncounter {
    /// Steps of fight time so far. Only an advance moves it; it is neither
    /// world time nor encounter time.
    pub step: u64,
    /// In join order: the player, the opponents the fight started with,
    /// then everyone who joined, as they did. Nobody is ever removed.
    pub fighters: Vec<Fighter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fighter {
    pub character: Id,
    /// 0 is the player's team; the opponents it started with are team 1.
    pub team: u32,
    pub control: Control,
    /// The step it joined at; 0 for those the fight started with.
    pub joined: u64,
    /// This fighter's vitals for this fight; `None` for the player, whose
    /// vitals stay in [`CombatState::vitals`], and for a named companion,
    /// whose vitals stay in its [`CompanionState`].
    pub vitals: Option<Vitals>,
    /// Fight-time regeneration progress toward the next MP point; it ends
    /// with the fight.
    pub mp_remainder: u64,
    /// What it is committed to; `Ready` for good once it is out.
    pub action: RealtimeAction,
    /// Stopped fighting, alive, in a yielding group.
    pub yielded: bool,
}

/// What a fighter is committed to. Each timed state ends when the clock
/// reaches its `until` step; the fighter is then in the next one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RealtimeAction {
    /// Free to act.
    Ready,
    /// A basic attack (`skill` is `None`) or a skill, already paid for,
    /// aimed at a hostile `target`.
    Attack {
        skill: Option<Id>,
        target: Id,
        phase: AttackPhase,
        until: u64,
        /// Whether this attack has landed; each lands at most once.
        connected: bool,
    },
    /// A guard, which stops hits from step `ready_at` until it is lowered.
    Guard { ready_at: u64 },
    /// Turning away: the player is gone at step `until` if still fighting.
    Retreat { until: u64 },
    /// A parry: it deflects contact before step `open_until`, then leaves
    /// the fighter committed and unprotected until `until`. A parry that
    /// deflected something skips that recovery (`until == open_until`).
    Parry { open_until: u64, until: u64 },
    /// A dodge: contact before step `open_until` finds nothing; then
    /// committed and unprotected until `until`.
    Dodge { open_until: u64, until: u64 },
    /// Interrupted: it can do nothing until `until`.
    Staggered { until: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackPhase {
    /// Committed; cannot connect yet.
    Windup,
    /// Can connect once, when the host reports contact.
    Active,
    /// Committed; can no longer connect.
    Recovery,
}

impl RealtimeEncounter {
    /// Fighter `i`'s vitals: its own for this fight, or, for the player and
    /// named companions, their only copy, which `state` holds outside it.
    pub fn vitals<'a>(&'a self, i: usize, state: &'a GameState) -> &'a Vitals {
        let fighter = &self.fighters[i];
        match (&fighter.vitals, i) {
            (Some(vitals), _) => vitals,
            (None, 0) => &state.combat.as_ref().unwrap().vitals,
            (None, _) => &state.companions[&fighter.character].vitals,
        }
    }
    /// Whether fighter `i` still fights: alive and not yielded.
    pub fn fighting(&self, i: usize, state: &GameState) -> bool {
        self.vitals(i, state).hp > 0 && !self.fighters[i].yielded
    }
    /// The fighter playing `character`, if it is in this fight.
    pub fn index(&self, character: &str) -> Option<usize> {
        self.fighters.iter().position(|f| f.character == character)
    }
    /// Whether anyone off the player's team still fights; the fight goes
    /// on while someone does.
    pub fn contested(&self, state: &GameState) -> bool {
        (1..self.fighters.len()).any(|i| self.fighters[i].team != 0 && self.fighting(i, state))
    }
}

/// A named companion once recruited: whether it travels with the player,
/// its HP, MP and Rage, and its standing tactical order. This is the
/// companion's only copy of its vitals; a fight it joins reads and changes
/// them here, so nothing it lost or regained is undone when the fight ends,
/// and leaving the party or joining it again restores nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionState {
    /// `None` while the companion travels in the party, wherever the player
    /// is; otherwise the location where it parted from the player and stays.
    pub parted_at: Option<Id>,
    pub vitals: Vitals,
    pub order: TacticalOrder,
}

impl CompanionState {
    /// Whether it travels with the player now.
    pub fn in_party(&self) -> bool {
        self.parted_at.is_none()
    }
    /// At 0 HP: it cannot join a fight until it recovers.
    pub fn downed(&self) -> bool {
        self.vitals.hp == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueState {
    pub npc: Id,
    pub node: Id,
}

/// A dialogue choice by IDs, which survive reordering its node's choices.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceRef {
    pub dialogue: Id,
    pub node: Id,
    pub choice: Id,
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
    /// Ranks in the proficiencies the world defines, once any is gained.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub proficiencies: BTreeMap<Proficiency, ProficiencyState>,
    /// The option chosen for each start question, in order, for display.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub start_choices: Vec<Id>,
    /// Evidence the player has discovered; present only in worlds that author it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub evidence: BTreeSet<Id>,
    /// The story phases reached so far: always the authored phases up to the
    /// current one, which is last. Empty in a world without phases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<Id>,
    /// The route outcome reached, for good; at most one per playthrough.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Id>,
    /// Ask-once dialogue choices the player has taken, never offered again.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub taken_choices: BTreeSet<ChoiceRef>,
    /// The player's value on every authored standing track.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub standing: BTreeMap<Id, Standing>,
    /// Pairs of factions at war, each in [`realmkit_spec::faction_pair`]
    /// order; every other pair is at peace. Only worlds with diplomacy have any.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub at_war: BTreeSet<[Id; 2]>,
    /// Every named companion ever recruited, in the party or parted from it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub companions: BTreeMap<Id, CompanionState>,
}

/// A track's value: one for a global track, one per faction for a faction track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Standing {
    Global(i32),
    Factions(BTreeMap<Id, i32>),
}

impl Standing {
    /// The value, with `faction` on a faction track; `None` for a scope or
    /// faction the track does not have.
    pub fn get(&self, faction: Option<&str>) -> Option<i32> {
        match (self, faction) {
            (Self::Global(value), None) => Some(*value),
            (Self::Factions(values), Some(faction)) => values.get(faction).copied(),
            _ => None,
        }
    }
    pub(crate) fn get_mut(&mut self, faction: Option<&str>) -> Option<&mut i32> {
        match (self, faction) {
            (Self::Global(value), None) => Some(value),
            (Self::Factions(values), Some(faction)) => values.get_mut(faction),
            _ => None,
        }
    }
}

/// A proficiency's rank, split by where it came from: points the player
/// trained, and ranks effects taught. Unspent points are derived from the level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProficiencyState {
    pub trained: u32,
    pub taught: u32,
}

impl ProficiencyState {
    pub fn rank(&self) -> u32 {
        self.trained.saturating_add(self.taught)
    }
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
    /// Each market's prosperity; present only in worlds that author it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prosperity: BTreeMap<Id, u32>,
    /// Each market's stock and purse; present only in worlds that author it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stock: BTreeMap<Id, MarketStock>,
    /// The player's workshops, by town then kind.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workshops: BTreeMap<Id, BTreeMap<Id, u32>>,
    /// The player's bank account; present only in worlds with banking.
    /// `currency` is what the player carries, apart from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank: Option<BankAccount>,
}

/// Money held at the bank. Interest follows the Gregorian month's daily
/// closing balances, so only their running sum is kept; how many days it
/// covers follows from the date.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankAccount {
    pub balance: u64,
    /// The sum of this month's daily closing balances so far.
    pub month_to_date: u64,
}

/// What a market's merchants hold: units of every good, and a purse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketStock {
    pub currency: u64,
    pub goods: BTreeMap<Id, u64>,
}

pub const SAVE_FORMAT_VERSION: u32 = 19;
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

impl SaveSnapshot {
    /// Reads a snapshot from JSON, rejecting another save format by its
    /// version, and the earlier vitals layout by its shape, before the typed
    /// parse, which would fail obscurely.
    pub fn from_json(bytes: &[u8]) -> Result<Self, EngineError> {
        let invalid = |e: serde_json::Error| EngineError::InvalidSave(e.to_string());
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(invalid)?;
        match value["save_format_version"].as_u64() {
            Some(version) if version != u64::from(SAVE_FORMAT_VERSION) => Err(
                EngineError::InvalidSave(format!("unsupported save format version {version}")),
            ),
            // Saves from before the character core kept the player's vitals
            // in the stance. They are refused by name, never reinterpreted.
            _ if value["state"]["combat"].is_object()
                && value["state"]["combat"].get("vitals").is_none() =>
            {
                Err(EngineError::InvalidSave(
                    "this save keeps the player's vitals in the stance, an earlier layout that is not migrated; start a new playthrough".into(),
                ))
            }
            _ => serde_json::from_value(value).map_err(invalid),
        }
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("snapshots always serialize")
    }
}
