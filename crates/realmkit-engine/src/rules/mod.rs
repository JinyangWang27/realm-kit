//! Command dispatch and the queries every rule shares.

use super::*;

mod actions;
mod player;
mod story;

pub(super) use actions::actions;
pub(super) use player::{clamp_vitals, granted_points, player_stats, unspent_points};
pub(super) use story::{choices, grant_items, grant_xp, progress, set_flag};

pub(super) fn conditions_met(state: &GameState, conditions: &[Condition]) -> bool {
    conditions.iter().all(|condition| match condition {
        Condition::Flag { flag } => state.flags.contains(flag),
        Condition::Quest { quest, status } => state.quests.get(quest) == Some(status),
        Condition::Technique { technique, rank } => state
            .combat
            .as_ref()
            .and_then(|c| c.techniques.get(technique))
            .is_some_and(|learned| learned.rank >= *rank),
    })
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
    })
}

pub(super) fn dead(state: &GameState) -> bool {
    player_vitals(state).is_some_and(|v| v.hp == 0)
}

fn defeated(state: &GameState, id: &str) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|c| c.defeated.contains(id))
}

fn fighting(state: &GameState) -> Option<&Encounter> {
    match &state.combat.as_ref()?.stance {
        Stance::Fighting(encounter) => Some(encounter),
        Stance::Exploring(_) => None,
    }
}

/// Placed at the player's location and present under its conditions.
pub(super) fn character_here<'a>(
    world: &'a WorldSpec,
    state: &GameState,
    id: &str,
) -> Option<&'a Character> {
    let placed = world
        .location(&state.player.location)
        .unwrap()
        .characters
        .iter()
        .any(|c| c == id);
    world
        .character(id)
        .filter(|c| placed && conditions_met(state, &c.requires))
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
        Command::Attack(_) | Command::UseSkill { .. } | Command::Flee
    );
    // During an encounter only combat actions and panels are possible.
    if fighting(state).is_some() && !panel && !combat_action {
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
        Command::Move(direction) => move_to(world, state, direction, &mut events)?,
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
        Command::Engage(id) => encounter::engage(world, state, id, &mut events)?,
        Command::Flee => encounter::flee(world, state, &mut events)?,
        Command::Attack(id) => encounter::player_action(world, state, id, None, &mut events)?,
        Command::UseSkill { skill, target } => use_skill(world, state, skill, target, &mut events)?,
        Command::Allocate { stat, points } => {
            player::allocate(world, state, stat, points, &mut events)?
        }
        Command::Respec => player::respec(world, state, &mut events)?,
        Command::Equip(piece) => gear::equip(world, state, piece, &mut events)?,
        Command::Unequip(piece) => gear::unequip(world, state, piece, &mut events)?,
        Command::Rest => player::rest(world, state, &mut events)?,
    }
    // A flag or quest this command changed may open a breakthrough gate.
    if !panel {
        techniques::promote(world, state, &mut events);
    }
    Ok(events)
}

fn move_to(
    world: &WorldSpec,
    state: &mut GameState,
    direction: Direction,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let location = world.location(&state.player.location).unwrap();
    let exit = location.exits.get(&direction).ok_or(EngineError::NoExit)?;
    if !conditions_met(state, &exit.requires) {
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
