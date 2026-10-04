//! Panels: views of the current state that spend no time.

use crate::render::{
    clock, direction_name, duration, money, piece_name, stat_name, workshop_name, Paint,
};
use realmkit_engine::Engine;
use realmkit_spec::{Proficiency, Stat};
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
            // A road to a place the player has not heard of is not shown.
            let to = road.leads(&location.id).filter(|to| engine.knows(to))?;
            let to = world.location(to)?;
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
    // Exits to places the player has not heard of are not shown either.
    let exits: Vec<_> = location
        .exits
        .iter()
        .filter(|(_, exit)| engine.knows(&exit.destination))
        .collect();
    // A place reached only by road has no compass exits to list.
    if !exits.is_empty() || roads.is_empty() {
        location_exits(output, engine, &exits)?;
    }
    for character in engine.present_here() {
        // A fighter shows its HP: current in a fight, full otherwise.
        let fighting = engine
            .encounter()
            .and_then(|e| e.participants.iter().find(|p| p.character == character.id));
        let hp = fighting
            .map(|p| p.hp)
            .or(character.combat.as_ref().map(|c| c.stats.hp));
        match hp {
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
    exits: &[(&realmkit_spec::Direction, &realmkit_spec::Exit)],
) -> io::Result<()> {
    write!(output, "Exits:")?;
    for (direction, exit) in exits {
        write!(
            output,
            " {}{}",
            direction_name(**direction),
            if engine.allows(exit.requires.as_ref()) {
                ""
            } else {
                " (locked)"
            }
        )?;
    }
    if exits.is_empty() {
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
    if let Some(prosperity) = wallet.prosperity.get(&market.location) {
        writeln!(output, "  Prosperity {prosperity}")?;
    }
    for good in &economy.goods {
        let Some(quote) = engine.quote(&good.item) else {
            continue;
        };
        let held = state.player.inventory.get(&good.item).copied().unwrap_or(0);
        let stock = quote
            .stock
            .map_or(String::new(), |n| format!(" · stock {n}"));
        writeln!(
            output,
            "  {} — buy {} · sell {}{stock} · carried {held}",
            world.item(&good.item).unwrap().name,
            money(world, quote.buy),
            money(world, quote.sell),
        )?;
    }
    if !market.wares.is_empty() {
        writeln!(output, "  {}", paint.title("Wares:"))?;
        for ware in &market.wares {
            let item = world.item(&ware.item).unwrap();
            writeln!(
                output,
                "  {} — {}{}",
                item.name,
                money(world, ware.price),
                item.equipment
                    .as_ref()
                    .map_or(String::new(), |e| { bonuses(world, e) })
            )?;
        }
    }
    match wallet.stock.get(&market.location) {
        Some(stock) => writeln!(
            output,
            "  You: {} · Merchants: {}",
            money(world, wallet.currency),
            money(world, stock.currency)
        ),
        None => writeln!(output, "  {}", money(world, wallet.currency)),
    }
}

/// " (Defence +6, Speed -10)": what wearing a piece at its base tier does.
fn bonuses(world: &realmkit_spec::WorldSpec, equipment: &realmkit_spec::Equipment) -> String {
    let mut parts: Vec<String> = Stat::ALL
        .into_iter()
        .filter_map(|stat| {
            let bonus = equipment.bonuses.get(&stat).filter(|b| **b > 0)?;
            Some(format!("{} +{bonus}", stat_name(world, stat)))
        })
        .collect();
    if equipment.speed_penalty > 0 {
        let speed = stat_name(world, Stat::Speed);
        parts.push(format!("{speed} -{}", equipment.speed_penalty));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

/// Soldiers by line and level, with the wounded, each squad's XP toward its
/// next level, the head count against the limit and the wages they draw.
pub fn retinue(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let (Some(troops), Some(retinue)) = (world.troops(), &engine.state().retinue) else {
        return Ok(());
    };
    let heads = retinue.heads();
    writeln!(
        output,
        "{}",
        paint.title(&format!("Retinue: {heads}/{}", troops.limit))
    )?;
    if heads == 0 {
        writeln!(output, "  Nobody yet")?;
    }
    let mut wages = 0;
    for line in &troops.lines {
        let Some(levels) = retinue.roster.get(&line.id) else {
            continue;
        };
        for (level, squad) in levels.iter().rev() {
            wages += line.wage_at(*level) * squad.heads();
            let wounded = if squad.wounded > 0 {
                format!(" · {} wounded", squad.wounded)
            } else {
                String::new()
            };
            let progress = match line.levels.get(*level) {
                Some(next) => format!(" · XP {}/{}", squad.share(), next.xp),
                None if !line.upgrades.is_empty() => " · ready to upgrade".into(),
                None => String::new(),
            };
            writeln!(
                output,
                "  {} (L{level}) ×{}{wounded}{progress}",
                line.name_at(*level),
                squad.heads()
            )?;
        }
    }
    if wages > 0 {
        writeln!(output, "  Wages due: {}", money(world, wages))?;
    }
    Ok(())
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
/// How the player answered the start questions: "Errand: Only to read."
fn answers(output: &mut impl Write, engine: &Engine<'_>) -> io::Result<()> {
    let questions = &engine.world().world.start_questions;
    for (question, choice) in questions.iter().zip(&engine.state().start_choices) {
        let option = question.options.iter().find(|o| &o.id == choice).unwrap();
        writeln!(output, "  {}: {}", question.name, option.text)?;
    }
    Ok(())
}

pub fn status(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let player = &world.character(&world.world.player).unwrap().name;
    let (Some(combat), Some(stats), Some(vitals), Some(rules)) = (
        &engine.state().combat,
        engine.player_stats(),
        engine.player_vitals(),
        world.combat(),
    ) else {
        writeln!(output, "{}", paint.title(player))?;
        answers(output, engine)?;
        return holdings(output, engine);
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
    answers(output, engine)?;
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
        Some(1) => writeln!(output, " · 1 point to spend")?,
        Some(points) if points > 0 => writeln!(output, " · {points} points to spend")?,
        _ => writeln!(output)?,
    }
    holdings(output, engine)
}

/// Proficiency ranks with points waiting, then the player's workshops.
fn holdings(output: &mut impl Write, engine: &Engine<'_>) -> io::Result<()> {
    let world = engine.world();
    let ranks: Vec<String> = Proficiency::ALL
        .into_iter()
        .filter_map(|p| {
            let name = world.proficiency_name(p)?;
            let max = world.proficiency_max(p)?;
            Some(format!("{name} {}/{max}", engine.proficiency_rank(p)))
        })
        .collect();
    if !ranks.is_empty() {
        write!(output, "  {}", ranks.join(" · "))?;
        match engine.unspent_proficiency_points() {
            0 => writeln!(output)?,
            1 => writeln!(output, " · 1 proficiency point to spend")?,
            n => writeln!(output, " · {n} proficiency points to spend")?,
        }
    }
    let Some(wallet) = &engine.state().economy else {
        return Ok(());
    };
    let owned: Vec<String> = wallet
        .workshops
        .iter()
        .flat_map(|(town, kinds)| {
            let town = &world.location(town).unwrap().name;
            kinds.iter().map(move |(kind, count)| match count {
                1 => format!("{} in {town}", workshop_name(world, kind)),
                n => format!("{} ×{n} in {town}", workshop_name(world, kind)),
            })
        })
        .collect();
    if !owned.is_empty() {
        writeln!(output, "  Workshops: {}", owned.join(", "))?;
    }
    Ok(())
}

/// The journal: the story's phase, the quests the player knows of, and the
/// evidence they have found.
pub fn quests(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let world = engine.world();
    let journal = engine.journal();
    if let Some(phase) = &journal.phase {
        let phase = &world.world.phases[world.phase_index(phase).unwrap()];
        writeln!(output, "{}: {}", paint.title("Chapter"), phase.name)?;
    }
    writeln!(output, "{}", paint.title("Quests:"))?;
    for entry in &journal.quests {
        let quest = world.quest(&entry.quest).unwrap();
        let main = if entry.main { " — main" } else { "" };
        writeln!(
            output,
            "  {} [{}]: {:?}{main}",
            quest.name, quest.id, entry.status
        )?;
    }
    if !journal.evidence.is_empty() {
        writeln!(output, "{}", paint.title("Evidence:"))?;
        for id in &journal.evidence {
            let evidence = world.evidence(id).unwrap();
            writeln!(output, "  {}: {}", evidence.name, evidence.description)?;
        }
    }
    if let Some(id) = &journal.outcome {
        let outcome = world.world.outcomes.iter().find(|o| &o.id == id).unwrap();
        writeln!(output, "{}: {}", paint.title("Reached"), outcome.name)?;
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
