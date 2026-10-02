//! Using a consumable: it restores vitals now, and in an encounter it takes
//! the player's turn.

use super::*;
use realmkit_spec::Consumable;

/// What a consumable restores to vitals `hp`/`mp`, capped at `max`.
pub(crate) fn gain(hp: u32, mp: u32, max: Stats, c: Consumable) -> (u32, u32) {
    (
        c.hp.min(max.hp.saturating_sub(hp)),
        c.mp.min(max.mp.saturating_sub(mp)),
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
    let max = player_stats(world, state.combat.as_ref().unwrap());
    let now = player_vitals(state).unwrap();
    let (hp, mp) = gain(now.hp, now.mp, max, restores);
    if hp == 0 && mp == 0 {
        return Err(EngineError::NothingToRestore);
    }
    let stack = [realmkit_spec::ItemStack {
        item: item.clone(),
        quantity: 1,
    }];
    story::take_items(state, &stack, &mut Vec::new())?;
    events.push(Event::Consumed { item, hp, mp });
    match &mut state.combat.as_mut().unwrap().stance {
        Stance::Exploring(vitals) => {
            // Within the maxima, which validation bounds far below u32::MAX.
            vitals.hp += hp;
            vitals.mp += mp;
            state.dialogue = None;
            Ok(())
        }
        Stance::Fighting(_) => encounter::consume_turn(world, state, hp, mp, events),
    }
}
