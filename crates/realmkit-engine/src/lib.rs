//! Synchronous gameplay; no generation or presentation dependencies.

use realmkit_spec::{
    Channel, Character, Condition, DialogueChoice, Direction, Effect, Id, ItemStack,
    QuestObjective, QuestStatus, Resource, Respec, Skill, SpecError, Stat, Stats, WorldSpec,
    BASIC_POWER,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod command;
mod encounter;
mod error;
mod formulas;
mod gear;
mod rng;
mod rules;
mod save;
mod state;
mod techniques;

pub use command::*;
pub use error::EngineError;
pub use formulas::*;
pub use rng::{splitmix64, RngState, RNG_VERSION};
pub use state::*;

#[derive(Clone)]
pub struct Engine<'w> {
    world: &'w WorldSpec,
    state: GameState,
}

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
                gear: BTreeMap::new(),
                next_gear: 1,
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
                rng: RngState::for_world(world, seed),
                time: world.world.time.as_ref().map(|t| t.start),
                // Each mover starts where it is placed.
                whereabouts: world
                    .characters
                    .iter()
                    .filter(|c| c.moves.is_some())
                    .filter_map(|c| {
                        let start = world
                            .locations
                            .iter()
                            .find(|l| l.characters.contains(&c.id))?;
                        Some((c.id.clone(), start.id.clone()))
                    })
                    .collect(),
                economy: world.economy().map(|economy| EconomyState {
                    currency: economy.currency.start,
                    prices: economy
                        .markets
                        .iter()
                        .map(|m| {
                            let prices = economy.goods.iter().map(|g| {
                                let index = m.prices.get(&g.item).copied();
                                (g.item.clone(), index.unwrap_or(realmkit_spec::BASE_INDEX))
                            });
                            (m.location.clone(), prices.collect())
                        })
                        .collect(),
                }),
            },
        };
        // Starting techniques, then vitals at the maxima their passives give.
        if let Some(rules) = world.combat() {
            let mut ignored = Vec::new();
            for grant in &rules.player_techniques {
                techniques::grant(world, &mut engine.state, grant, &mut ignored)?;
            }
            let combat = engine.state.combat.as_mut().unwrap();
            gear::receive_starting(world, combat)?;
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
        let spends_time = !rules::is_panel(&command);
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

    /// Characters at the player's location, whatever their conditions: those
    /// placed here, then characters who move and are here now.
    pub fn placed_here(&self) -> Vec<&'w Id> {
        rules::placed_here(self.world, &self.state)
    }

    /// A good's prices at the open market here: one unit now, and the next
    /// after it. `None` away from an open market or for an untraded good.
    pub fn quote(&self, good: &str) -> Option<Quote> {
        rules::quote(self.world, &self.state, good)
    }

    /// One unit's fixed price of a ware at the open market here; `None` away
    /// from an open market or for an item not sold here.
    pub fn ware_price(&self, item: &str) -> Option<u64> {
        rules::ware_price(self.world, &self.state, item)
    }

    /// Who is here now: present under their conditions and not defeated.
    pub fn present_here(&self) -> Vec<&'w Character> {
        rules::present_here(self.world, &self.state)
    }

    /// Whether a condition holds now; evaluating it changes nothing.
    pub fn holds(&self, condition: &Condition) -> bool {
        rules::holds(&self.state, condition)
    }
    /// Whether an optional requirement is met; an absent one always is.
    pub fn allows(&self, requires: Option<&Condition>) -> bool {
        rules::allowed(&self.state, requires)
    }
}
