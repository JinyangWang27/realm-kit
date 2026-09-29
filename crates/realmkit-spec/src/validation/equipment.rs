//! Slots and wearable items.

use super::*;

/// Slots exist, gear occupies declared slots within the engine's bounds, and
/// the starting gear is wearable.
pub(super) fn equipment(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    ids(out, "slot", combat.slots.iter().map(String::as_str));
    if combat.slots.len() > SLOT_BOUND {
        issue(
            out,
            &w.world.id,
            "invalid_slots",
            format!("a world has at most {SLOT_BOUND} slots"),
        );
    }
    for item in &w.items {
        if let Some(gear) = &item.equipment {
            piece(out, combat, &item.id, gear);
        }
    }
    competing_weapons(out, w);
    for id in &combat.player_equipment {
        let wearable = w.item(id).is_some_and(|i| i.equipment.is_some());
        reference(out, &w.world.player, "equipment item", id, wearable);
    }
}

fn piece(out: &mut Vec<Diagnostic>, combat: &Combat, id: &str, gear: &Equipment) {
    let mut seen = BTreeSet::new();
    if gear.slots.is_empty() || !gear.slots.iter().all(|s| seen.insert(s)) {
        issue(
            out,
            id,
            "invalid_slots",
            "equipment occupies at least one slot, each once",
        );
    }
    for slot in &gear.slots {
        reference(out, id, "slot", slot, combat.slots.contains(slot));
    }
    if gear
        .bonuses
        .values()
        .chain([&gear.speed_penalty])
        .any(|v| *v > STAT_BOUND)
    {
        issue(
            out,
            id,
            "invalid_stats",
            format!("equipment bonuses and penalties are at most {STAT_BOUND}"),
        );
    }
    if gear
        .basic_time
        .is_some_and(|t| !(TIME_BOUNDS.0..=TIME_BOUNDS.1).contains(&t))
    {
        issue(
            out,
            id,
            "invalid_time",
            "a weapon's basic-attack time is 1 to 1,000 percent",
        );
    }
    if gear
        .modifiers
        .values()
        .any(|m| m.num > 10 || !(1..=10).contains(&m.den))
    {
        issue(
            out,
            id,
            "invalid_modifier",
            "a damage modifier is 0 to 10 over 1 to 10",
        );
    }
}

/// Pieces that set the basic attack must share a slot, so at most one is worn.
fn competing_weapons(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let weapons: Vec<_> = w
        .items
        .iter()
        .filter_map(|i| Some((&i.id, i.equipment.as_ref()?)))
        .filter(|(_, g)| g.basic_channel.is_some() || g.basic_time.is_some())
        .collect();
    for (i, (id, gear)) in weapons.iter().enumerate() {
        if weapons[..i]
            .iter()
            .any(|(_, other)| !other.slots.iter().any(|s| gear.slots.contains(s)))
        {
            issue(
                out,
                id,
                "competing_weapons",
                "pieces that change the basic attack must share a slot",
            );
        }
    }
}

/// A weapon that changes the basic attack's channel needs attack there,
/// counting what the weapon itself adds to the player's `first` stats.
pub(super) fn weapon_channels(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    combat: &Combat,
    first: Stats,
) {
    for item in &w.items {
        let Some(gear) = &item.equipment else {
            continue;
        };
        let Some(channel) = gear.basic_channel else {
            continue;
        };
        let mut stats = first;
        for (stat, bonus) in &gear.bonuses {
            let total = stats.get_mut(*stat);
            *total = total.saturating_add(*bonus);
        }
        if stats.combined(channel, combat.cross_share, false) == 0 {
            issue(
                out,
                &item.id,
                "no_attack",
                "this weapon's basic attack needs attack in its channel",
            );
        }
    }
}
