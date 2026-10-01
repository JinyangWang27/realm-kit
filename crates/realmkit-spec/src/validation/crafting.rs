//! Recipes and improvement tiers.

use super::*;

/// Recipes forge wearable items at stations some location offers, from real
/// materials; tiers name the piece by template and cost real materials too.
pub(super) fn crafting(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    let stations: BTreeSet<&str> = w
        .locations
        .iter()
        .flat_map(|l| &l.stations)
        .map(String::as_str)
        .collect();
    ids(out, "recipe", combat.recipes.iter().map(|r| r.id.as_str()));
    for recipe in &combat.recipes {
        let id = &recipe.id;
        station(out, id, &recipe.station, &stations);
        materials(out, w, id, &recipe.inputs);
        let wearable = w.item(&recipe.output).and_then(|i| i.equipment.as_ref());
        if wearable.is_none() {
            issue(
                out,
                id,
                "invalid_output",
                format!("a recipe forges a wearable item: {}", recipe.output),
            );
        }
        condition(out, w, id, recipe.known_when.as_ref());
        condition(out, w, id, recipe.requires.as_ref());
        trains(out, w, id, recipe.trains.as_ref());
    }
    enchantments(out, w, combat, &stations);
    for item in &w.items {
        let Some(gear) = &item.equipment else {
            continue;
        };
        for tier in &gear.tiers {
            let id = &item.id;
            template(out, id, &tier.name.0, &["item"]);
            station(out, id, &tier.station, &stations);
            materials(out, w, id, &tier.cost);
            condition(out, w, id, tier.requires.as_ref());
            trains(out, w, id, tier.trains.as_ref());
            if tier
                .bonuses
                .values()
                .chain(&tier.speed_penalty)
                .any(|v| *v > STAT_BOUND)
            {
                issue(
                    out,
                    id,
                    "invalid_stats",
                    format!("tier bonuses and penalties are at most {STAT_BOUND}"),
                );
            }
        }
    }
}

/// Enchantments name the piece by template, fit declared slots, add real
/// bonuses within bounds, and cost a catalyst at a station.
fn enchantments(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    combat: &Combat,
    stations: &BTreeSet<&str>,
) {
    ids(
        out,
        "enchantment",
        combat.enchantments.iter().map(|e| e.id.as_str()),
    );
    for enchantment in &combat.enchantments {
        let id = &enchantment.id;
        template(out, id, &enchantment.name.0, &["item"]);
        if enchantment.slots.is_empty() {
            issue(
                out,
                id,
                "invalid_slots",
                "an enchantment fits at least one slot",
            );
        }
        for slot in &enchantment.slots {
            reference(out, id, "slot", slot, combat.slots.contains(slot));
        }
        let bonuses = enchantment.bonuses.values();
        if !bonuses.clone().any(|v| *v > 0) || bonuses.clone().any(|v| *v > STAT_BOUND) {
            issue(
                out,
                id,
                "invalid_stats",
                format!("an enchantment adds at least one bonus, each at most {STAT_BOUND}"),
            );
        }
        station(out, id, &enchantment.station, stations);
        materials(out, w, id, &enchantment.catalyst);
        condition(out, w, id, enchantment.known_when.as_ref());
        condition(out, w, id, enchantment.requires.as_ref());
        trains(out, w, id, enchantment.trains.as_ref());
    }
}

fn station(out: &mut Vec<Diagnostic>, owner: &str, station: &str, stations: &BTreeSet<&str>) {
    reference(out, owner, "station", station, stations.contains(station));
}

/// At least one material, each a real counted item in a positive quantity,
/// listed once so one count is checked once.
fn materials(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, stacks: &[ItemStack]) {
    ids(out, "material", stacks.iter().map(|s| s.item.as_str()));
    // Equipment arrives as pieces, never as a count crafting could spend.
    for stack in stacks {
        if w.item(&stack.item).is_some_and(|i| i.equipment.is_some()) {
            issue(
                out,
                owner,
                "invalid_material",
                format!("equipment cannot be a crafting material: {}", stack.item),
            );
        }
    }
    if stacks.is_empty() {
        issue(
            out,
            owner,
            "invalid_quantity",
            "forging and improving cost at least one material",
        );
    }
    items(out, w, owner, stacks);
}

/// Crafting gives technique XP; teaching a rank belongs to teachers.
fn trains(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, grant: Option<&TechniqueGrant>) {
    let Some(grant) = grant else {
        return;
    };
    progression::technique_grant(out, w, owner, grant);
    if grant.rank.is_some() {
        issue(
            out,
            owner,
            "invalid_grant",
            "crafting trains a technique with XP; it cannot set a rank",
        );
    }
}
