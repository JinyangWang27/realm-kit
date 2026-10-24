//! Using a consumable: it restores vitals now, and in an encounter it takes
//! the player's turn.

use super::*;
use realmkit_spec::Consumable;

/// What a consumable restores to the player's `now`, capped at the maxima:
/// none of a resource the player does not use.
pub(crate) fn gain(
    world: &WorldSpec,
    combat: &CombatState,
    now: Vitals,
    c: Consumable,
) -> (u32, u32) {
    let max = player_stats(world, combat);
    let mp_max = crate::character::resources::player_pools(world, &max).mp_max();
    (
        c.hp.min(max.hp.saturating_sub(now.hp)),
        c.mp.min(mp_max.saturating_sub(now.mp)),
    )
}

/// The consumable `item`, if it is one.
pub(crate) fn consumable(world: &WorldSpec, item: &str) -> Result<Consumable, EngineError> {
    world
        .item(item)
        .and_then(|i| i.consumable)
        .ok_or_else(|| EngineError::NotConsumable(item.into()))
}

pub(crate) fn consume(
    world: &WorldSpec,
    state: &mut GameState,
    item: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let restores = consumable(world, &item)?;
    // Having none is the answer even when there is also nothing to restore.
    if !state.player.inventory.contains_key(&item) {
        return Err(EngineError::NotEnoughMaterials(item));
    }
    // Validation keeps consumables to worlds with combat.
    let now = player_vitals(state).unwrap();
    let (hp, mp) = gain(world, state.combat.as_ref().unwrap(), now, restores);
    if hp == 0 && mp == 0 {
        return Err(EngineError::NothingToRestore);
    }
    let stack = [realmkit_spec::ItemStack {
        item: item.clone(),
        quantity: 1,
    }];
    story::take_items(state, &stack, &mut Vec::new())?;
    events.push(Event::Consumed { item, hp, mp });
    let combat = state.combat.as_mut().unwrap();
    match &combat.stance {
        Stance::Exploring => {
            // Within the maxima, which validation bounds far below u32::MAX.
            combat.vitals.hp += hp;
            combat.vitals.mp += mp;
            state.dialogue = None;
            Ok(())
        }
        Stance::Fighting(_) => encounter::consume_turn(world, state, hp, mp, events),
        // The battle gate refuses items before they get here.
        Stance::Battle(_) => Err(EngineError::InEncounter),
    }
}
