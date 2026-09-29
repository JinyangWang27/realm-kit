//! Individual pieces of equipment: saved instances, what wearing them adds to
//! effective stats, and the damage modifiers they give the wearer.

use super::*;
use realmkit_spec::Equipment;

fn equipment<'w>(world: &'w WorldSpec, item: &str) -> &'w Equipment {
    world.item(item).unwrap().equipment.as_ref().unwrap()
}

/// The equipped pieces, in instance order.
pub(super) fn worn<'w>(world: &'w WorldSpec, combat: &CombatState) -> Vec<&'w Equipment> {
    combat
        .gear
        .values()
        .filter(|g| g.equipped)
        .map(|g| equipment(world, &g.item))
        .collect()
}

/// Adds equipped bonuses, then takes away speed penalties; speed stays at
/// least 1. The speed cap applies later, when turns are scheduled.
pub(super) fn apply(world: &WorldSpec, combat: &CombatState, stats: &mut Stats) {
    let mut penalty = 0_u32;
    for piece in worn(world, combat) {
        for (stat, bonus) in &piece.bonuses {
            let total = stats.get_mut(*stat);
            *total = total.saturating_add(*bonus);
        }
        penalty = penalty.saturating_add(piece.speed_penalty);
    }
    stats.speed = stats.speed.saturating_sub(penalty).max(1);
}

/// A worn weapon's basic-attack channel and time, if any piece sets them.
pub(super) fn basic(world: &WorldSpec, combat: &CombatState) -> (Option<Channel>, Option<u32>) {
    let pieces = worn(world, combat);
    (
        pieces.iter().find_map(|p| p.basic_channel),
        pieces.iter().find_map(|p| p.basic_time),
    )
}

/// The multiplier worn gear applies to damage on `channel`: modifiers multiply,
/// any immunity wins (0), and the product stays within 1/10 to 10.
pub(super) fn modifier(world: &WorldSpec, combat: &CombatState, channel: Channel) -> (u64, u64) {
    // At most 32 slots of factors up to 10: products stay below 10^32, exact in u128.
    let (mut num, mut den) = (1_u128, 1_u128);
    for piece in worn(world, combat) {
        if let Some(m) = piece.modifiers.get(&channel) {
            if m.num == 0 {
                return (0, 1);
            }
            num *= u128::from(m.num);
            den *= u128::from(m.den);
        }
    }
    if num > den * 10 {
        (10, 1)
    } else if num * 10 < den {
        (1, 10)
    } else {
        // Within 1/10 to 10: reduce, then scale both terms down together until
        // they fit the damage arithmetic; the ratio barely moves.
        let divisor = gcd(num, den);
        let (mut num, mut den) = (num / divisor, den / divisor);
        while num > 1_000_000 || den > 1_000_000 {
            (num, den) = ((num / 2).max(1), (den / 2).max(1));
        }
        (num as u64, den as u64)
    }
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Creates `quantity` new pieces of `item` in the pack.
pub(super) fn receive(
    combat: &mut CombatState,
    item: &Id,
    quantity: u64,
) -> Result<(), EngineError> {
    for _ in 0..quantity {
        let id = combat.next_gear;
        combat.next_gear = id.checked_add(1).ok_or(EngineError::NumericLimit)?;
        combat.gear.insert(
            id,
            Gear {
                item: item.clone(),
                equipped: false,
            },
        );
    }
    Ok(())
}

/// The world's starting gear, each piece worn in order while its slots are free.
pub(super) fn receive_starting(
    world: &WorldSpec,
    combat: &mut CombatState,
) -> Result<(), EngineError> {
    for item in &world.combat().unwrap().player_equipment {
        let id = combat.next_gear;
        receive(combat, item, 1)?;
        let slots = &equipment(world, item).slots;
        let taken = worn(world, combat)
            .iter()
            .any(|worn| worn.slots.iter().any(|s| slots.contains(s)));
        if !taken {
            combat.gear.get_mut(&id).unwrap().equipped = true;
        }
    }
    Ok(())
}

/// Wears a piece; whatever occupies any of its slots returns to the pack.
pub(super) fn equip(
    world: &WorldSpec,
    state: &mut GameState,
    gear: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let combat = state.combat.as_mut().ok_or(EngineError::NoSuchGear(gear))?;
    let item = combat
        .gear
        .get(&gear)
        .ok_or(EngineError::NoSuchGear(gear))?
        .item
        .clone();
    if combat.gear[&gear].equipped {
        return Err(EngineError::AlreadyEquipped(gear));
    }
    let slots = &equipment(world, &item).slots;
    let displaced: Vec<u64> = combat
        .gear
        .iter()
        .filter(|(_, g)| g.equipped)
        .filter(|(_, g)| {
            equipment(world, &g.item)
                .slots
                .iter()
                .any(|s| slots.contains(s))
        })
        .map(|(id, _)| *id)
        .collect();
    for id in displaced {
        combat.gear.get_mut(&id).unwrap().equipped = false;
        events.push(Event::Unequipped { gear: id });
    }
    combat.gear.get_mut(&gear).unwrap().equipped = true;
    events.push(Event::Equipped { gear });
    rules::clamp_vitals(world, state);
    Ok(())
}

pub(super) fn unequip(
    world: &WorldSpec,
    state: &mut GameState,
    gear: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let combat = state.combat.as_mut().ok_or(EngineError::NoSuchGear(gear))?;
    let piece = combat
        .gear
        .get_mut(&gear)
        .ok_or(EngineError::NoSuchGear(gear))?;
    if !piece.equipped {
        return Err(EngineError::NotEquipped(gear));
    }
    piece.equipped = false;
    events.push(Event::Unequipped { gear });
    rules::clamp_vitals(world, state);
    Ok(())
}
