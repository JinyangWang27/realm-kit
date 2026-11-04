//! Fights a host runs itself. The engine authorizes the fight and suspends the
//! world while it lasts; the host owns every blow and clock inside it. When it
//! ends, the host hands back one settlement: the engine checks it against what
//! the world allows and commits it through the same settlement every other
//! personal fight ends in, once. A fight's live vitals are the host's while it
//! runs, so the state here keeps the values from before it, for recovery only.

use super::*;
use crate::character::{profile, resources};

/// Starts a host-run fight with `id` and whoever engaging it would bring in,
/// under the same conditions as any other personal fight.
pub(crate) fn begin(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let fighter = rules::character_here(world, state, &id).and_then(|c| c.combat.as_ref());
    let (Some(fighter), Some(combat)) = (fighter, state.combat.as_ref()) else {
        return Err(EngineError::NotHere(id));
    };
    if combat.defeated.contains(&id) {
        return Err(EngineError::AlreadyDefeated(id));
    }
    let Stance::Exploring = combat.stance else {
        return Err(EngineError::InEncounter);
    };
    if fighter.ally {
        return Err(EngineError::NotHostile(id));
    }
    let opponents = encounter::opponents_for(world, state, &id);
    let fight = state.turn;
    let combat = state.combat.as_mut().unwrap();
    resources::after_fight(world, &mut combat.vitals);
    combat.stance = Stance::Hosted(HostedFight {
        id: fight,
        opponents: opponents.clone(),
    });
    state.dialogue = None;
    events.push(Event::EncounterStarted { opponents });
    Ok(())
}

/// Ends the host-run fight with the host's account of it, or changes nothing.
/// The stance is gone before anything is granted, so a settlement repeated
/// afterwards finds no fight and is refused.
pub(crate) fn settle(
    world: &WorldSpec,
    state: &mut GameState,
    settlement: FightSettlement,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let invalid = |why: &str| EngineError::InvalidSettlement(why.into());
    let Some(Stance::Hosted(fight)) = state.combat.as_ref().map(|c| &c.stance) else {
        return Err(EngineError::NotFighting);
    };
    if fight.id != settlement.fight {
        return Err(invalid("it is for another fight"));
    }
    let group = fight
        .opponents
        .first()
        .and_then(|id| world.group(profile(world, id).group.as_deref()?));
    if settlement.outcome == Outcome::Fled && group.is_some_and(|g| g.no_flee) {
        return Err(EngineError::NoFlee);
    }
    check_fallen(world, state, &settlement.fallen).map_err(invalid)?;
    check_companions(world, state, &settlement.companions).map_err(invalid)?;

    let rules = world.combat().unwrap();
    let combat = state.combat.as_mut().unwrap();
    if gear::together(world, combat, &settlement.worn).is_none() {
        return Err(invalid("those pieces cannot be worn together"));
    }
    for (id, piece) in &mut combat.gear {
        piece.equipped = settlement.worn.contains(id);
    }
    let max = character::player_stats(world, combat);
    let pools = resources::player_pools(world, &max);
    if settlement.vitals.hp == 0 || !save::within(rules, &settlement.vitals, &max, pools) {
        return Err(invalid(
            "the player's vitals are not ones a fight can leave",
        ));
    }
    combat.vitals = settlement.vitals;
    for (id, companion) in settlement.companions {
        let record = state.companions.get_mut(&id).unwrap();
        record.order = companion.order;
        if let Some(vitals) = companion.vitals {
            record.vitals = vitals;
            resources::after_fight(world, &mut record.vitals);
        }
    }
    for companion in state.companions.values_mut() {
        if let TacticalOrder::Attack { .. } = companion.order {
            companion.order = TacticalOrder::Follow;
        }
    }
    settle::settle(
        world,
        state,
        settlement.outcome,
        group,
        settlement.fallen,
        events,
    )
}

/// Those who fell are fighters present here, against the player, each once.
fn check_fallen(world: &WorldSpec, state: &GameState, fallen: &[Id]) -> Result<(), &'static str> {
    let defeated = &state.combat.as_ref().unwrap().defeated;
    let mut seen = BTreeSet::new();
    let ok = fallen.iter().all(|id| {
        rules::character_here(world, state, id)
            .and_then(|c| c.combat.as_ref())
            .is_some_and(|c| !c.ally)
            && !defeated.contains(id)
            && seen.insert(id)
    });
    ok.then_some(())
        .ok_or("someone who fell could not have fought")
}

/// Each companion travels in the party, with vitals within its maxima and an
/// order it could have been given. An order to attack names someone in the
/// host's fight, which the engine cannot see, and lapses at the fight's end.
fn check_companions(
    world: &WorldSpec,
    state: &GameState,
    companions: &BTreeMap<Id, SettledCompanion>,
) -> Result<(), &'static str> {
    let rules = world.combat().unwrap();
    let ok = companions.iter().all(|(id, companion)| {
        let ward = match &companion.order {
            TacticalOrder::Defend { ally } => {
                *ally == world.world.player || (ally != id && rules::in_party(state, ally))
            }
            _ => true,
        };
        let vitals = companion.vitals.is_none_or(|v| {
            let profile = profile(world, id);
            let pools = resources::profile_pools(world, profile);
            save::within(rules, &v, &profile.stats, pools)
        });
        rules::in_party(state, id) && ward && vitals
    });
    ok.then_some(())
        .ok_or("a companion's account does not hold")
}
