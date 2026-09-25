use realmkit_engine::{Command, Engine, EngineError};
use realmkit_spec::{SpecError, WorldSpec};
use std::{
    error::Error,
    io::{self, BufRead, Write},
    path::Path,
};

mod input;
mod render;

const USAGE: &str = "RealmKit — static worlds, deterministic adventures\n\n  realmkit play <world-directory>\n  realmkit validate <world-directory>\n  realmkit inspect <world-directory>";

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
    if args.len() != 2 {
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
        "play" => play(&world, io::stdin().lock(), &mut output)?,
        _ => unreachable!(),
    }
    Ok(())
}

fn play(
    world: &WorldSpec,
    mut input: impl BufRead,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut engine = Engine::new(world)?;
    writeln!(
        output,
        "{}\nType help for commands. Progress lasts for this session.\n",
        world.world.name
    )?;
    let events = engine.execute(Command::Look)?;
    render::events(output, &engine, &events)?;
    let mut line = String::new();
    loop {
        write!(output, "\n> ")?;
        output.flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            writeln!(output)?;
            break;
        }
        match input::parse(&line) {
            Ok(input::Input::Quit) => break,
            Ok(input::Input::Blank) => continue,
            Ok(input::Input::Help) => writeln!(output, "{}", input::HELP)?,
            Ok(input::Input::Command(command)) => match engine.execute(command) {
                Ok(events) => render::events(output, &engine, &events)?,
                Err(EngineError::ExitLocked {
                    location,
                    direction,
                }) => writeln!(
                    output,
                    "{}",
                    world.location(&location).unwrap().exits[&direction].blocked_text
                )?,
                Err(error) => writeln!(output, "{error}")?,
            },
            Err(message) => writeln!(
                output,
                "Invalid command: {message}. Type help for commands."
            )?,
        }
    }
    Ok(())
}
