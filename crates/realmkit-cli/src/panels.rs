//! Panels: views of the current state that spend no time.

use crate::render::{clock, direction_name, duration, money, piece_name, stat_name, Paint};
use realmkit_engine::Engine;
use realmkit_spec::Stat;
use std::io::{self, Write};

/// The location: its name, description, exits (locked or not) and who is here.
pub fn location(
    output: &mut impl Write,
    engine: &Engine<'_>,
    location: &str,
    paint: Paint,
) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    let location = world.location(location).unwrap();
    writeln!(output, "{}", paint.title(&location.name))?;
    if let Some(now) = state.time.and_then(|now| clock(world, now)) {
        writeln!(output, "{}", paint.dim(&now))?;
    }
    writeln!(output, "{}", location.description)?;
    let roads: Vec<String> = world
        .world
        .roads
        .iter()
        .filter_map(|road| {
            let to = world.location(road.leads(&location.id)?)?;
            let mut notes = Vec::new();
            if road.minutes > 0 {
                notes.push(duration(road.minutes));
            }
            if !engine.allows(road.requires.as_ref()) {
                notes.push("closed".into());
            }
            Some(match notes.is_empty() {
                true => to.name.clone(),
                false => format!("{} ({})", to.name, notes.join(", ")),
            })
        })
        .collect();
    if !roads.is_empty() {
        writeln!(output, "Roads: {}", roads.join(", "))?;
    }
    // A place reached only by road has no compass exits to list.
    if !location.exits.is_empty() || roads.is_empty() {
        location_exits(output, engine, location)?;
    }
    for id in engine.placed_here() {
        let character = world.character(id).unwrap();
        if !engine.allows(character.requires.as_ref()) {
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

/// "Exits: north east (locked)", or "Exits: none".
fn location_exits(
    output: &mut impl Write,
    engine: &Engine<'_>,
    location: &realmkit_spec::Location,
) -> io::Result<()> {
    write!(output, "Exits:")?;
    for (direction, exit) in &location.exits {
        write!(
            output,
            " {}{}",
            direction_name(*direction),
            if engine.allows(exit.requires.as_ref()) {
                ""
            } else {
                " (locked)"
            }
        )?;
    }
    if location.exits.is_empty() {
        write!(output, " none")?;
    }
    writeln!(output)
}

/// The market here: each good's buying and selling price, and how many the
/// player carries.
pub fn market(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    let (Some(economy), Some(wallet)) = (world.economy(), &state.economy) else {
        return Ok(());
    };
    let Some(market) = economy.market(&state.player.location) else {
        return Ok(());
    };
    let name = &world.location(&market.location).unwrap().name;
    writeln!(output, "{}", paint.title(&format!("Market at {name}:")))?;
    for good in &economy.goods {
        let Some(quote) = engine.quote(&good.item) else {
            continue;
        };
        let held = state.player.inventory.get(&good.item).copied().unwrap_or(0);
        writeln!(
            output,
            "  {} — buy {} · sell {} · carried {held}",
            world.item(&good.item).unwrap().name,
            money(world, quote.buy),
            money(world, quote.sell),
        )?;
    }
    writeln!(output, "  {}", money(world, wallet.currency))
}

/// Pieces of equipment by number, then counted items.
pub fn inventory(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    writeln!(output, "{}", paint.title("Inventory:"))?;
    if let Some(wallet) = &state.economy {
        writeln!(output, "  {}", money(world, wallet.currency))?;
    }
    let gear = state.combat.as_ref().map(|c| &c.gear);
    if state.player.inventory.is_empty() && gear.is_none_or(|g| g.is_empty()) {
        writeln!(output, "  Empty")?;
    }
    // Each piece of equipment is listed on its own, by number.
    for (id, piece) in gear.into_iter().flatten() {
        let item = world.item(&piece.item).unwrap();
        let worn = if piece.equipped { " [equipped]" } else { "" };
        let name = piece_name(world, piece);
        writeln!(output, "  #{id} {name}{worn} — {}", item.description)?;
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
pub fn status(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let player = &world.character(&world.world.player).unwrap().name;
    let (Some(combat), Some(stats), Some(vitals), Some(rules)) = (
        &engine.state().combat,
        engine.player_stats(),
        engine.player_vitals(),
        world.combat(),
    ) else {
        return writeln!(output, "{}", paint.title(player));
    };
    // Who and where: the level, and the core internal art's rank as the realm.
    let mut heading = format!("{player} — Level {}", combat.level);
    let realm = rules.core_art.as_ref().and_then(|core| {
        let learned = combat.techniques.get(core)?;
        Some(&world.technique(core)?.ranks[learned.rank - 1].name)
    });
    if let Some(realm) = realm {
        heading += &format!(" · Realm {realm}");
    }
    writeln!(output, "{}", paint.title(&heading))?;
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
pub fn quests(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    writeln!(output, "{}", paint.title("Quests:"))?;
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
pub fn techniques(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    writeln!(output, "{}", paint.title("Techniques:"))?;
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
