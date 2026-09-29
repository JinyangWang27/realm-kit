//! Synchronous gameplay; no generation or presentation dependencies.

use realmkit_spec::{
    Condition, DialogueChoice, DialogueEffect, Direction, Id, ItemStack, QuestObjective,
    QuestStatus, SpecError, WorldSpec,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Look,
    Move(Direction),
    Attack(Id),
    Talk(Id),
    /// One-based index into the currently visible choices.
    ChooseDialogue(usize),
    AcceptQuest(Id),
    CompleteQuest(Id),
    Inventory,
    Status,
    Quests,
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
    DamageDealt {
        target: Id,
        amount: u32,
        variant: usize,
    },
    DamageReceived {
        source: Id,
        amount: u32,
        variant: usize,
    },
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
    LevelUp {
        level: usize,
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
    pub hp: u32,
    pub max_hp: u32,
    pub attack: u32,
    pub xp: u64,
    pub level: usize,
    pub inventory: BTreeMap<Id, u64>,
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
    pub monster_hp: BTreeMap<Id, u32>,
    pub quests: BTreeMap<Id, QuestStatus>,
    pub flags: BTreeSet<Id>,
    pub dialogue: Option<DialogueState>,
    pub turn: u64,
}

pub const SAVE_FORMAT_VERSION: u32 = 1;
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

mod rules;
mod save;

impl<'w> Engine<'w> {
    pub fn new(world: &'w WorldSpec) -> Result<Self, EngineError> {
        world.validate()?;
        let stats = &world.combat().expect("combat world").levels[0];
        Ok(Self {
            world,
            state: GameState {
                player: PlayerState {
                    location: world.world.start.clone(),
                    hp: stats.hp,
                    max_hp: stats.hp,
                    attack: stats.attack,
                    xp: 0,
                    level: 1,
                    inventory: BTreeMap::new(),
                },
                monster_hp: world
                    .characters
                    .iter()
                    .filter_map(|c| Some((c.id.clone(), c.combat.as_ref()?.hp)))
                    .collect(),
                quests: world
                    .quests
                    .iter()
                    .map(|q| (q.id.clone(), QuestStatus::Available))
                    .collect(),
                flags: BTreeSet::new(),
                dialogue: None,
                turn: 0,
            },
        })
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }
    pub fn world(&self) -> &'w WorldSpec {
        self.world
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
            Command::Look | Command::Inventory | Command::Status | Command::Quests
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
