//! Forging new pieces and improving owned ones, at stations. Every check runs
//! before anything changes, so a refusal spends nothing.

use super::*;
use realmkit_spec::{Recipe, TechniqueGrant, Tier};

/// A recipe the player knows: hidden recipes do not exist for them.
fn known<'w>(world: &'w WorldSpec, state: &GameState, id: &str) -> Option<&'w Recipe> {
    world
        .recipe(id)
        .filter(|r| allowed(state, r.known_when.as_ref()))
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
    requires: Option<&Condition>,
    materials: &[ItemStack],
) -> Result<(), EngineError> {
    let here = world.location(&state.player.location).unwrap();
    if !here.stations.contains(station) {
        return Err(EngineError::NoStation(station.clone()));
    }
    if !allowed(state, requires) {
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
        recipe.requires.as_ref(),
        &recipe.inputs,
    )?;
    story::take_items(state, &recipe.inputs, events)?;
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
    ready(
        world,
        state,
        &tier.station,
        tier.requires.as_ref(),
        &tier.cost,
    )?;
    story::take_items(state, &tier.cost, events)?;
    let gear = state.combat.as_mut().unwrap().gear.get_mut(&piece).unwrap();
    gear.tier += 1;
    events.push(Event::Improved {
        gear: piece,
        tier: gear.tier,
    });
    train(world, state, tier.trains.as_ref(), events)?;
    // Maxima settle only after every change: a weaker tier's lost bonus may
    // come back through the rank its training reaches.
    clamp_vitals(world, state);
    Ok(())
}

/// Lays a known enchantment on an unenchanted piece it fits, for good.
pub(crate) fn enchant(
    world: &WorldSpec,
    state: &mut GameState,
    piece: u64,
    id: &Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let gear = state
        .combat
        .as_ref()
        .and_then(|c| c.gear.get(&piece))
        .ok_or(EngineError::NoSuchGear(piece))?;
    let enchantment = world
        .enchantment(id)
        .filter(|e| allowed(state, e.known_when.as_ref()))
        .ok_or_else(|| EngineError::UnknownEnchantment(id.clone()))?;
    if gear.enchantment.is_some() {
        return Err(EngineError::AlreadyEnchanted(piece));
    }
    let equipment = world.item(&gear.item).unwrap().equipment.as_ref().unwrap();
    if !enchantment.fits(equipment) {
        return Err(EngineError::DoesNotFit {
            gear: piece,
            enchantment: id.clone(),
        });
    }
    ready(
        world,
        state,
        &enchantment.station,
        enchantment.requires.as_ref(),
        &enchantment.catalyst,
    )?;
    story::take_items(state, &enchantment.catalyst, events)?;
    let gear = state.combat.as_mut().unwrap().gear.get_mut(&piece).unwrap();
    gear.enchantment = Some(id.clone());
    events.push(Event::Enchanted {
        gear: piece,
        enchantment: id.clone(),
    });
    train(world, state, enchantment.trains.as_ref(), events)?;
    clamp_vitals(world, state);
    Ok(())
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
        .filter(|r| here.stations.contains(&r.station) && allowed(state, r.known_when.as_ref()))
        .map(|r| Action {
            command: Command::Forge(r.id.clone()),
            available: ready(world, state, &r.station, r.requires.as_ref(), &r.inputs).is_ok(),
        });
    let pieces = state.combat.iter().flat_map(|c| &c.gear);
    let improve = pieces.filter_map(|(id, gear)| {
        let tier = next_tier(world, gear).filter(|t| here.stations.contains(&t.station))?;
        Some(Action {
            command: Command::Improve(*id),
            available: ready(
                world,
                state,
                &tier.station,
                tier.requires.as_ref(),
                &tier.cost,
            )
            .is_ok(),
        })
    });
    // Every known enchantment here, on every unenchanted piece it fits.
    let enchant = rules
        .enchantments
        .iter()
        .filter(|e| here.stations.contains(&e.station) && allowed(state, e.known_when.as_ref()))
        .flat_map(|e| {
            let pieces = state.combat.iter().flat_map(|c| &c.gear);
            pieces.filter_map(move |(id, gear)| {
                let equipment = world.item(&gear.item)?.equipment.as_ref()?;
                (gear.enchantment.is_none() && e.fits(equipment)).then(|| Action {
                    command: Command::Enchant {
                        piece: *id,
                        enchantment: e.id.clone(),
                    },
                    available: ready(world, state, &e.station, e.requires.as_ref(), &e.catalyst)
                        .is_ok(),
                })
            })
        });
    forge.chain(improve).chain(enchant).collect()
}
