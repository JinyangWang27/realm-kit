//! Command dispatch and the queries every rule shares.

use super::*;

mod actions;
mod consume;
mod crafting;
mod economy;
mod map;
mod player;
mod proficiency;
mod retinue;
mod story;
mod time;

pub(super) use actions::actions;
pub(super) use economy::{quote, stock_up, ware_price};
pub(super) use map::{known, map_view};
pub(super) use player::{clamp_vitals, granted_points, player_stats, unspent_points};
pub(super) use proficiency::{
    granted_points as granted_proficiency_points, rank, unspent_proficiency_points,
};
pub(super) use retinue::{leave, promote, prune};
pub(super) use story::{
    apply as apply_effects, choices, grant_items, grant_xp, journal, progress, set_flag,
};

/// Evaluates a condition against the state; pure, so it may run any number of times.
pub(super) fn holds(state: &GameState, condition: &Condition) -> bool {
    match condition {
        Condition::All { of } => of.iter().all(|c| holds(state, c)),
        Condition::Any { of } => of.iter().any(|c| holds(state, c)),
        Condition::Not { condition } => !holds(state, condition),
        Condition::Flag { flag } => state.flags.contains(flag),
        Condition::Quest { quest, status } => state.quests.get(quest) == Some(status),
        Condition::Technique { technique, rank } => state
            .combat
            .as_ref()
            .and_then(|c| c.techniques.get(technique))
            .is_some_and(|learned| learned.rank >= *rank),
        Condition::Item { item, quantity } => state
            .player
            .inventory
            .get(item)
            .is_some_and(|n| n >= quantity),
        // Validation keeps time conditions to worlds with a clock.
        Condition::Currency { amount } => state
            .economy
            .as_ref()
            .is_some_and(|e| e.currency >= *amount),
        Condition::Workshop { workshop, location } => state.economy.as_ref().is_some_and(|e| {
            e.workshops
                .iter()
                .filter(|(at, _)| location.as_ref().is_none_or(|l| l == *at))
                .any(|(_, kinds)| kinds.get(workshop).is_some_and(|n| *n > 0))
        }),
        Condition::Proficiency {
            proficiency,
            rank: at_least,
        } => rank(state, *proficiency) >= *at_least,
        Condition::Evidence { evidence } => state.evidence.contains(evidence),
        Condition::Phase { phase } => state.phases.contains(phase),
        Condition::TimeOfDay { from, to } => state.time.is_some_and(|now| {
            let minute = now % realmkit_spec::MINUTES_PER_DAY;
            if from < to {
                (*from..*to).contains(&minute)
            } else {
                minute >= *from || minute < *to
            }
        }),
    }
}

/// An optional requirement: absent means always.
pub(super) fn allowed(state: &GameState, requires: Option<&Condition>) -> bool {
    requires.is_none_or(|c| holds(state, c))
}

pub(super) fn player_vitals(state: &GameState) -> Option<Vitals> {
    Some(match &state.combat.as_ref()?.stance {
        Stance::Exploring(vitals) => *vitals,
        Stance::Fighting(encounter) => {
            let player = &encounter.participants[0];
            Vitals {
                hp: player.hp,
                mp: player.mp,
            }
        }
        Stance::Battle(battle) => Vitals {
            hp: battle.hp,
            mp: battle.mp,
        },
    })
}

/// Only a personal fight kills: a battle knocks the player out instead.
pub(super) fn dead(state: &GameState) -> bool {
    let battle = state
        .combat
        .as_ref()
        .is_some_and(|c| matches!(c.stance, Stance::Battle(_)));
    !battle && player_vitals(state).is_some_and(|v| v.hp == 0)
}

pub(super) fn defeated(state: &GameState, id: &str) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|c| c.defeated.contains(id))
}

fn fighting(state: &GameState) -> Option<&Encounter> {
    match &state.combat.as_ref()?.stance {
        Stance::Fighting(encounter) => Some(encounter),
        Stance::Exploring(_) | Stance::Battle(_) => None,
    }
}

fn leading(state: &GameState) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|c| matches!(c.stance, Stance::Battle(_)))
}

/// Characters at the player's location, whatever their conditions: those
/// placed here in the location's order, then movers who are here now.
pub(super) fn placed_here<'a>(world: &'a WorldSpec, state: &GameState) -> Vec<&'a Id> {
    let here = &state.player.location;
    let placed = world
        .location(here)
        .unwrap()
        .characters
        .iter()
        .filter(|id| !state.whereabouts.contains_key(*id));
    let movers = world
        .characters
        .iter()
        .filter(|c| state.whereabouts.get(&c.id) == Some(here))
        .map(|c| &c.id);
    placed.chain(movers).collect()
}

/// Who is at the player's location now: placed or moved here, present
/// under their conditions and not defeated, in [`placed_here`] order.
pub(super) fn present_here<'a>(world: &'a WorldSpec, state: &GameState) -> Vec<&'a Character> {
    placed_here(world, state)
        .into_iter()
        .filter(|id| !defeated(state, id))
        .filter_map(|id| character_here(world, state, id))
        .collect()
}

/// At the player's location and present under its conditions.
pub(super) fn character_here<'a>(
    world: &'a WorldSpec,
    state: &GameState,
    id: &str,
) -> Option<&'a Character> {
    let here = match state.whereabouts.get(id) {
        Some(at) => *at == state.player.location,
        None => world
            .location(&state.player.location)
            .unwrap()
            .characters
            .iter()
            .any(|c| c == id),
    };
    world
        .character(id)
        .filter(|c| here && allowed(state, c.requires.as_ref()))
}

/// Can be talked to here; a defeated fighter is gone, like its listing.
pub(super) fn npc_here(world: &WorldSpec, state: &GameState, id: &str) -> bool {
    !defeated(state, id) && character_here(world, state, id).is_some_and(|c| c.dialogue.is_some())
}

/// Inspection commands: they show state and spend no time.
pub(super) fn is_panel(command: &Command) -> bool {
    matches!(
        command,
        Command::Look
            | Command::Status
            | Command::Inventory
            | Command::Quests
            | Command::Techniques
            | Command::Market
            | Command::Retinue
            | Command::Map
    )
}

pub(super) fn execute(
    world: &WorldSpec,
    state: &mut GameState,
    command: Command,
) -> Result<Vec<Event>, EngineError> {
    let panel = is_panel(&command);
    if dead(state) && !panel {
        return Err(EngineError::PlayerDead);
    }
    let combat_action = matches!(
        command,
        Command::Attack(_) | Command::UseSkill { .. } | Command::Flee | Command::Use(_)
    );
    // During an encounter only combat actions and panels are possible.
    if fighting(state).is_some() && !panel && !combat_action {
        return Err(EngineError::InEncounter);
    }
    // In a battle only orders and panels are possible.
    let order = matches!(command, Command::Order(_) | Command::Autoresolve);
    if leading(state) && !panel && !order {
        return Err(EngineError::InEncounter);
    }
    let mut events = Vec::new();
    match command {
        Command::Look => events.push(Event::LocationViewed {
            location: state.player.location.clone(),
        }),
        Command::Inventory => events.push(Event::InventoryViewed),
        Command::Status => events.push(Event::StatusViewed),
        Command::Quests => events.push(Event::QuestsViewed),
        Command::Techniques => events.push(Event::TechniquesViewed),
        Command::Map => {
            if map_view(world, state).is_none() {
                return Err(EngineError::NoMap);
            }
            events.push(Event::MapViewed)
        }
        Command::Retinue => {
            if state.retinue.is_none() {
                return Err(EngineError::NoRetinue);
            }
            events.push(Event::RetinueViewed)
        }
        Command::Recruit { line, quantity } => {
            retinue::recruit(world, state, line, quantity, &mut events)?
        }
        Command::Upgrade { line, to, quantity } => {
            retinue::upgrade(world, state, line, to, quantity, &mut events)?
        }
        Command::Market => {
            economy::market_here(world, state)?;
            events.push(Event::MarketViewed)
        }
        Command::Buy { good, quantity } => {
            economy::trade(world, state, &good, quantity, true, &mut events)?
        }
        Command::Sell { good, quantity } => {
            economy::trade(world, state, &good, quantity, false, &mut events)?
        }
        Command::Move(direction) => move_to(world, state, direction, &mut events)?,
        Command::Travel(to) => time::travel(world, state, to, &mut events)?,
        Command::Wait(minutes) => time::wait(world, state, minutes, &mut events)?,
        Command::Talk(id) => story::talk(world, state, id, &mut events)?,
        Command::ChooseDialogue(number) => story::choose(world, state, number, &mut events)?,
        Command::AcceptQuest(id) => {
            story::quest(world, state, &id, false, &mut events)?;
            state.dialogue = None;
        }
        Command::CompleteQuest(id) => {
            story::quest(world, state, &id, true, &mut events)?;
            state.dialogue = None;
        }
        Command::Engage(id) if world.character(&id).is_some_and(|c| c.army.is_some()) => {
            battle::engage(world, state, id, &mut events)?
        }
        Command::Engage(id) => encounter::engage(world, state, id, &mut events)?,
        Command::Order(order) => battle::command(world, state, order, &mut events)?,
        Command::Autoresolve => battle::autoresolve(world, state, &mut events)?,
        Command::Flee => encounter::flee(world, state, &mut events)?,
        Command::Attack(id) => encounter::player_action(world, state, id, None, &mut events)?,
        Command::UseSkill { skill, target } => use_skill(world, state, skill, target, &mut events)?,
        Command::Allocate { stat, points } => {
            player::allocate(world, state, stat, points, &mut events)?
        }
        Command::Respec => player::respec(world, state, &mut events)?,
        Command::Train {
            proficiency,
            points,
        } => proficiency::train(world, state, proficiency, points, &mut events)?,
        Command::Equip(piece) => gear::equip(world, state, piece, &mut events)?,
        Command::Unequip(piece) => gear::unequip(world, state, piece, &mut events)?,
        Command::Forge(recipe) => crafting::forge(world, state, &recipe, &mut events)?,
        Command::Improve(piece) => crafting::improve(world, state, piece, &mut events)?,
        Command::Enchant { piece, enchantment } => {
            crafting::enchant(world, state, piece, &enchantment, &mut events)?
        }
        Command::Rest => player::rest(world, state, &mut events)?,
        Command::Use(item) => consume::consume(world, state, item, &mut events)?,
    }
    // A flag or quest this command changed may open a breakthrough gate.
    if !panel {
        techniques::promote(world, state, &mut events);
        // Any change, such as a trade that empties a choice's condition, can
        // leave the conversation with no speaker or nothing to say: it ends.
        if let Some(open) = &state.dialogue {
            if !npc_here(world, state, &open.npc)
                || choices(world, state, &open.npc, &open.node).is_empty()
            {
                state.dialogue = None;
                events.push(Event::DialogueEnded);
            }
        }
        // Validation proves outcomes exclude each other, so at most one holds.
        if state.outcome.is_none() {
            if let Some(reached) = outcomes(world, state).pop() {
                state.outcome = Some(reached.clone());
                events.push(Event::OutcomeReached { outcome: reached });
            }
        }
    }
    Ok(events)
}

/// The authored outcomes whose conditions hold now, in authored order.
pub(super) fn outcomes(world: &WorldSpec, state: &GameState) -> Vec<Id> {
    world
        .world
        .outcomes
        .iter()
        .filter(|o| holds(state, &o.when))
        .map(|o| o.id.clone())
        .collect()
}

fn move_to(
    world: &WorldSpec,
    state: &mut GameState,
    direction: Direction,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let location = world.location(&state.player.location).unwrap();
    let exit = location
        .exits
        .get(&direction)
        .filter(|exit| known(world, state, &exit.destination))
        .ok_or(EngineError::NoExit)?;
    if !allowed(state, exit.requires.as_ref()) {
        return Err(EngineError::ExitLocked {
            location: location.id.clone(),
            direction,
        });
    }
    let from = std::mem::replace(&mut state.player.location, exit.destination.clone());
    state.dialogue = None;
    events.push(Event::Moved {
        from,
        to: exit.destination.clone(),
    });
    events.push(Event::LocationViewed {
        location: exit.destination.clone(),
    });
    Ok(())
}

/// A skill the player knows (an authored player skill or a learned
/// technique's current rank), used on `target`.
fn use_skill(
    world: &WorldSpec,
    state: &mut GameState,
    skill: Id,
    target: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let known = state.combat.as_ref().is_some_and(|c| {
        techniques::player_skills(world, c)
            .iter()
            .any(|s| s.id == skill)
    });
    let skill = world
        .skill(&skill)
        .filter(|_| known)
        .ok_or(EngineError::UnknownSkill(skill))?;
    encounter::player_action(world, state, target, Some(skill), events)
}
