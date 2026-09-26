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

const USAGE: &str = "RealmKit — static worlds, deterministic adventures\n\n  realmkit play <world-directory> [--line]\n  realmkit validate <world-directory>\n  realmkit inspect <world-directory>";

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
    let line_mode = args.len() == 3 && args[0] == "play" && args[2] == "--line";
    if args.len() != 2 && !line_mode {
        return Err(USAGE.into());
    }
    let action = args[0].to_str().ok_or(USAGE)?;
    if !matches!(action, "play" | "validate" | "inspect") {
        return Err(USAGE.into());
    }
    let world = WorldSpec::load(Path::new(&args[1]))?;
    match action {
        "validate" => writeln!(output, "{}: valid (format {})", world.world.name, world.world.format_version)?,
        "inspect" => writeln!(output, "{} [{}]\nLanguage: {}\n{} locations, {} NPCs, {} monsters, {} items, {} quests, {} dialogues\nStart: {}", world.world.name, world.world.id, world.world.language, world.locations.len(), world.npcs.len(), world.monsters.len(), world.items.len(), world.quests.len(), world.dialogues.len(), world.world.start)?,
        "play" if !line_mode && io::stdin().is_terminal() && output.is_terminal() => {
            play_keys(&world, terminal_keys(), &mut output)?
        }
        "play" => play(&world, io::stdin().lock(), &mut output)?,
        _ => unreachable!(),
    }
    Ok(())
}

/// Runs one engine command and prints its events or the reason it was refused.
fn apply(engine: &mut Engine<'_>, command: Command, output: &mut impl Write) -> io::Result<()> {
    match engine.execute(command) {
        Ok(events) => render::events(output, engine, &events),
        Err(EngineError::ExitLocked {
            location,
            direction,
        }) => writeln!(
            output,
            "{}",
            engine.world().location(&location).unwrap().exits[&direction].blocked_text
        ),
        Err(error) => writeln!(output, "{error}"),
    }
}

fn start<'w>(world: &'w WorldSpec, output: &mut impl Write) -> Result<Engine<'w>, Box<dyn Error>> {
    let mut engine = Engine::new(world)?;
    writeln!(
        output,
        "{}\nProgress lasts for this session.\n",
        world.world.name
    )?;
    let events = engine.execute(Command::Look)?;
    render::events(output, &engine, &events)?;
    Ok(engine)
}

/// Line-oriented play for pipes, scripts and terminals without raw input.
fn play(
    world: &WorldSpec,
    mut input: impl BufRead,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut engine = start(world, output)?;
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
                writeln!(output, "{}", input::HELP)?;
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
        apply(&mut engine, command, output)?;
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

/// Reads a typed command from keys, echoing it; `None` when cancelled with Esc.
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
            Some(Key::Esc | Key::Quit) | None => {
                writeln!(output)?;
                return Ok(None);
            }
            _ => {}
        }
    }
    writeln!(output)?;
    Ok(Some(line))
}

/// Menu-driven play from key presses. The engine never sees keys, only commands.
fn play_keys(
    world: &WorldSpec,
    mut keys: impl Iterator<Item = io::Result<Key>>,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut engine = start(world, output)?;
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
                    writeln!(output, "{}", input::HELP)?;
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
                        continue 'scene;
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
                            writeln!(output, "{}", input::HELP)?;
                            continue 'scene;
                        }
                        Ok(input::Input::Blank) => continue 'scene,
                        Err(message) => {
                            writeln!(output, "Invalid command: {message}.")?;
                            continue 'scene;
                        }
                    }
                }
            };
            // Stepping back from a conversation lasts until the player speaks again.
            leave_dialogue &= !matches!(command, Command::Talk(_) | Command::ChooseDialogue(_));
            apply(&mut engine, command, output)?;
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
            Enter, Enter, Enter, // attack ×3
            Enter, // south
            Enter, Enter, // talk, report the wolf
            Down, Down, Enter, // east
            Down, Enter, // down
        ]);
        let mut output = Vec::new();
        play_keys(&world, keys.into_iter().map(Ok), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        for passage in [
            "> 1. Talk to Elder Mara",
            "Esc back",
            ": status",
            "HP 24/24",
            "> Attack The Ash Wolf",
            "Level 2",
            "She opens the chapel gate",
            "Your journey through the demo is complete.",
        ] {
            assert!(text.contains(passage), "missing {passage:?} in {text}");
        }
    }

    #[test]
    fn key_errors_end_play_with_the_error() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let keys = [Ok(Down), Err(io::Error::other("tty lost"))];
        let error = play_keys(&world, keys.into_iter(), &mut Vec::new()).unwrap_err();
        assert_eq!(error.to_string(), "tty lost");
    }
}
