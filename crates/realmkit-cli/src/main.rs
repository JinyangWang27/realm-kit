use realmkit_spec::{SpecError, WorldSpec};
use saves::Saves;
use std::{
    error::Error,
    io::{self, IsTerminal, Write},
    path::Path,
};

mod input;
mod keys;
mod menu;
mod panels;
mod play;
mod render;
mod saves;
mod session;

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
            play::play_keys(&world, saves, seed, keys::terminal_keys(), &mut output)?
        }
        "play" => play::play(&world, saves, seed, io::stdin().lock(), &mut output)?,
        _ => unreachable!(),
    }
    Ok(())
}
