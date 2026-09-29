use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    queue,
    terminal::{self, Clear, ClearType},
};
use menu::{Key, Menu, Outcome};
use realmkit_engine::{Command, Engine, EngineError};
use realmkit_spec::{SpecError, WorldSpec};
use std::{
    error::Error,
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
};

mod input;
mod menu;
mod render;
mod saves;

use saves::{Entry, Kind, Saves};

const USAGE: &str = "RealmKit — static worlds, deterministic adventures\n\n  realmkit play <world-directory> [--line] [--saves <directory>] [--seed <number>]\n  realmkit validate <world-directory>\n  realmkit inspect <world-directory>";

fn main() {
    if let Err(error) = run() {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
        {
            return;
        }
        if let Some(SpecError::Validation(diagnostics)) = error.downcast_ref::<SpecError>() {
            for diagnostic in diagnostics {
                eprintln!(
                    "{} [{}]: {}",
                    diagnostic.entity_id.as_deref().unwrap_or("world"),
                    diagnostic.code,
                    diagnostic.message
                );
            }
        } else {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut output = io::stdout().lock();
    if args.is_empty() || (args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        writeln!(output, "{USAGE}")?;
        return Ok(());
    }
    let action = args[0].to_str().ok_or(USAGE)?;
    if args.len() < 2 || !matches!(action, "play" | "validate" | "inspect") {
        return Err(USAGE.into());
    }
    let (mut line_mode, mut saves, mut seed) = (false, None, None);
    let mut options = args[2..].iter();
    while let Some(option) = options.next() {
        match option.to_str() {
            Some("--line") if action == "play" => line_mode = true,
            Some("--saves") if action == "play" => {
                saves = Some(Saves::open(options.next().ok_or(USAGE)?)?)
            }
            Some("--seed") if action == "play" => {
                let value = options.next().and_then(|v| v.to_str()).ok_or(USAGE)?;
                seed = Some(value.parse::<u64>().map_err(|_| USAGE)?)
            }
            _ => return Err(USAGE.into()),
        }
    }
    let saves = saves.as_ref();
    let world = WorldSpec::load(Path::new(&args[1]))?;
    match action {
        "validate" => writeln!(output, "{}: valid (format {})", world.world.name, world.world.format_version)?,
        "inspect" => writeln!(output, "{} [{}]\nLanguage: {}\n{} locations, {} characters, {} items, {} quests, {} dialogues\nStart: {}", world.world.name, world.world.id, world.world.language, world.locations.len(), world.characters.len(), world.items.len(), world.quests.len(), world.dialogues.len(), world.world.start)?,
        "play" if !line_mode && io::stdin().is_terminal() && output.is_terminal() => {
            play_keys(&world, saves, seed, terminal_keys(), &mut output)?
        }
        "play" => play(&world, saves, seed, io::stdin().lock(), &mut output)?,
        _ => unreachable!(),
    }
    Ok(())
}

/// Runs one engine command and prints its events or the reason it was refused.
/// With saves on, completing a quest auto-saves and death restores the newest
/// save. Returns whether a save replaced the playthrough.
fn apply(
    engine: &mut Engine<'_>,
    saves: Option<&Saves>,
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
        Err(error) => return writeln!(output, "{error}").map(|()| false),
    };
    // Checkpoint before printing, so a closed terminal cannot lose the progress.
    let completed = events
        .iter()
        .any(|e| matches!(e, realmkit_engine::Event::QuestCompleted { .. }));
    if let (Some(saves), true) = (saves, completed) {
        save(engine, saves, Kind::Auto, output)?;
    }
    render::events(output, engine, &events)?;
    if let (Some(saves), true) = (saves, events.contains(&realmkit_engine::Event::PlayerDied)) {
        writeln!(output, "\nRestoring your most recent save…")?;
        return restore(engine, saves, None, output);
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
    index: Option<usize>,
    output: &mut impl Write,
) -> io::Result<bool> {
    let world = engine.world();
    let loaded = saves.load(index, |snapshot| {
        let restored = Engine::restore(world, snapshot)?;
        // Recovery resumes a living player; a dead save could never recover.
        if restored.is_dead() {
            return Err("the player is dead in this save".into());
        }
        Ok(restored)
    });
    match loaded {
        Ok((index, restored)) => {
            *engine = restored;
            writeln!(output, "Loaded save {}.\n", index + 1)?;
            let events = engine
                .execute(Command::Look)
                .expect("looking is always allowed");
            render::events(output, engine, &events)?;
            // Resume a conversation with the line its choices answer.
            if let Some(dialogue) = engine.state().dialogue.clone() {
                let event = realmkit_engine::Event::Dialogue {
                    npc: dialogue.npc,
                    node: dialogue.node,
                    choices: Vec::new(),
                };
                render::events(output, engine, &[event])?;
            }
            Ok(true)
        }
        Err((None, error)) => writeln!(output, "{error}").map(|()| false),
        Err((Some(index), error)) => {
            writeln!(output, "Save {} could not be loaded: {error}", index + 1)?;
            let entries = saves.entries().unwrap_or_default();
            let older = &entries[..index.min(entries.len())];
            if !older.is_empty() {
                writeln!(output, "Type load <number> to restore an older save:")?;
                list(older, output)?;
            }
            Ok(false)
        }
    }
}

/// Save commands belong to the client; the engine only captures and checks snapshots.
fn persist(
    engine: &mut Engine<'_>,
    saves: Option<&Saves>,
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
                Some(index) => restore(engine, saves, Some(index), output).map(drop),
                None => writeln!(output, "Choose one of the listed saves."),
            }
        }
        _ => unreachable!("only save commands are persisted"),
    }
}

/// Resumes the newest save when there is one; otherwise starts the route and
/// auto-saves its start.
fn start<'w>(
    world: &'w WorldSpec,
    saves: Option<&Saves>,
    seed: Option<u64>,
    output: &mut impl Write,
) -> Result<Engine<'w>, Box<dyn Error>> {
    // Without a chosen seed, the clock picks one; it is printed so a run
    // with random content can be replayed with --seed.
    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
    });
    let mut engine = Engine::new_with_seed(world, seed)?;
    let note = if saves.is_some() {
        "Progress is saved when you complete a quest; type save to save now."
    } else {
        "Progress lasts for this session."
    };
    writeln!(output, "{}\n{note}\n", world.world.name)?;
    if let Some(saves) = saves {
        let resumable = !saves.entries().is_ok_and(|e| e.is_empty());
        if resumable && restore(&mut engine, saves, None, output)? {
            return Ok(engine);
        }
        if resumable {
            writeln!(
                output,
                "Starting a new game; older saves stay available with load.\n"
            )?;
        }
        // The new start becomes the newest save, so death recovery never
        // falls back to the save that just failed.
        save(&engine, saves, Kind::Auto, output)?;
    }
    if world.stochastic() {
        writeln!(output, "Seed: {seed}\n")?;
    }
    let events = engine.execute(Command::Look)?;
    render::events(output, &engine, &events)?;
    Ok(engine)
}

/// Line-oriented play for pipes, scripts and terminals without raw input.
fn play(
    world: &WorldSpec,
    saves: Option<&Saves>,
    seed: Option<u64>,
    mut input: impl BufRead,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut engine = start(world, saves, seed, output)?;
    let mut menu = Menu::new(&engine, false);
    menu.write(output, false)?;
    writeln!(output, "{}", menu::LINE_HINT)?;
    let mut line = String::new();
    loop {
        write!(output, "\n> ")?;
        output.flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            writeln!(output)?;
            break;
        }
        let command = match input::parse(&line) {
            Ok(input::Input::Quit) => break,
            Ok(input::Input::Blank) => continue,
            Ok(input::Input::Help) => {
                writeln!(output, "{}", input::help(world))?;
                continue;
            }
            Ok(request @ (input::Input::Save | input::Input::Load(_))) => {
                persist(&mut engine, saves, request, output)?;
                menu = Menu::new(&engine, false);
                menu.write(output, false)?;
                continue;
            }
            Ok(input::Input::Select(number)) => match menu.select(number) {
                Some(entry) => entry.command.clone(),
                None => {
                    writeln!(output, "{}", menu::NOT_LISTED)?;
                    continue;
                }
            },
            Ok(input::Input::Command(command)) => command,
            Err(message) => {
                writeln!(
                    output,
                    "Invalid command: {message}. Type help for commands."
                )?;
                continue;
            }
        };
        apply(&mut engine, saves, command, output)?;
        menu = Menu::new(&engine, false);
        menu.write(output, false)?;
    }
    Ok(())
}

fn erase(output: &mut impl Write, lines: u16) -> io::Result<()> {
    queue!(
        output,
        MoveUp(lines),
        MoveToColumn(0),
        Clear(ClearType::FromCursorDown)
    )
}

/// Reads a typed command from keys, echoing it. Esc cancels (an empty line);
/// `None` means quit (Ctrl-C/Ctrl-D or end of input).
fn read_typed(
    keys: &mut impl Iterator<Item = io::Result<Key>>,
    output: &mut impl Write,
) -> io::Result<Option<String>> {
    let mut line = String::new();
    write!(output, ": ")?;
    loop {
        output.flush()?;
        match keys.next().transpose()? {
            Some(Key::Enter) => break,
            Some(Key::Char(c)) => {
                line.push(c);
                write!(output, "{c}")?;
            }
            Some(Key::Backspace) if line.pop().is_some() => write!(output, "\u{8} \u{8}")?,
            Some(Key::Esc) => {
                writeln!(output)?;
                return Ok(Some(String::new()));
            }
            Some(Key::Quit) | None => return Ok(None),
            _ => {}
        }
    }
    writeln!(output)?;
    Ok(Some(line))
}

/// Menu-driven play from key presses. The engine never sees keys, only commands.
fn play_keys(
    world: &WorldSpec,
    saves: Option<&Saves>,
    seed: Option<u64>,
    mut keys: impl Iterator<Item = io::Result<Key>>,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut engine = start(world, saves, seed, output)?;
    let mut leave_dialogue = false;
    'scene: loop {
        let mut menu = Menu::new(&engine, leave_dialogue);
        let mut lines = menu.write(output, true)?;
        loop {
            output.flush()?;
            let key = match keys.next().transpose()? {
                None | Some(Key::Quit) => break 'scene,
                Some(key) => key,
            };
            let command = match menu.handle(key) {
                Outcome::Ignore => continue,
                Outcome::Redraw => {
                    erase(output, lines)?;
                    lines = menu.write(output, true)?;
                    continue;
                }
                Outcome::Back => {
                    erase(output, lines)?;
                    leave_dialogue = true;
                    continue 'scene;
                }
                Outcome::Help => {
                    writeln!(output, "{}", input::help(world))?;
                    continue 'scene;
                }
                Outcome::Run(command) => {
                    let label = menu.entries.iter().find(|e| e.command == command);
                    erase(output, lines)?;
                    if let Some(entry) = label {
                        writeln!(output, "> {}", entry.label)?;
                    }
                    command
                }
                Outcome::Typed => {
                    let Some(line) = read_typed(&mut keys, output)? else {
                        break 'scene;
                    };
                    match input::parse(&line) {
                        Ok(input::Input::Quit) => break 'scene,
                        Ok(input::Input::Command(command)) => command,
                        Ok(input::Input::Select(number)) => match menu.select(number) {
                            Some(entry) => entry.command.clone(),
                            None => {
                                writeln!(output, "{}", menu::NOT_LISTED)?;
                                continue 'scene;
                            }
                        },
                        Ok(input::Input::Help) => {
                            writeln!(output, "{}", input::help(world))?;
                            continue 'scene;
                        }
                        Ok(input::Input::Blank) => continue 'scene,
                        Ok(request @ (input::Input::Save | input::Input::Load(_))) => {
                            persist(&mut engine, saves, request, output)?;
                            leave_dialogue = false;
                            continue 'scene;
                        }
                        Err(message) => {
                            writeln!(output, "Invalid command: {message}.")?;
                            continue 'scene;
                        }
                    }
                }
            };
            // Stepping back from a conversation lasts until the player speaks again.
            leave_dialogue &= !matches!(command, Command::Talk(_) | Command::ChooseDialogue(_));
            // A restored save may be mid-conversation; show its choices again.
            if apply(&mut engine, saves, command, output)? {
                leave_dialogue = false;
            }
            continue 'scene;
        }
    }
    writeln!(output)?;
    Ok(())
}

/// Leaves raw mode when dropped, including on `?` errors and panics.
struct RawGuard;

impl RawGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Raw mode is held only while waiting for a key, so rendering keeps plain `\n`.
fn terminal_keys() -> impl Iterator<Item = io::Result<Key>> {
    std::iter::from_fn(|| {
        let _raw = match RawGuard::new() {
            Ok(guard) => guard,
            Err(error) => return Some(Err(error)),
        };
        loop {
            let event = match event::read() {
                Ok(event) => event,
                Err(error) => return Some(Err(error)),
            };
            let Event::Key(key) = event else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            return Some(Ok(match key.code {
                KeyCode::Char('c' | 'd') if ctrl => Key::Quit,
                KeyCode::Up => Key::Up,
                KeyCode::Down => Key::Down,
                KeyCode::Enter => Key::Enter,
                KeyCode::Esc => Key::Esc,
                KeyCode::Backspace => Key::Backspace,
                KeyCode::Char(c) if !ctrl => Key::Char(c),
                _ => continue,
            }));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Key::*;

    #[test]
    fn arrows_and_enter_alone_finish_the_demo() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let mut keys = vec![Enter, Esc, Char(':')];
        keys.extend("status".chars().map(Char));
        keys.extend([
            Enter, // typed status
            Enter, Enter, Enter, // talk, ask, accept
            Down, Enter, // north
            Enter, Enter, Enter, Enter, // engage, attack ×3
            Enter, // south
            Enter, Enter, // talk, report the wolf
            Down, Down, Enter, // east
            Down, Enter, // down
        ]);
        let mut output = Vec::new();
        play_keys(&world, None, None, keys.into_iter().map(Ok), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        for passage in [
            "> 1. Talk to Elder Mara",
            "Esc back",
            ": status",
            "HP 40/40",
            "> Attack The Ash Wolf",
            "Level 2",
            "She opens the chapel gate",
            "Your journey through the demo is complete.",
        ] {
            assert!(text.contains(passage), "missing {passage:?} in {text}");
        }
    }

    #[test]
    fn quitting_from_the_typed_prompt_ends_play() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let mut output = Vec::new();
        let keys = [Char(':'), Char('x'), Quit, Enter];
        play_keys(&world, None, None, keys.into_iter().map(Ok), &mut output).unwrap();
        assert!(!String::from_utf8(output)
            .unwrap()
            .contains("looking at the bell"));
    }

    #[test]
    fn key_errors_end_play_with_the_error() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let keys = [Ok(Down), Err(io::Error::other("tty lost"))];
        let error = play_keys(&world, None, None, keys.into_iter(), &mut Vec::new()).unwrap_err();
        assert_eq!(error.to_string(), "tty lost");
    }

    #[test]
    fn each_level_up_reports_the_mp_of_its_own_level() {
        let mut world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        world.world.combat.as_mut().unwrap().levels[2].stats.mp = 5;
        world.characters[2].combat.as_mut().unwrap().xp = 30;
        let mut output = Vec::new();
        let input = "north\nengage wolf\nattack wolf\nattack wolf\nattack wolf\n".as_bytes();
        play(&world, None, None, input, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Level 2! Health restored."), "{text}");
        assert!(text.contains("Level 3! Health and MP restored."), "{text}");
    }

    #[test]
    fn a_level_up_mentions_mp_bought_with_points() {
        // The arena has no base MP; buying it with a point makes level-ups restore it.
        let mut world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap();
        let points = world
            .world
            .combat
            .as_mut()
            .unwrap()
            .stat_points
            .as_mut()
            .unwrap();
        points.values.insert(realmkit_spec::Stat::Mp, 5);
        let mut output = Vec::new();
        let rats = "engage rat\nattack rat\n".repeat(3);
        let input = format!("allocate mp\neast\n{rats}");
        play(&world, None, Some(1), input.as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Level 2! Health and MP restored."), "{text}");
    }

    #[test]
    fn a_sealed_rank_shows_its_waiting_progress() {
        let mut world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect")).unwrap();
        world.world.combat.as_mut().unwrap().player_techniques[0].xp = 40;
        let mut output = Vec::new();
        play(&world, None, None, "techniques\n".as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("  Azure Breath — Second Layer (30/30 to Third Layer, sealed)"),
            "{text}"
        );
    }

    #[test]
    fn death_restores_the_newest_save() {
        let mut world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        world.characters[2].combat.as_mut().unwrap().stats.patk = 100;
        let dir = std::env::temp_dir().join(format!("realmkit-death-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let saves = Saves::open(&dir).unwrap();
        let mut output = Vec::new();
        let input = "north\nsave\nengage wolf\nsave\nstatus\n".as_bytes();
        play(&world, Some(&saves), None, input, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let died = text.find("Your journey has ended").expect(&text);
        let after = &text[died..];
        assert!(after.contains("Loaded save 2."), "{after}");
        assert!(after.contains("The Pine Track"), "{after}");
        assert!(after.contains("HP 40/40"), "{after}");
        assert!(!after.contains("You cannot save now"), "{after}");

        let mut dead = Engine::new(&world).unwrap();
        dead.execute(Command::Move(realmkit_spec::Direction::North))
            .unwrap();
        dead.execute(Command::Engage("wolf".into())).unwrap();
        saves.write(Kind::Manual, &dead.snapshot()).unwrap();
        let mut output = Vec::new();
        play(&world, Some(&saves), None, "".as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("the player is dead in this save"), "{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
