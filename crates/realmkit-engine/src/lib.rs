//! Synchronous gameplay; no generation or presentation dependencies.

use realmkit_spec::{
    Channel, Character, Condition, DialogueChoice, DialogueEffect, Direction, Id, ItemStack,
    QuestObjective, QuestStatus, Resource, Respec, Skill, SpecError, Stat, Stats, WorldSpec,
    BASIC_POWER,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Look,
    Move(Direction),
    /// Starts an encounter with a fighter at the current location.
    Engage(Id),
    /// In an encounter: a basic attack in the player's basic-attack channel.
    Attack(Id),
    UseSkill {
        skill: Id,
        target: Id,
    },
    /// In an encounter: spend this turn turning to run; the escape happens
    /// at the player's next turn if they live.
    Flee,
    /// Restores HP and MP at a safe location.
    Rest,
    /// Spends unspent stat points on one stat.
    Allocate {
        stat: Stat,
        points: u32,
    },
    /// Refunds every spent stat point, where the world allows it.
    Respec,
    Talk(Id),
    /// One-based index into the currently visible choices.
    ChooseDialogue(usize),
    AcceptQuest(Id),
    CompleteQuest(Id),
    Inventory,
    Status,
    Quests,
    /// Lists learned techniques and their ranks; only in worlds with techniques.
    Techniques,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    LocationViewed {
        location: Id,
    },
    Moved {
        from: Id,
        to: Id,
    },
    /// `skill` is `None` for a basic attack, which uses narrative `variant`.
    DamageDealt {
        target: Id,
        amount: u32,
        variant: usize,
        skill: Option<Id>,
        critical: bool,
    },
    DamageReceived {
        source: Id,
        amount: u32,
        variant: usize,
        skill: Option<Id>,
        critical: bool,
    },
    ResourceSpent {
        character: Id,
        resource: Resource,
        amount: u32,
    },
    EncounterStarted {
        opponents: Vec<Id>,
    },
    FleeStarted,
    /// A participant in a yielding group stops fighting, alive.
    Yielded {
        character: Id,
    },
    /// Any rewards follow a victory.
    EncounterEnded {
        outcome: Outcome,
    },
    Rested,
    PointsAllocated {
        stat: Stat,
        points: u32,
    },
    PointsRefunded,
    TechniqueLearned {
        technique: Id,
    },
    /// `rank` is 1-based; its authored name is shown.
    TechniqueRankUp {
        technique: Id,
        rank: usize,
    },
    TechniqueXpGained {
        technique: Id,
        amount: u64,
    },
    TechniquesViewed,
    EnemyDefeated {
        monster: Id,
    },
    ItemReceived {
        item: Id,
        quantity: u64,
    },
    ExperienceGranted {
        amount: u64,
    },
    /// Levelling up restores HP, and MP when the player has any.
    LevelUp {
        level: usize,
        mp_restored: bool,
    },
    PlayerDied,
    Dialogue {
        npc: Id,
        node: Id,
        choices: Vec<String>,
    },
    DialogueEnded,
    QuestAccepted {
        quest: Id,
    },
    QuestProgressed {
        quest: Id,
    },
    QuestCompleted {
        quest: Id,
    },
    StoryFlagSet {
        flag: Id,
    },
    InventoryViewed,
    StatusViewed,
    QuestsViewed,
}

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
    pub stance: Stance,
}

/// Where the player's HP and MP live: exactly one owner at a time, so a save
/// never holds two copies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Stance {
    Exploring(Vitals),
    Fighting(Encounter),
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

/// How an encounter ended; the player's death leaves it open instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every opponent died or yielded.
    Victory,
    /// The player yielded.
    Yielded,
    Fled,
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
}

pub const SAVE_FORMAT_VERSION: u32 = 8;
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

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    World(#[from] SpecError),
    #[error("there is no exit in that direction")]
    NoExit,
    #[error("that exit is locked")]
    ExitLocked { location: Id, direction: Direction },
    #[error("{0} is not available here")]
    NotHere(Id),
    #[error("{0} has already been defeated")]
    AlreadyDefeated(Id),
    #[error("you do not know that skill: {0}")]
    UnknownSkill(Id),
    #[error("you have not reached the level for {0}")]
    SkillLocked(Id),
    #[error("not enough MP for {0}")]
    NotEnoughMp(Id),
    #[error("not enough rage for {0}")]
    NotEnoughRage(Id),
    #[error("finish the fight first")]
    InEncounter,
    #[error("there is no running from this fight")]
    NoFlee,
    #[error("you cannot spend points on that stat")]
    NoSuchStat,
    #[error("not enough unspent stat points")]
    NotEnoughPoints,
    #[error("that stat cannot take more points")]
    PointCap,
    #[error("stat points cannot be refunded here")]
    NoRespec,
    #[error("you are not fighting anyone; engage first")]
    NotFighting,
    #[error("this is not a safe place to rest")]
    NotSafe,
    #[error("there is no active conversation")]
    NoDialogue,
    #[error("choose one of the displayed options")]
    InvalidChoice,
    #[error("quest cannot be accepted or completed in its current state: {0}")]
    QuestState(Id),
    #[error("unknown quest: {0}")]
    UnknownQuest(Id),
    #[error("the player is dead; start a new game")]
    PlayerDead,
    #[error("numeric limit exceeded; command was not applied")]
    NumericLimit,
    #[error("save cannot be loaded: {0}")]
    InvalidSave(String),
}

/// A command a client may offer in the current scene. Unavailable actions are
/// shown for explanation; the engine still rechecks legality on execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub command: Command,
    pub available: bool,
}

#[derive(Clone)]
pub struct Engine<'w> {
    world: &'w WorldSpec,
    state: GameState,
}

mod encounter;
mod rng;
mod rules;
mod save;
mod techniques;

pub use rng::{splitmix64, RngState, RNG_VERSION};

impl<'w> Engine<'w> {
    /// A new playthrough with seed 0; see [`Engine::new_with_seed`].
    pub fn new(world: &'w WorldSpec) -> Result<Self, EngineError> {
        Self::new_with_seed(world, 0)
    }

    /// A new playthrough whose random draws follow `seed`. The same seed and
    /// commands always give the same events and state.
    pub fn new_with_seed(world: &'w WorldSpec, seed: u64) -> Result<Self, EngineError> {
        world.validate()?;
        let combat = world.combat().map(|combat| {
            let stats = combat.levels[0].stats;
            CombatState {
                xp: 0,
                level: 1,
                defeated: BTreeSet::new(),
                allocation: BTreeMap::new(),
                techniques: BTreeMap::new(),
                stance: Stance::Exploring(Vitals {
                    hp: stats.hp,
                    mp: stats.mp,
                }),
            }
        });
        let mut engine = Self {
            world,
            state: GameState {
                player: PlayerState {
                    location: world.world.start.clone(),
                    inventory: BTreeMap::new(),
                },
                combat,
                quests: world
                    .quests
                    .iter()
                    .map(|q| (q.id.clone(), QuestStatus::Available))
                    .collect(),
                flags: BTreeSet::new(),
                dialogue: None,
                turn: 0,
                rng: world.stochastic().then(|| RngState::new(seed)),
            },
        };
        // Starting techniques, then vitals at the maxima their passives give.
        if let Some(rules) = world.combat() {
            let mut ignored = Vec::new();
            for grant in &rules.player_techniques {
                techniques::grant(world, &mut engine.state, grant, &mut ignored)?;
            }
            let combat = engine.state.combat.as_mut().unwrap();
            let max = rules::player_stats(world, combat);
            combat.stance = Stance::Exploring(Vitals {
                hp: max.hp,
                mp: max.mp,
            });
        }
        Ok(engine)
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }
    pub fn world(&self) -> &'w WorldSpec {
        self.world
    }
    /// The player's effective stats: the level table plus allocated points;
    /// `None` without combat.
    pub fn player_stats(&self) -> Option<Stats> {
        let combat = self.state.combat.as_ref()?;
        Some(rules::player_stats(self.world, combat))
    }
    /// Stat points granted so far and not yet spent; `None` without combat.
    pub fn unspent_points(&self) -> Option<u32> {
        let combat = self.state.combat.as_ref()?;
        Some(rules::unspent_points(self.world, combat))
    }
    /// The player's current HP and MP, wherever they live; `None` without combat.
    pub fn player_vitals(&self) -> Option<Vitals> {
        rules::player_vitals(&self.state)
    }
    /// The active encounter, if any.
    pub fn encounter(&self) -> Option<&Encounter> {
        match &self.state.combat.as_ref()?.stance {
            Stance::Fighting(encounter) => Some(encounter),
            Stance::Exploring(_) => None,
        }
    }
    /// The next `n` actors, projected as if every action took a basic
    /// action's time. A projection, not a promise: skills with other times
    /// change it.
    pub fn turn_order(&self, n: usize) -> Vec<Id> {
        encounter::turn_order(self.world, &self.state, n)
    }
    /// Only a world with combat can kill the player.
    pub fn is_dead(&self) -> bool {
        rules::dead(&self.state)
    }

    /// Captures the playthrough without changing it, so saving spends no time.
    pub fn snapshot(&self) -> SaveSnapshot {
        SaveSnapshot {
            save_format_version: SAVE_FORMAT_VERSION,
            package_id: self.world.world.id.clone(),
            package_revision: self.world.revision(),
            player_route_id: DEFAULT_ROUTE.into(),
            state: self.state.clone(),
        }
    }

    /// Resumes a snapshot, or rejects it whole: a save for another package,
    /// revision or route, or with state this world could not produce, never loads.
    pub fn restore(world: &'w WorldSpec, snapshot: SaveSnapshot) -> Result<Self, EngineError> {
        let mut engine = Self::new(world)?;
        save::check(world, &engine.state, &snapshot).map_err(EngineError::InvalidSave)?;
        engine.state = snapshot.state;
        Ok(engine)
    }

    pub fn execute(&mut self, command: Command) -> Result<Vec<Event>, EngineError> {
        // ponytail: clone for atomic commands; use a change set if worlds become large.
        let mut next = self.state.clone();
        let spends_time = !matches!(
            command,
            Command::Look
                | Command::Inventory
                | Command::Status
                | Command::Quests
                | Command::Techniques
        );
        let events = rules::execute(self.world, &mut next, command)?;
        if spends_time {
            next.turn = next.turn.checked_add(1).ok_or(EngineError::NumericLimit)?;
        }
        self.state = next;
        Ok(events)
    }

    /// Location-level actions in display order: talk, attack, travel, panels.
    pub fn actions(&self) -> Vec<Action> {
        rules::actions(self.world, &self.state)
    }

    /// Text of the choices in the active conversation; empty when none.
    pub fn dialogue_choices(&self) -> Vec<&'w str> {
        self.state.dialogue.as_ref().map_or_else(Vec::new, |d| {
            rules::choices(self.world, &self.state, &d.npc, &d.node)
                .into_iter()
                .map(|c| c.text.as_str())
                .collect()
        })
    }

    pub fn conditions_met(&self, conditions: &[Condition]) -> bool {
        rules::conditions_met(&self.state, conditions)
    }
}

/// Damage of one landed hit: the channel's combined attack `A` against its
/// combined defence `D` (each 100 × main + share × other), then
/// `max(1, A × power × A / (100 × 100 × (A + D)))`, rounded down once.
/// Defence equal to the attack halves damage; no scale constant or level enters.
pub fn damage(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
) -> Result<u32, EngineError> {
    damage_scaled(attacker, defender, channel, power, share, 100)
}

/// [`damage`] scaled by `multiplier` percent (a critical hit) before the
/// single final rounding.
pub fn damage_scaled(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
    multiplier: u32,
) -> Result<u32, EngineError> {
    let a = attacker.combined(channel, share, false);
    let d = defender.combined(channel, share, true);
    let hit = a
        .checked_mul(u128::from(power))
        .and_then(|v| v.checked_mul(a))
        .and_then(|v| v.checked_mul(u128::from(multiplier)))
        .and_then(|v| v.checked_div(100 * 100 * 100 * (a + d)))
        .ok_or(EngineError::NumericLimit)?;
    u32::try_from(hit.max(1)).map_err(|_| EngineError::NumericLimit)
}

/// XP for defeating an opponent, by level difference `opponent − player`:
/// ±10% per level, capped at ±40%, rounded down, and nothing five or more
/// levels below. Mirrors `scripts/combat_sim` `XpRules.for_kill`.
pub fn xp_for_defeat(xp: u64, player_level: usize, opponent_level: usize) -> u64 {
    let diff = opponent_level as i128 - player_level as i128;
    if diff <= -5 {
        return 0;
    }
    let scaled = u128::from(xp) * (10 + diff.clamp(-4, 4)) as u128 / 10;
    // At most 140% of a u64 value: saturate rather than wrap on absurd content.
    u64::try_from(scaled).unwrap_or(u64::MAX)
}
