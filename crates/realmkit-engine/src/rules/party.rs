//! The player's party of named companions: joining and leaving it by
//! authored effects, standing tactical orders, and what resting does for
//! the companions who travel with the player. Each companion's vitals have
//! one owner, its [`CompanionState`], which fights read and change in place.

use super::*;
use crate::character::{profile, resources};

/// A companion's vitals at full HP and MP with no Rage, as when it is
/// first recruited.
fn whole(world: &WorldSpec, id: &str) -> Vitals {
    let profile = profile(world, id);
    resources::full(&profile.stats, resources::profile_pools(world, profile))
}

fn is_companion(world: &WorldSpec, id: &str) -> bool {
    world.character(id).is_some_and(|c| c.companion.is_some())
}

/// Whether `id` travels in the party now.
pub(crate) fn in_party(state: &GameState, id: &str) -> bool {
    state
        .companions
        .get(id)
        .is_some_and(CompanionState::in_party)
}

/// A companion here joins the party, under its standing order to follow.
/// The first time, it arrives whole; after that it brings the vitals it
/// had when it parted, so leaving and joining again heals nothing.
pub(crate) fn join(
    world: &WorldSpec,
    state: &mut GameState,
    id: &Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if !is_companion(world, id) {
        return Err(EngineError::NotACompanion(id.clone()));
    }
    if in_party(state, id) {
        return Err(EngineError::AlreadyInParty(id.clone()));
    }
    if character_here(world, state, id).is_none() {
        return Err(EngineError::NotHere(id.clone()));
    }
    let record = state
        .companions
        .entry(id.clone())
        .or_insert_with(|| CompanionState {
            parted_at: None,
            vitals: whole(world, id),
            order: TacticalOrder::Follow,
        });
    record.parted_at = None;
    record.order = TacticalOrder::Follow;
    events.push(Event::JoinedParty {
        character: id.clone(),
    });
    Ok(())
}

/// A companion in the party leaves it and stays where the player is now,
/// keeping its vitals. Orders to defend it lapse to following.
pub(crate) fn leave(
    world: &WorldSpec,
    state: &mut GameState,
    id: &Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if !in_party(state, id) {
        return Err(match is_companion(world, id) {
            true => EngineError::NotInParty(id.clone()),
            false => EngineError::NotACompanion(id.clone()),
        });
    }
    let here = state.player.location.clone();
    let record = state.companions.get_mut(id).unwrap();
    record.parted_at = Some(here);
    record.order = TacticalOrder::Follow;
    for other in state.companions.values_mut() {
        if matches!(&other.order, TacticalOrder::Defend { ally } if ally == id) {
            other.order = TacticalOrder::Follow;
        }
    }
    events.push(Event::LeftParty {
        character: id.clone(),
    });
    Ok(())
}

/// Gives a companion in the party a standing order. Every check comes first,
/// so a refused order changes nothing. An order only changes whom the
/// companion's next attacks aim at: it never begins, cancels or lands one.
pub(crate) fn tactic(
    world: &WorldSpec,
    state: &mut GameState,
    companion: Id,
    order: TacticalOrder,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if !in_party(state, &companion) {
        return Err(match is_companion(world, &companion) {
            true => EngineError::NotInParty(companion),
            false => EngineError::NotACompanion(companion),
        });
    }
    match &order {
        TacticalOrder::Follow | TacticalOrder::Hold => {}
        TacticalOrder::Defend { ally } => {
            let ward = *ally == world.world.player || (*ally != companion && in_party(state, ally));
            if !ward {
                return Err(EngineError::InvalidWard(ally.clone()));
            }
        }
        // Only a fight has foes to name; the companion need not be in it yet.
        TacticalOrder::Attack { target } => {
            let fight = match state.combat.as_ref().map(|c| &c.stance) {
                Some(Stance::Realtime(fight)) => fight,
                _ => return Err(EngineError::NotFighting),
            };
            let t = fight
                .index(target)
                .filter(|&t| fight.fighting(t, state))
                .ok_or_else(|| EngineError::InvalidTarget(target.clone()))?;
            if fight.fighters[t].team == 0 {
                return Err(EngineError::NotHostile(target.clone()));
            }
        }
    }
    state.companions.get_mut(&companion).unwrap().order = order.clone();
    events.push(Event::OrderGiven { companion, order });
    Ok(())
}

/// Resting with the player restores each companion in the party to full HP
/// and MP, a downed one included; Rage, as for the player, is untouched.
pub(crate) fn rest(world: &WorldSpec, state: &mut GameState) {
    for (id, companion) in state.companions.iter_mut() {
        if companion.in_party() {
            let full = whole(world, id);
            companion.vitals.hp = full.hp;
            companion.vitals.mp = full.mp;
        }
    }
}
