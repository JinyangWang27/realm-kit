//! One playthrough's commands and saves: running a command, saving,
//! loading, and starting or resuming play.

use crate::{
    input, render,
    saves::{Entry, Kind, Saves},
};
use realmkit_engine::{Command, Engine, EngineError};
use realmkit_spec::{StartQuestion, WorldSpec};
use std::{
    error::Error,
    io::{self, Write},
};

/// Runs one engine command and prints its events or the reason it was refused.
/// With saves on, completing a quest auto-saves and death restores the newest
/// save. Returns whether a save replaced the playthrough.
pub(crate) fn apply(
    engine: &mut Engine<'_>,
    saves: Option<&Saves>,
    log: &mut render::Log,
    command: Command,
    output: &mut impl Write,
) -> io::Result<bool> {
    let events = match engine.execute(command) {
        Ok(events) => events,
        Err(EngineError::ExitLocked {
            location,
            direction,
        }) => {
            return writeln!(
                output,
                "{}",
                engine.world().location(&location).unwrap().exits[&direction].blocked_text
            )
            .map(|()| false)
        }
        Err(EngineError::RoadBlocked { road }) => {
            let road = engine.world().world.roads.iter().find(|r| r.id == road);
            let text = road.and_then(|r| r.blocked_text.as_deref());
            return writeln!(output, "{}", text.unwrap_or_default()).map(|()| false);
        }
        Err(EngineError::ChoiceBlocked(number)) => {
            let choices = engine.dialogue_choices();
            let text = choices[number - 1].blocked.as_deref();
            return writeln!(output, "{}", text.unwrap_or_default()).map(|()| false);
        }
        Err(error) => return writeln!(output, "{error}").map(|()| false),
    };
    // Checkpoint before printing, so a closed terminal cannot lose the progress.
    let completed = events
        .iter()
        .any(|e| matches!(e, realmkit_engine::Event::QuestCompleted { .. }));
    if let (Some(saves), true) = (saves, completed) {
        save(engine, saves, Kind::Auto, output)?;
    }
    log.events(output, engine, &events)?;
    if let (Some(saves), true) = (saves, events.contains(&realmkit_engine::Event::PlayerDied)) {
        writeln!(output, "\nRestoring your most recent save…")?;
        return restore(engine, saves, log, None, output);
    }
    Ok(false)
}

fn save(engine: &Engine<'_>, saves: &Saves, kind: Kind, output: &mut impl Write) -> io::Result<()> {
    match saves.write(kind, &engine.snapshot()) {
        Ok(()) if kind == Kind::Manual => writeln!(output, "Saved."),
        Ok(()) => Ok(()),
        Err(error) => writeln!(
            output,
            "Saving failed; earlier saves are unchanged: {error}"
        ),
    }
}

fn list(entries: &[Entry], output: &mut impl Write) -> io::Result<()> {
    for (i, entry) in entries.iter().enumerate() {
        let kind = match entry.kind {
            Kind::Auto => "auto-save",
            Kind::Manual => "save",
        };
        writeln!(output, "  {}. {kind}", i + 1)?;
    }
    Ok(())
}

/// Loads the save at `index` (the newest when `None`) and shows where play
/// resumes. A save that fails to load is reported, never skipped; older saves
/// are offered for the player to choose. Returns whether play resumed.
fn restore(
    engine: &mut Engine<'_>,
    saves: &Saves,
    log: &mut render::Log,
    index: Option<usize>,
    output: &mut impl Write,
) -> io::Result<bool> {
    let Some(restored) = load(engine.world(), saves, log, index, output)? else {
        return Ok(false);
    };
    *engine = restored;
    Ok(true)
}

/// [`restore`] without a playthrough to replace: the loaded one, if any.
fn load<'w>(
    world: &'w WorldSpec,
    saves: &Saves,
    log: &mut render::Log,
    index: Option<usize>,
    output: &mut impl Write,
) -> io::Result<Option<Engine<'w>>> {
    let loaded = saves.load(index, |snapshot| {
        let restored = Engine::restore(world, snapshot)?;
        // Recovery resumes a living player; a dead save could never recover.
        if restored.is_dead() {
            return Err("the player is dead in this save".into());
        }
        Ok(restored)
    });
    match loaded {
        Ok((index, engine)) => {
            // Progress since the save is gone, and so is anything tallied for it.
            log.reset();
            writeln!(output, "Loaded save {}.\n", index + 1)?;
            let mut engine = engine;
            let events = engine
                .execute(Command::Look)
                .expect("looking is always allowed");
            render::events(output, &engine, &events, log.paint)?;
            // Resume a conversation with the line its choices answer.
            if let Some(dialogue) = engine.state().dialogue.clone() {
                let event = realmkit_engine::Event::Dialogue {
                    npc: dialogue.npc,
                    node: dialogue.node,
                    choices: Vec::new(),
                };
                render::events(output, &engine, &[event], log.paint)?;
            }
            Ok(Some(engine))
        }
        Err((None, error)) => writeln!(output, "{error}").map(|()| None),
        Err((Some(index), error)) => {
            writeln!(output, "Save {} could not be loaded: {error}", index + 1)?;
            let entries = saves.entries().unwrap_or_default();
            let older = &entries[..index.min(entries.len())];
            if !older.is_empty() {
                writeln!(output, "Type load <number> to restore an older save:")?;
                list(older, output)?;
            }
            Ok(None)
        }
    }
}

/// Save commands belong to the client; the engine only captures and checks snapshots.
pub(crate) fn persist(
    engine: &mut Engine<'_>,
    saves: Option<&Saves>,
    log: &mut render::Log,
    request: input::Input,
    output: &mut impl Write,
) -> io::Result<()> {
    let Some(saves) = saves else {
        return writeln!(
            output,
            "Saving is off. Start with --saves <directory> to keep progress."
        );
    };
    let entries = match saves.entries() {
        Ok(entries) => entries,
        Err(error) => return writeln!(output, "Saves could not be read: {error}"),
    };
    match request {
        input::Input::Save if engine.is_dead() => {
            writeln!(output, "You cannot save now.")
        }
        input::Input::Save => save(engine, saves, Kind::Manual, output),
        input::Input::Load(None) if entries.is_empty() => {
            writeln!(output, "There are no saves yet.")
        }
        input::Input::Load(None) => {
            writeln!(output, "Type load <number> to restore a save:")?;
            list(&entries, output)
        }
        input::Input::Load(Some(number)) => {
            match number.checked_sub(1).filter(|i| *i < entries.len()) {
                Some(index) => restore(engine, saves, log, Some(index), output).map(drop),
                None => writeln!(output, "Choose one of the listed saves."),
            }
        }
        _ => unreachable!("only save commands are persisted"),
    }
}

/// Resumes the newest save when there is one; otherwise asks the start
/// questions through `ask`, starts the route and auto-saves its start.
/// `ask` returns the chosen option's index, or `None` when the player quits,
/// and then so does this.
pub(crate) fn start<'w, W: Write>(
    world: &'w WorldSpec,
    saves: Option<&Saves>,
    seed: Option<u64>,
    log: &mut render::Log,
    output: &mut W,
    mut ask: impl FnMut(&StartQuestion, &mut W) -> io::Result<Option<usize>>,
) -> Result<Option<Engine<'w>>, Box<dyn Error>> {
    // Without a chosen seed, the clock picks one; it is printed so a run
    // with random content can be replayed with --seed.
    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
    });
    let note = if saves.is_some() {
        "Progress is saved when you complete a quest; type save to save now."
    } else {
        "Progress lasts for this session."
    };
    writeln!(output, "{}\n{note}\n", world.world.name)?;
    if let Some(saves) = saves {
        let resumable = !saves.entries().is_ok_and(|e| e.is_empty());
        if resumable {
            if let Some(engine) = load(world, saves, log, None, output)? {
                return Ok(Some(engine));
            }
            writeln!(
                output,
                "Starting a new game; older saves stay available with load.\n"
            )?;
        }
    }
    let mut choices = Vec::new();
    for question in &world.world.start_questions {
        let Some(index) = ask(question, output)? else {
            return Ok(None);
        };
        choices.push(question.options[index].id.clone());
    }
    let mut engine = Engine::start(world, seed, &choices)?;
    // The new start becomes the newest save, so death recovery never
    // falls back to the save that just failed.
    if let Some(saves) = saves {
        save(&engine, saves, Kind::Auto, output)?;
    }
    if world.stochastic() {
        writeln!(output, "Seed: {seed}\n")?;
    }
    let events = engine.execute(Command::Look)?;
    render::events(output, &engine, &events, log.paint)?;
    Ok(Some(engine))
}
