//! The world header, locations, the player and other characters.

use super::*;

/// Format, names, language, unique IDs and the starting location.
pub(super) fn header(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let owner = &w.world.id;
    ids(out, "world", std::iter::once(owner.as_str()));
    if w.world.format_version != FORMAT_VERSION {
        issue(
            out,
            owner,
            "unsupported_version",
            format!(
                "expected format version {FORMAT_VERSION}, got {}",
                w.world.format_version
            ),
        );
    }
    if w.world.name.trim().is_empty() || w.characters.iter().any(|c| c.name.trim().is_empty()) {
        issue(
            out,
            owner,
            "empty_name",
            "world and character names must not be empty",
        );
    }
    if w.world.language.trim().is_empty() {
        issue(
            out,
            owner,
            "missing_language",
            "declare the language of the world's player-facing content",
        );
    }
    ids(out, "location", w.locations.iter().map(|v| v.id.as_str()));
    ids(out, "character", w.characters.iter().map(|v| v.id.as_str()));
    ids(out, "item", w.items.iter().map(|v| v.id.as_str()));
    if w.combat().is_none() {
        for item in w.items.iter().filter(|i| i.equipment.is_some()) {
            issue(
                out,
                &item.id,
                "combat_disabled",
                "this world has no combat block, so nothing can be worn for it",
            );
        }
    }
    for item in &w.items {
        let Some(consumable) = item.consumable else {
            continue;
        };
        if w.combat().is_none() {
            issue(
                out,
                &item.id,
                "combat_disabled",
                "this world has no combat block, so there are no HP or MP to restore",
            );
        }
        let restores = consumable.hp > 0 || consumable.mp > 0;
        let bounded = consumable.hp <= STAT_BOUND && consumable.mp <= STAT_BOUND;
        // MP that no level, point or bonus can ever give is never restored.
        let mp_possible = w.combat().is_none_or(|c| {
            c.levels.iter().any(|l| l.stats.mp > 0)
                || c.stat_points
                    .as_ref()
                    .is_some_and(|p| p.values.get(&Stat::Mp).is_some_and(|v| *v > 0))
                || progression::worst_bonus(w, c, Stat::Mp) > 0
        });
        if !restores || !bounded || item.equipment.is_some() || (consumable.mp > 0 && !mp_possible)
        {
            issue(
                out,
                &item.id,
                "invalid_consumable",
                format!(
                    "a consumable restores 1 to {STAT_BOUND} HP or MP the player can have, and is not equipment"
                ),
            );
        }
    }
    ids(out, "quest", w.quests.iter().map(|v| v.id.as_str()));
    ids(out, "dialogue", w.dialogues.iter().map(|v| v.id.as_str()));
    ids(out, "flag", w.world.flags.iter().map(String::as_str));
    reference(
        out,
        owner,
        "starting location",
        &w.world.start,
        w.location(&w.world.start).is_some(),
    );
}

/// Exits and placements; returns the characters placed somewhere.
pub(super) fn locations<'w>(out: &mut Vec<Diagnostic>, w: &'w WorldSpec) -> BTreeSet<&'w str> {
    let mut placed = BTreeSet::new();
    for l in &w.locations {
        ids(out, "station", l.stations.iter().map(String::as_str));
        if !l.stations.is_empty() && w.combat().is_none() {
            issue(
                out,
                &l.id,
                "combat_disabled",
                "this world has no combat block, so there is nothing to forge",
            );
        }
        if l.safe && w.combat().is_none() {
            issue(
                out,
                &l.id,
                "combat_disabled",
                "this world has no combat block, so there is nothing to rest from",
            );
        }
        for exit in l.exits.values() {
            reference(
                out,
                &l.id,
                "exit destination",
                &exit.destination,
                w.location(&exit.destination).is_some(),
            );
            condition(out, w, &l.id, exit.requires.as_ref());
        }
        ids(
            out,
            "character placement",
            l.characters.iter().map(String::as_str),
        );
        for id in &l.characters {
            let character = w.character(id);
            reference(out, &l.id, "character", id, character.is_some());
            // A fighter's defeat is permanent, so it can be in one place only.
            if !placed.insert(id.as_str()) && character.is_some_and(|c| c.combat.is_some()) {
                issue(
                    out,
                    &l.id,
                    "duplicate_placement",
                    format!("character {id} can fight, so it is one instance and may be placed only once"),
                );
            }
        }
    }
    map(out, w);
    placed
}

/// Either every place has a position or none does, each within the bound
/// and none on top of another.
fn map(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let positioned = w.locations.iter().filter(|l| l.map.is_some()).count();
    for l in &w.locations {
        condition(out, w, &l.id, l.known_when.as_ref());
        // Knowledge is derived, not saved: a condition that can stop holding
        // makes the player forget the place, and lose the road back.
        if l.known_when.as_ref().is_some_and(|c| !lasts(c)) {
            warn(
                out,
                &l.id,
                "forgettable_place",
                "known_when can stop holding, so the player could forget this place; prefer flags, evidence, phases or completed quests",
            );
        }
        if l.known_when.is_some() && positioned == 0 {
            issue(
                out,
                &l.id,
                "map_disabled",
                "places have no map positions, so there is no map to know this place on",
            );
        }
    }
    if positioned == 0 {
        return;
    }
    let mut seen = BTreeSet::new();
    for l in &w.locations {
        let Some(point) = l.map else {
            issue(
                out,
                &l.id,
                "map_partial",
                "other places have a map position, so this one needs one too",
            );
            continue;
        };
        if point.x > MAP_BOUND || point.y > MAP_BOUND {
            issue(
                out,
                &l.id,
                "map_bounds",
                format!("map coordinates are at most {MAP_BOUND}"),
            );
        }
        if !seen.insert((point.x, point.y)) {
            issue(
                out,
                &l.id,
                "map_duplicate",
                format!("another place is already at ({}, {})", point.x, point.y),
            );
        }
    }
}

/// The player's numbers come from the level table, not from a profile.
pub(super) fn player(out: &mut Vec<Diagnostic>, w: &WorldSpec, placed: &BTreeSet<&str>) {
    match w.character(&w.world.player) {
        None => reference(out, &w.world.id, "player character", &w.world.player, false),
        Some(player) => {
            if player.combat.is_some()
                || player.dialogue.is_some()
                || placed.contains(player.id.as_str())
            {
                issue(
                    out,
                    &player.id,
                    "invalid_player",
                    "the player character must have no combat profile or dialogue and be placed at no location",
                );
            }
        }
    }
}

pub(super) fn characters(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    for character in &w.characters {
        if let Some(dialogue) = &character.dialogue {
            reference(
                out,
                &character.id,
                "dialogue",
                dialogue,
                w.dialogue(dialogue).is_some(),
            );
        }
        condition(out, w, &character.id, character.requires.as_ref());
        let fixed = character.combat.is_none()
            && character.army.is_none()
            && character.moves.is_none()
            && character.dialogue.is_some();
        if character.kind == CharacterKind::Feature && !fixed {
            issue(
                out,
                &character.id,
                "invalid_feature",
                "a feature is examined through its dialogue; it cannot fight, lead an army or move",
            );
        }
        if let Some(profile) = &character.combat {
            combat_profile(out, w, &character.id, profile);
        }
    }
}

fn combat_profile(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, profile: &CombatProfile) {
    combat::stats(out, owner, &profile.stats);
    combat::crit(out, owner, profile.basic_crit);
    items(out, w, owner, &profile.loot);
    let usable = profile
        .skills
        .iter()
        .filter_map(|id| w.skill(id))
        .filter(|s| s.level <= profile.level);
    if profile.level == 0 {
        issue(
            out,
            owner,
            "invalid_level",
            "a combat profile's level is 1 or more",
        );
    }
    if let Some(group) = &profile.group {
        reference(out, owner, "group", group, w.group(group).is_some());
    }
    if let Some(rules) = w.combat() {
        ids(
            out,
            "skill reference",
            profile.skills.iter().map(String::as_str),
        );
        for id in &profile.skills {
            reference(out, owner, "skill", id, w.skill(id).is_some());
        }
        combat::usable_skills(
            out,
            rules,
            owner,
            profile.basic_channel,
            usable.map(|s| (s, &profile.stats)),
            &profile.stats,
        );
    }
    if w.combat().is_none() {
        issue(
            out,
            owner,
            "combat_disabled",
            "this world has no combat block, so no character can fight",
        );
    }
}

/// Whether a condition, once it holds, holds for good: flags, evidence,
/// phases reached, technique and proficiency ranks and completed quests
/// never go back, and neither do compositions of them alone.
fn lasts(condition: &Condition) -> bool {
    match condition {
        Condition::All { of } | Condition::Any { of } | Condition::AtLeast { of, .. } => {
            of.iter().all(lasts)
        }
        Condition::Flag { .. }
        | Condition::Evidence { .. }
        | Condition::Phase { .. }
        | Condition::Technique { .. }
        | Condition::Proficiency { .. } => true,
        Condition::Quest { status, .. } => *status == QuestStatus::Completed,
        _ => false,
    }
}
