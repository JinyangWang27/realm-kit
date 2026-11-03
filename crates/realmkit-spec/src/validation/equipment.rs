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
            if let Some(ranged) = gear.ranged {
                drawn(out, combat, &item.id, ranged);
            }
        }
    }
    competing_weapons(out, w);
    for id in &combat.player_equipment {
        let wearable = w.item(id).is_some_and(|i| i.equipment.is_some());
        reference(out, &w.world.player, "equipment item", id, wearable);
    }
}

/// A drawn weapon needs a real-time clock, timing and power in bounds, and a
/// stagger for its impact to cause: one of 0 steps would never end.
fn drawn(out: &mut Vec<Diagnostic>, combat: &Combat, id: &str, w: RangedWeapon) {
    if w.impact > 0 && combat.realtime.is_some_and(|r| r.stagger == 0) {
        issue(
            out,
            id,
            "missing_stagger",
            "a drawn weapon with impact staggers, so give the stagger 1 or more steps",
        );
    }
    let phase = |steps: u32| (1..=PHASE_BOUND).contains(&steps);
    let power = |p: u32| (POWER_BOUNDS.0..=POWER_BOUNDS.1).contains(&p);
    if combat.realtime.is_none()
        || !(phase(w.draw) && phase(w.min_draw) && phase(w.recovery) && phase(w.flight))
        || w.min_draw > w.draw
        || !(power(w.power) && power(w.min_power))
        || w.min_power > w.power
        || w.impact > POISE_BOUND
    {
        issue(
            out,
            id,
            "invalid_ranged",
            format!("a drawn weapon needs a real-time world; draw, recovery and flight are 1 to {PHASE_BOUND} steps, with the shortest draw and least power no more than the full ones"),
        );
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

/// Pieces that set the basic attack or are drawn must share a slot, so at
/// most one is worn.
fn competing_weapons(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let weapons: Vec<_> = w
        .items
        .iter()
        .filter_map(|i| Some((&i.id, i.equipment.as_ref()?)))
        .filter(|(_, g)| g.basic_channel.is_some() || g.basic_time.is_some() || g.ranged.is_some())
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
                "pieces that change the basic attack or are drawn must share a slot",
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
        let drawn = gear.ranged.map(|r| r.channel);
        let channels = [gear.basic_channel, drawn].into_iter().flatten();
        // Every tier, since a tier's bonuses replace the ones before.
        let unarmed = channels.into_iter().any(|channel| {
            (0..=gear.tiers.len()).any(|tier| {
                let mut stats = first;
                for (stat, bonus) in gear.bonuses_at(tier) {
                    let total = stats.get_mut(*stat);
                    *total = total.saturating_add(*bonus);
                }
                stats.combined(channel, combat.cross_share, false) == 0
            })
        });
        if unarmed {
            issue(
                out,
                &item.id,
                "no_attack",
                "this weapon's attack needs attack in its channel",
            );
        }
    }
}
