//! Synchronous gameplay; no generation or presentation dependencies.

use realmkit_spec::{
    Channel, Character, Condition, Dialogue, DialogueChoice, DialogueNode, Direction, Effect, Id,
    ItemStack, MapPoint, Proficiency, Quest, QuestObjective, QuestStatus, Resource, Respec, Skill,
    SpecError, Stat, Stats, WorldSpec, BASIC_POWER,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod battle;
mod character;
mod command;
mod encounter;
mod error;
mod formulas;
mod gear;
mod realtime;
mod rng;
mod rules;
mod save;
mod settle;
mod state;
mod techniques;

pub use character::Pools;
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

    /// A new playthrough whose random draws follow `seed`, in a world that
    /// asks no start questions; see [`Engine::start`].
    pub fn new_with_seed(world: &'w WorldSpec, seed: u64) -> Result<Self, EngineError> {
        Self::start(world, seed, &[])
    }

    /// A new playthrough whose random draws follow `seed`, with `choices`
    /// answering each start question in order by an option ID. The same
    /// seed, choices and commands always give the same events and state.
    pub fn start(world: &'w WorldSpec, seed: u64, choices: &[Id]) -> Result<Self, EngineError> {
        let mut engine = Self::initial(world, seed)?;
        let questions = &world.world.start_questions;
        if choices.len() != questions.len() {
            return Err(EngineError::StartChoices);
        }
        // What the answers grant shows in the panels, not as events.
        let mut ignored = Vec::new();
        for (question, choice) in questions.iter().zip(choices) {
            let option = question
                .options
                .iter()
                .find(|o| &o.id == choice)
                .ok_or(EngineError::StartChoices)?;
            rules::apply_effects(world, &mut engine.state, &option.effects, &mut ignored)?;
        }
        // A flag an answer set may open a breakthrough gate, as after any command.
        techniques::promote(world, &mut engine.state, &mut ignored);
        engine.state.start_choices = choices.to_vec();
        // Vitals start at the maxima that techniques' passives give.
        if let Some(combat) = engine.state.combat.as_mut() {
            character::restore(world, combat);
        }
        Ok(engine)
    }

    /// The route's state before any start question is answered.
    fn initial(world: &'w WorldSpec, seed: u64) -> Result<Self, EngineError> {
        world.validate()?;
        let combat = world.combat().map(|combat| {
            let stats = combat.levels[0].stats;
            let pools = character::resources::player_pools(world, &stats);
            CombatState {
                xp: 0,
                level: 1,
                defeated: BTreeSet::new(),
                allocation: BTreeMap::new(),
                techniques: BTreeMap::new(),
                gear: BTreeMap::new(),
                next_gear: 1,
                vitals: character::resources::full(&stats, pools),
                stance: Stance::Exploring,
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
                // Every recruiting pool starts full.
                retinue: world.troops().map(|_| RetinueState {
                    roster: BTreeMap::new(),
                    pools: world
                        .locations
                        .iter()
                        .filter_map(|l| {
                            let recruits = l.recruits.as_ref()?;
                            let pools = recruits.troops.iter().map(|o| (o.line.clone(), o.size));
                            Some((l.id.clone(), pools.collect()))
                        })
                        .collect(),
                }),
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
                    prosperity: match &economy.prosperity {
                        Some(rules) => economy
                            .markets
                            .iter()
                            .map(|m| (m.location.clone(), m.prosperity.unwrap_or(rules.base)))
                            .collect(),
                        None => BTreeMap::new(),
                    },
                    stock: BTreeMap::new(),
                    workshops: BTreeMap::new(),
                    bank: economy.banking.as_ref().map(|_| BankAccount::default()),
                }),
                proficiencies: BTreeMap::new(),
                start_choices: Vec::new(),
                evidence: BTreeSet::new(),
                // The story opens in its first phase.
                phases: world
                    .world
                    .phases
                    .iter()
                    .take(1)
                    .map(|p| p.id.clone())
                    .collect(),
                outcome: None,
                taken_choices: BTreeSet::new(),
                standing: world
                    .world
                    .standing
                    .iter()
                    .map(|track| (track.id.clone(), rules::initial_standing(world, track)))
                    .collect(),
                at_war: world
                    .world
                    .diplomacy
                    .iter()
                    .flat_map(|d| &d.at_war)
                    .map(realmkit_spec::faction_pair)
                    .collect(),
                companions: BTreeMap::new(),
            },
        };
        // Merchants open with their stock at its targets, drawing nothing.
        rules::stock_up(world, &mut engine.state);
        // Starting techniques and gear.
        if let Some(rules) = world.combat() {
            let mut ignored = Vec::new();
            for grant in &rules.player_techniques {
                techniques::grant(world, &mut engine.state, grant, &mut ignored)?;
            }
            gear::receive_starting(world, engine.state.combat.as_mut().unwrap())?;
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
        Some(character::player_stats(self.world, combat))
    }
    /// The resources the player uses and their ceilings; `None` without combat.
    pub fn player_pools(&self) -> Option<Pools> {
        let stats = self.player_stats()?;
        Some(character::resources::player_pools(self.world, &stats))
    }
    /// The resources a fighter (the player or a character with a combat
    /// profile) uses, and their ceilings at its own stats.
    pub fn pools_of(&self, character: &str) -> Option<Pools> {
        if character == self.world.world.player {
            return self.player_pools();
        }
        let profile = self.world.character(character)?.combat.as_ref()?;
        self.world.combat()?;
        Some(character::resources::profile_pools(self.world, profile))
    }
    /// Stat points granted so far and not yet spent; `None` without combat.
    pub fn unspent_points(&self) -> Option<u32> {
        let combat = self.state.combat.as_ref()?;
        Some(rules::unspent_points(self.world, combat))
    }
    /// The player's rank in a proficiency; 0 if never gained.
    pub fn proficiency_rank(&self, proficiency: Proficiency) -> u32 {
        rules::rank(&self.state, proficiency)
    }
    /// Proficiency points granted so far and not yet trained.
    pub fn unspent_proficiency_points(&self) -> u32 {
        rules::unspent_proficiency_points(self.world, &self.state)
    }
    /// The player's current HP, MP and Rage; `None` without combat.
    pub fn player_vitals(&self) -> Option<Vitals> {
        rules::player_vitals(&self.state)
    }
    /// A participant's current vitals in the active encounter: the
    /// player's own, or an opponent's for this fight.
    pub fn vitals_of(&self, participant: &Participant) -> Option<Vitals> {
        participant.vitals.or_else(|| self.player_vitals())
    }
    /// The active encounter, if any.
    pub fn encounter(&self) -> Option<&Encounter> {
        rules::fighting(&self.state)
    }
    /// The real-time fight, if any.
    pub fn realtime(&self) -> Option<&RealtimeEncounter> {
        match &self.state.combat.as_ref()?.stance {
            Stance::Realtime(duel) => Some(duel),
            Stance::Exploring | Stance::Fighting(_) | Stance::Battle(_) => None,
        }
    }
    /// The named companions travelling with the player now, by ID.
    pub fn party(&self) -> Vec<&Id> {
        let party = self.state.companions.iter().filter(|(_, c)| c.in_party());
        party.map(|(id, _)| id).collect()
    }
    /// A recruited companion's record: where it is, its vitals and its
    /// standing order. `None` for a companion never recruited.
    pub fn companion(&self, id: &str) -> Option<&CompanionState> {
        self.state.companions.get(id)
    }
    /// A real-time fighter's current vitals: its own for the fight, or the
    /// player's or a named companion's only copy.
    pub fn fighter_vitals(&self, i: usize) -> Option<Vitals> {
        let fight = self.realtime()?;
        (i < fight.fighters.len()).then(|| *fight.vitals(i, &self.state))
    }
    /// The battle the player is leading, if any.
    pub fn battle(&self) -> Option<&BattleState> {
        match &self.state.combat.as_ref()?.stance {
            Stance::Battle(battle) => Some(battle),
            Stance::Exploring | Stance::Fighting(_) | Stance::Realtime(_) => None,
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
        let mut engine = Self::initial(world, 0)?;
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

    /// The choices in the active conversation, numbered from 1 as
    /// `ChooseDialogue` takes them; empty when none. A choice listed with its
    /// `blocked` reason is refused.
    pub fn dialogue_choices(&self) -> Vec<DialogueOption> {
        self.state.dialogue.as_ref().map_or_else(Vec::new, |d| {
            rules::choices(self.world, &self.state, &d.npc, &d.node)
                .iter()
                .map(|c| c.option())
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

    /// The overland map as the player may know it; `None` in a world whose
    /// places have no positions.
    pub fn map_view(&self) -> Option<MapView> {
        rules::map_view(self.world, &self.state)
    }

    /// Whether the player knows of a place: where they stand, or its
    /// `known_when` holds. Clients name only known places, roads included.
    pub fn knows(&self, location: &str) -> bool {
        rules::known(self.world, &self.state, location)
    }

    /// The player's journal: phase, known quests, evidence and outcome.
    pub fn journal(&self) -> Journal {
        rules::journal(self.world, &self.state)
    }

    /// How the player currently reads known evidence: the index of its last
    /// interpretation whose condition holds. `None` while the evidence is
    /// unknown or has no interpretations.
    pub fn evidence_reading(&self, evidence: &str) -> Option<usize> {
        rules::reading(self.world, &self.state, evidence)
    }

    /// The facts of known evidence the player has learned, by index in
    /// authored order: those whose `when` holds. Empty while the evidence is
    /// unknown. Derived, never saved; a fact once known stays known.
    pub fn evidence_facts(&self, evidence: &str) -> Vec<usize> {
        rules::facts(self.world, &self.state, evidence)
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
