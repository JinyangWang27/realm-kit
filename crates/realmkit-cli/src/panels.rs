//! Panels: views of the current state that spend no time.

use crate::render::{direction_name, stat_name};
use realmkit_engine::Engine;
use realmkit_spec::Stat;
use std::io::{self, Write};

/// The location: its name, description, exits (locked or not) and who is here.
pub fn location(output: &mut impl Write, engine: &Engine<'_>, location: &str) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    let location = world.location(location).unwrap();
    writeln!(output, "{}\n{}", location.name, location.description)?;
    write!(output, "Exits:")?;
    for (direction, exit) in &location.exits {
        write!(
            output,
            " {}{}",
            direction_name(*direction),
            if engine.conditions_met(&exit.requires) {
                ""
            } else {
                " (locked)"
            }
        )?;
    }
    if location.exits.is_empty() {
        write!(output, " none")?;
    }
    writeln!(output)?;
    for id in &location.characters {
        let character = world.character(id).unwrap();
        if !engine.conditions_met(&character.requires) {
            continue;
        }
        let defeated = state
            .combat
            .as_ref()
            .is_some_and(|c| c.defeated.contains(id));
        // A fighter shows its HP: current in a fight, full otherwise.
        let fighting = engine
            .encounter()
            .and_then(|e| e.participants.iter().find(|p| &p.character == id));
        let hp = fighting
            .map(|p| p.hp)
            .or(character.combat.as_ref().map(|c| c.stats.hp));
        match hp {
            _ if defeated => {}
            Some(0) => {}
            Some(hp) => writeln!(
                output,
                "{} (HP {hp}) — {}",
                character.name, character.description
            )?,
            None => writeln!(output, "{} — {}", character.name, character.description)?,
        }
    }
    Ok(())
}

/// Pieces of equipment by number, then counted items.
pub fn inventory(output: &mut impl Write, engine: &Engine<'_>) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    writeln!(output, "Inventory:")?;
    let gear = state.combat.as_ref().map(|c| &c.gear);
    if state.player.inventory.is_empty() && gear.is_none_or(|g| g.is_empty()) {
        writeln!(output, "  Empty")?;
    }
    // Each piece of equipment is listed on its own, by number.
    for (id, piece) in gear.into_iter().flatten() {
        let item = world.item(&piece.item).unwrap();
        let worn = if piece.equipped { " [equipped]" } else { "" };
        writeln!(output, "  #{id} {}{worn} — {}", item.name, item.description)?;
    }
    for (id, count) in &state.player.inventory {
        let item = world.item(id).unwrap();
        writeln!(
            output,
            "  {} ×{} [{}] — {}",
            item.name, count, id, item.description
        )?;
    }
    Ok(())
}

/// The player's level, realm, vitals, stats, XP and unspent points.
pub fn status(output: &mut impl Write, engine: &Engine<'_>) -> io::Result<()> {
    let world = engine.world();
    let player = &world.character(&world.world.player).unwrap().name;
    let (Some(combat), Some(stats), Some(vitals), Some(rules)) = (
        &engine.state().combat,
        engine.player_stats(),
        engine.player_vitals(),
        world.combat(),
    ) else {
        return writeln!(output, "{player}");
    };
    // Who and where: the level, and the core internal art's rank as the realm.
    write!(output, "{player} — Level {}", combat.level)?;
    let realm = rules.core_art.as_ref().and_then(|core| {
        let learned = combat.techniques.get(core)?;
        Some(&world.technique(core)?.ranks[learned.rank - 1].name)
    });
    match realm {
        Some(realm) => writeln!(output, " · Realm {realm}")?,
        None => writeln!(output)?,
    }
    // Vitals; a world or build without MP shows none.
    write!(output, "  HP {}/{}", vitals.hp, stats.hp)?;
    match stats.mp {
        0 => writeln!(output)?,
        max => writeln!(output, " · MP {}/{max}", vitals.mp)?,
    }
    let shown: Vec<String> = [Stat::Patk, Stat::Pdef, Stat::Satk, Stat::Sdef, Stat::Speed]
        .into_iter()
        .map(|s| format!("{} {}", stat_name(world, s), stats.get(s)))
        .collect();
    writeln!(output, "  {}", shown.join(" · "))?;
    // Progress: XP toward the next level, and points waiting to be spent.
    write!(output, "  XP {}", combat.xp)?;
    match rules.levels.get(combat.level) {
        Some(next) => write!(output, " (level {} at {})", combat.level + 1, next.xp)?,
        None => write!(output, " (highest level)")?,
    }
    match engine.unspent_points() {
        Some(1) => writeln!(output, " · 1 point to spend"),
        Some(points) if points > 0 => writeln!(output, " · {points} points to spend"),
        _ => writeln!(output),
    }
}

/// Every quest and its status.
pub fn quests(output: &mut impl Write, engine: &Engine<'_>) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    writeln!(output, "Quests:")?;
    for quest in &world.quests {
        writeln!(
            output,
            "  {} [{}]: {:?}",
            quest.name, quest.id, state.quests[&quest.id]
        )?;
    }
    Ok(())
}

/// Learned techniques, their ranks and progress toward the next.
pub fn techniques(output: &mut impl Write, engine: &Engine<'_>) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    writeln!(output, "Techniques:")?;
    let learned = state.combat.as_ref().map(|c| &c.techniques);
    if learned.is_none_or(|l| l.is_empty()) {
        writeln!(output, "  None yet")?;
    }
    for (id, progress) in learned.into_iter().flatten() {
        let technique = world.technique(id).unwrap();
        let rank = &technique.ranks[progress.rank - 1];
        write!(output, "  {} — {}", technique.name, rank.name)?;
        match technique.ranks.get(progress.rank) {
            // A closed gate holds XP at the next threshold.
            Some(next) if progress.xp >= next.xp => writeln!(
                output,
                " ({}/{} to {}, sealed)",
                progress.xp, next.xp, next.name
            )?,
            Some(next) => writeln!(output, " ({}/{} to {})", progress.xp, next.xp, next.name)?,
            None => writeln!(output, " (mastered)")?,
        }
    }
    Ok(())
}
