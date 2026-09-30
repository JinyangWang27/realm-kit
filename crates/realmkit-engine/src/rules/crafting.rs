//! Forging new pieces and improving owned ones, at stations. Every check runs
//! before anything changes, so a refusal spends nothing.

use super::*;
use realmkit_spec::{Recipe, TechniqueGrant, Tier};

/// A recipe the player knows: hidden recipes do not exist for them.
fn known<'w>(world: &'w WorldSpec, state: &GameState, id: &str) -> Option<&'w Recipe> {
    world
        .recipe(id)
        .filter(|r| conditions_met(state, &r.known_when))
}

/// The next tier of an owned piece, if it has one.
fn next_tier<'w>(world: &'w WorldSpec, gear: &Gear) -> Option<&'w Tier> {
    let equipment = world.item(&gear.item)?.equipment.as_ref()?;
    equipment.tiers.get(gear.tier)
}

/// The station, then the requirements, then the materials: the order a
/// player would put them right.
fn ready(
    world: &WorldSpec,
    state: &GameState,
    station: &Id,
    requires: &[Condition],
    materials: &[ItemStack],
) -> Result<(), EngineError> {
    let here = world.location(&state.player.location).unwrap();
    if !here.stations.contains(station) {
        return Err(EngineError::NoStation(station.clone()));
    }
    if !conditions_met(state, requires) {
        return Err(EngineError::RequirementsUnmet);
    }
    for stack in materials {
        let held = state
            .player
            .inventory
            .get(&stack.item)
            .copied()
            .unwrap_or(0);
        if held < stack.quantity {
            return Err(EngineError::NotEnoughMaterials(stack.item.clone()));
        }
    }
    Ok(())
}

fn spend(state: &mut GameState, materials: &[ItemStack], events: &mut Vec<Event>) {
    for stack in materials {
        let held = state.player.inventory.get_mut(&stack.item).unwrap();
        *held -= stack.quantity;
        if *held == 0 {
            state.player.inventory.remove(&stack.item);
        }
        events.push(Event::ItemsSpent {
            item: stack.item.clone(),
            quantity: stack.quantity,
        });
    }
}

/// Crafting's technique XP, which teaches the technique if it is unknown.
fn train(
    world: &WorldSpec,
    state: &mut GameState,
    grant: Option<&TechniqueGrant>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    match grant {
        Some(grant) => techniques::grant(world, state, grant, events),
        None => Ok(()),
    }
}

pub(crate) fn forge(
    world: &WorldSpec,
    state: &mut GameState,
    id: &Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let recipe = known(world, state, id).ok_or_else(|| EngineError::UnknownRecipe(id.clone()))?;
    ready(
        world,
        state,
        &recipe.station,
        &recipe.requires,
        &recipe.inputs,
    )?;
    spend(state, &recipe.inputs, events);
    let combat = state.combat.as_mut().unwrap();
    let gear = combat.next_gear;
    gear::receive(combat, &recipe.output, 1)?;
    events.push(Event::Forged {
        recipe: id.clone(),
        gear,
    });
    train(world, state, recipe.trains.as_ref(), events)
}

/// The piece rises one tier; the new tier's bonuses replace the old ones.
pub(crate) fn improve(
    world: &WorldSpec,
    state: &mut GameState,
    piece: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let gear = state
        .combat
        .as_ref()
        .and_then(|c| c.gear.get(&piece))
        .ok_or(EngineError::NoSuchGear(piece))?;
    let tier = next_tier(world, gear).ok_or(EngineError::NoHigherTier(piece))?;
    ready(world, state, &tier.station, &tier.requires, &tier.cost)?;
    spend(state, &tier.cost, events);
    let gear = state.combat.as_mut().unwrap().gear.get_mut(&piece).unwrap();
    gear.tier += 1;
    events.push(Event::Improved {
        gear: piece,
        tier: gear.tier,
    });
    // A lighter or weaker tier changes the maxima of a worn piece.
    clamp_vitals(world, state);
    train(world, state, tier.trains.as_ref(), events)
}

/// Known recipes and improvable pieces at this location's stations. Those
/// short of ability or materials stay listed, so the player sees why.
pub(crate) fn offered(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let here = world.location(&state.player.location).unwrap();
    let Some(rules) = world.combat() else {
        return Vec::new();
    };
    let forge = rules
        .recipes
        .iter()
        .filter(|r| here.stations.contains(&r.station) && conditions_met(state, &r.known_when))
        .map(|r| Action {
            command: Command::Forge(r.id.clone()),
            available: ready(world, state, &r.station, &r.requires, &r.inputs).is_ok(),
        });
    let pieces = state.combat.iter().flat_map(|c| &c.gear);
    let improve = pieces.filter_map(|(id, gear)| {
        let tier = next_tier(world, gear).filter(|t| here.stations.contains(&t.station))?;
        Some(Action {
            command: Command::Improve(*id),
            available: ready(world, state, &tier.station, &tier.requires, &tier.cost).is_ok(),
        })
    });
    forge.chain(improve).collect()
}
