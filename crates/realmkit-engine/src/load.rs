//! What the player carries, by mass: derived from the inventory and the
//! gear the player owns, never saved.

use super::*;

/// The grams the player carries: every counted unit at its item's mass, and
/// every owned piece of equipment once, worn or packed. Items without a mass
/// weigh nothing. Currency is a ledger, not an item, and weighs nothing.
///
/// Iterates the saved maps in key order and adds with checked arithmetic, so
/// the same state always gives the same total or the same `NumericLimit`.
pub(super) fn carried_mass(world: &WorldSpec, state: &GameState) -> Result<u64, EngineError> {
    let mass = |item: &str| u64::from(world.item(item).and_then(|i| i.mass_g).unwrap_or(0));
    let mut total = 0_u64;
    for (item, quantity) in &state.player.inventory {
        let stack = quantity
            .checked_mul(mass(item))
            .ok_or(EngineError::NumericLimit)?;
        total = total.checked_add(stack).ok_or(EngineError::NumericLimit)?;
    }
    for piece in state.combat.iter().flat_map(|c| c.gear.values()) {
        total = total
            .checked_add(mass(&piece.item))
            .ok_or(EngineError::NumericLimit)?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use realmkit_spec::MASS_BOUND;

    #[test]
    fn a_total_past_u64_is_a_numeric_limit_not_a_wrap() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena");
        let mut world = WorldSpec::load(path).unwrap();
        world.items[0].mass_g = Some(MASS_BOUND);
        let mut state = Engine::new(&world).unwrap().state().clone();
        state
            .player
            .inventory
            .insert("rat_tail".into(), u64::MAX / 2);
        assert!(matches!(
            carried_mass(&world, &state),
            Err(EngineError::NumericLimit)
        ));
        state.player.inventory.insert("rat_tail".into(), 3);
        assert_eq!(
            carried_mass(&world, &state).ok(),
            Some(3 * u64::from(MASS_BOUND))
        );
    }
}
