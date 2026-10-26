//! The optional turn-based layer: encounters on the paused initiative
//! timeline. It owns who acts when (initiative, action delays, opponents'
//! policy) and how an encounter starts and ends; what an action does comes
//! from the character core. Every rule here mirrors
//! `scripts/combat_sim/encounter.py`; tests hold the two to the same numbers.

use super::*;
use crate::character::{profile, resources, Pools};
use realmkit_spec::{Group, Timeline};

mod action;
mod timeline;

use action::act;
pub(crate) use action::{check_skill, choose};
pub(super) use timeline::{baseline_turn, delay, turn_order};
use timeline::{next_actor, tick};

/// The player during an encounter: level (for skill unlocks), effective
/// stats and pools, which cannot change until the encounter ends.
#[derive(Clone, Copy)]
pub(super) struct Me {
    pub level: usize,
    pub stats: Stats,
    pub pools: Pools,
    /// A worn weapon's basic-attack channel and time, if any.
    pub basic: (Option<Channel>, Option<u32>),
    /// Worn damage modifiers for physical and special hits on the player.
    pub guard: [(u64, u64); 2],
}

impl Me {
    fn guard(&self, channel: Channel) -> (u64, u64) {
        self.guard[match channel {
            Channel::Physical => 0,
            Channel::Special => 1,
        }]
    }
}

/// The encounter beside the player's vitals, so each participant's vitals
/// have one owner: the encounter for opponents, the character for the player.
pub(super) struct Board<'s> {
    pub encounter: &'s mut Encounter,
    pub player: &'s mut Vitals,
}

impl Board<'_> {
    pub(super) fn vitals(&self, i: usize) -> &Vitals {
        self.encounter.vitals(i, self.player)
    }
    pub(super) fn vitals_mut(&mut self, i: usize) -> &mut Vitals {
        match self.encounter.participants[i].vitals.as_mut() {
            Some(vitals) => vitals,
            None => self.player,
        }
    }
    pub(super) fn fighting(&self, i: usize) -> bool {
        self.encounter.fighting(i, self.player)
    }
    pub(super) fn pools(&self, world: &WorldSpec, me: Me, i: usize) -> Pools {
        pools(world, me, &self.encounter.participants[i])
    }
}

/// A participant's stats: the player's effective stats, anyone else's
/// from their combat profile.
pub(super) fn stats(world: &WorldSpec, player: Stats, p: &Participant) -> Stats {
    match p.control {
        Control::Player => player,
        Control::Policy => profile(world, &p.character).stats,
    }
}

/// A participant's pools, as [`stats`].
pub(super) fn pools(world: &WorldSpec, me: Me, p: &Participant) -> Pools {
    match p.control {
        Control::Player => me.pools,
        Control::Policy => resources::profile_pools(world, profile(world, &p.character)),
    }
}

/// The world's timeline; a world without one has no turn-based fights.
pub(super) fn timeline(world: &WorldSpec) -> Result<&Timeline, EngineError> {
    world
        .combat()
        .and_then(|c| c.timeline.as_ref())
        .ok_or(EngineError::NoEncounters)
}

/// Starts an encounter with `id`, then lets opponents act until the player's
/// turn. The player brings their own vitals; Rage that belongs to one fight
/// starts from 0.
pub(super) fn engage(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let fighter = rules::character_here(world, state, &id).and_then(|c| c.combat.as_ref());
    let (Some(_), Some(_), Some(combat)) = (fighter, world.combat(), state.combat.as_ref()) else {
        return Err(EngineError::NotHere(id));
    };
    if combat.defeated.contains(&id) {
        return Err(EngineError::AlreadyDefeated(id));
    }
    let Stance::Exploring = combat.stance else {
        return Err(EngineError::InEncounter);
    };
    let timeline = timeline(world)?;
    // A named companion fights beside the player, never against them.
    if world.character(&id).is_some_and(|c| c.companion.is_some()) {
        return Err(EngineError::NotHostile(id));
    }
    let player = character::player_stats(world, combat);
    let opponents: Vec<Id> = opponents_for(world, state, &id);
    let combat = state.combat.as_mut().unwrap();
    // Everyone's first action comes after one opening delay, so speed matters at once.
    let joiner = |character: Id, side, control, vitals, speed| -> Result<_, EngineError> {
        Ok(Participant {
            character,
            side,
            control,
            vitals,
            mp_remainder: 0,
            next_time: delay(timeline, speed, 100)?,
            yielded: false,
        })
    };
    let mut participants = vec![joiner(
        world.world.player.clone(),
        0,
        Control::Player,
        None,
        player.speed,
    )?];
    for character in &opponents {
        let profile = profile(world, character);
        let pools = resources::profile_pools(world, profile);
        participants.push(joiner(
            character.clone(),
            1,
            Control::Policy,
            Some(resources::full(&profile.stats, pools)),
            profile.stats.speed,
        )?);
    }
    resources::after_fight(world, &mut combat.vitals);
    combat.stance = Stance::Fighting(Encounter {
        now: 0,
        participants,
    });
    state.dialogue = None;
    events.push(Event::EncounterStarted { opponents });
    advance(world, state, events, false)
}

/// Who engaging `id` brings in: `id` alone, or every member of its group
/// placed here, present under its conditions and not defeated, in the
/// location's authored order.
pub(super) fn opponents_for(world: &WorldSpec, state: &GameState, id: &str) -> Vec<Id> {
    let Some(group) = world
        .character(id)
        .and_then(|c| c.combat.as_ref()?.group.as_ref())
    else {
        return vec![id.into()];
    };
    let defeated = |c: &str| {
        state
            .combat
            .as_ref()
            .is_some_and(|s| s.defeated.contains(c))
    };
    world
        .location(&state.player.location)
        .unwrap()
        .characters
        .iter()
        .filter(|c| !defeated(c))
        .filter(|c| {
            rules::character_here(world, state, c)
                .and_then(|m| m.combat.as_ref())
                .is_some_and(|m| m.group.as_ref() == Some(group))
        })
        .cloned()
        .collect()
}

/// A participant yields at or below `max(1, ⌊max HP × share / 100⌋)`.
pub(super) fn yield_threshold(max_hp: u32, share: u32) -> u32 {
    (u64::from(max_hp) * u64::from(share) / 100).max(1) as u32
}

/// The group an encounter's opponents share, if any.
pub(super) fn group<'w>(world: &'w WorldSpec, encounter: &Encounter) -> Option<&'w Group> {
    let first = encounter.participants.get(1)?;
    world.group(profile(world, &first.character).group.as_deref()?)
}

/// Declares a flight: the player's turn is spent, opponents act until the
/// player's next turn, and the escape happens then if the player lives.
pub(super) fn flee(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (board, me) = fighting(world, &mut state.combat)?;
    if group(world, board.encounter).is_some_and(|g| g.no_flee) {
        return Err(EngineError::NoFlee);
    }
    let speed = me.stats.speed;
    let step = delay(timeline(world)?, speed, 100)?;
    let player = &mut board.encounter.participants[0];
    player.next_time = player
        .next_time
        .checked_add(step)
        .ok_or(EngineError::NumericLimit)?;
    events.push(Event::FleeStarted);
    advance(world, state, events, true)
}

/// Spends the player's turn on a consumable: `hp`/`mp` (already capped) are
/// restored now, the turn takes one basic action's time, and opponents act
/// until the player's next turn. No rage or technique XP is earned.
pub(super) fn consume_turn(
    world: &WorldSpec,
    state: &mut GameState,
    hp: u32,
    mp: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (board, me) = fighting(world, &mut state.combat)?;
    let step = delay(timeline(world)?, me.stats.speed, 100)?;
    board.player.hp += hp;
    board.player.mp += mp;
    let player = &mut board.encounter.participants[0];
    player.next_time = player
        .next_time
        .checked_add(step)
        .ok_or(EngineError::NumericLimit)?;
    advance(world, state, events, false)
}

fn fighting<'s>(
    world: &WorldSpec,
    combat: &'s mut Option<CombatState>,
) -> Result<(Board<'s>, Me), EngineError> {
    let combat = combat.as_mut().ok_or(EngineError::NotFighting)?;
    let stats = character::player_stats(world, combat);
    let me = Me {
        level: combat.level,
        stats,
        pools: resources::player_pools(world, &stats),
        basic: gear::basic(world, combat),
        guard: [
            gear::modifier(world, combat, Channel::Physical),
            gear::modifier(world, combat, Channel::Special),
        ],
    };
    match &mut combat.stance {
        Stance::Fighting(encounter) => Ok((
            Board {
                encounter,
                player: &mut combat.vitals,
            },
            me,
        )),
        Stance::Exploring | Stance::Realtime(_) | Stance::Battle(_) => {
            Err(EngineError::NotFighting)
        }
    }
}

/// The player's attack or skill on `target`, then opponents act until the
/// player's next turn or the end.
pub(super) fn player_action(
    world: &WorldSpec,
    state: &mut GameState,
    target: Id,
    skill: Option<&Skill>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let turn = state.turn;
    let (mut board, me) = fighting(world, &mut state.combat)?;
    let index = (0..board.encounter.participants.len())
        .find(|&i| {
            let p = &board.encounter.participants[i];
            p.character == target && p.side != 0 && board.fighting(i)
        })
        .ok_or_else(|| EngineError::NotHere(target.clone()))?;
    // Advancing stops at the player's turn, already regenerated, so the
    // player acts now with the MP they see.
    if let Some(skill) = skill {
        check_skill(board.player, me.pools, me.level, skill)?;
    }
    act(
        world,
        &mut board,
        me,
        turn,
        &mut state.rng,
        0,
        index,
        skill,
        events,
    )?;
    // Using a technique trains it, less so against much weaker opponents.
    let combat = state.combat.as_ref().unwrap();
    if let Some(technique) = skill.and_then(|s| techniques::of_skill(world, combat, &s.id)) {
        let per_use = u64::from(world.combat().unwrap().technique_xp_per_use);
        let gain = xp_for_defeat(per_use, me.level, profile(world, &target).level);
        techniques::add_xp(world, state, technique, gain, events)?;
    }
    advance(world, state, events, false)
}

/// Resolves policy actions until the player's turn, the player's death or
/// yield, or victory. A declared flight escapes at the player's turn.
fn advance(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
    fleeing: bool,
) -> Result<(), EngineError> {
    let turn = state.turn;
    loop {
        let (mut board, me) = fighting(world, &mut state.combat)?;
        if board.player.hp == 0 {
            return Ok(());
        }
        if board.encounter.participants[0].yielded {
            return end(world, state, Outcome::Yielded, events);
        }
        if !(1..board.encounter.participants.len()).any(|i| board.fighting(i)) {
            return end(world, state, Outcome::Victory, events);
        }
        let actor = next_actor(board.encounter, board.player).unwrap();
        // Everyone regenerates up to the next turn before it is taken; the
        // player's turn then waits for a command.
        tick(world, &mut board, me, actor)?;
        if board.encounter.participants[actor].control == Control::Player {
            return if fleeing {
                end(world, state, Outcome::Fled, events)
            } else {
                Ok(())
            };
        }
        let side = board.encounter.participants[actor].side;
        let target = (0..board.encounter.participants.len())
            .find(|&i| board.encounter.participants[i].side != side && board.fighting(i))
            .unwrap();
        let character = &board.encounter.participants[actor].character;
        let skill = choose(world, board.vitals(actor), character);
        act(
            world,
            &mut board,
            me,
            turn,
            &mut state.rng,
            actor,
            target,
            skill,
            events,
        )?;
    }
}

/// Ends the encounter and settles its outcome, once.
fn end(
    world: &WorldSpec,
    state: &mut GameState,
    outcome: Outcome,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let combat = state.combat.as_ref().unwrap();
    let Stance::Fighting(encounter) = &combat.stance else {
        unreachable!("only a running encounter ends");
    };
    let group = group(world, encounter);
    let fallen: Vec<Id> = encounter.participants[1..]
        .iter()
        .filter(|p| p.vitals.is_some_and(|v| v.hp == 0))
        .map(|p| p.character.clone())
        .collect();
    settle::settle(world, state, outcome, group, fallen, events)
}
