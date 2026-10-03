//! The two ways to play: menus driven by key presses, and numbered lines
//! for pipes, scripts and `--line`.

use crate::{
    fight::{self, FightScreen},
    input, map,
    menu::{self, Key, Menu, Outcome, Pick},
    render::{self, Paint},
    saves::Saves,
    session::{apply, persist, start},
};
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    queue,
    terminal::{Clear, ClearType},
};
use realmkit_engine::{Command, Engine};
use realmkit_spec::{StartQuestion, WorldSpec};
use std::{
    error::Error,
    io::{self, BufRead, Write},
};

/// Line-oriented play for pipes, scripts and terminals without raw input.
pub(crate) fn play(
    world: &WorldSpec,
    saves: Option<&Saves>,
    seed: Option<u64>,
    mut input: impl BufRead,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut log = render::Log::default();
    let answer =
        |question: &StartQuestion, output: &mut _| ask(world, question, &mut input, output);
    let Some(mut engine) = start(world, saves, seed, &mut log, output, answer)? else {
        writeln!(output)?;
        return Ok(());
    };
    let mut menu = Menu::new(&engine, false, None);
    menu.write(output, false, Paint::default())?;
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
        let command = match input::parse(world, &line) {
            Ok(input::Input::Quit) => break,
            Ok(input::Input::Blank) => continue,
            Ok(input::Input::Help) => {
                writeln!(output, "{}", input::help(world))?;
                continue;
            }
            Ok(input::Input::MapZoom { zoom, place }) => {
                zoomed_map(&engine, zoom, &place, output)?;
                continue;
            }
            Ok(request @ (input::Input::Save | input::Input::Load(_))) => {
                persist(&mut engine, saves, &mut log, request, output)?;
                menu = Menu::new(&engine, false, None);
                menu.write(output, false, Paint::default())?;
                continue;
            }
            Ok(input::Input::Select(number)) => match menu.choose(number) {
                Outcome::Run(command) => command,
                Outcome::Redraw => {
                    menu.write(output, false, Paint::default())?;
                    continue;
                }
                _ => {
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
        let open = menu.stays_open(&command);
        apply(&mut engine, saves, &mut log, command, output)?;
        menu = Menu::new(&engine, false, open);
        menu.write(output, false, Paint::default())?;
    }
    Ok(())
}

/// Prints a closer view of the map centred on a place.
fn zoomed_map(
    engine: &Engine<'_>,
    zoom: u32,
    place: &str,
    output: &mut impl Write,
) -> io::Result<()> {
    let frame = engine
        .map_view()
        .and_then(|view| map::frame(engine.world(), &view, Some((zoom, place))));
    match frame {
        Some(lines) => lines.iter().try_for_each(|line| writeln!(output, "{line}")),
        None => writeln!(output, "{}", menu::NOT_ON_MAP),
    }
}

/// Asks a start question by numbered lines until one of its options is
/// chosen; `None` when the player quits or input ends.
fn ask(
    world: &WorldSpec,
    question: &StartQuestion,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Option<usize>> {
    let mut menu = Menu::question(question);
    menu.write(output, false, Paint::default())?;
    let mut line = String::new();
    loop {
        write!(output, "\n> ")?;
        output.flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        match input::parse(world, &line) {
            Ok(input::Input::Quit) => return Ok(None),
            Ok(input::Input::Select(number)) => {
                if let Outcome::Run(Command::ChooseDialogue(n)) = menu.choose(number) {
                    return Ok(Some(n - 1));
                }
            }
            _ => {}
        }
        writeln!(output, "{}", menu::CHOOSE_ANSWER)?;
    }
}

/// Asks a start question by keys, leaving the question and the answer on
/// screen; `None` when the player quits.
fn ask_keys(
    question: &StartQuestion,
    keys: &mut impl Iterator<Item = io::Result<Key>>,
    output: &mut impl Write,
    paint: Paint,
) -> io::Result<Option<usize>> {
    let mut menu = Menu::question(question);
    let mut lines = menu.write(output, true, paint)?;
    loop {
        output.flush()?;
        let key = match keys.next().transpose()? {
            None | Some(Key::Quit) => return Ok(None),
            Some(key) => key,
        };
        match menu.handle(key) {
            Outcome::Run(Command::ChooseDialogue(n)) => {
                erase(output, lines)?;
                let answer = &question.options[n - 1].text;
                writeln!(output, "\n{}\n> {answer}\n", question.text)?;
                return Ok(Some(n - 1));
            }
            Outcome::Redraw => {
                erase(output, lines)?;
                lines = menu.write(output, true, paint)?;
            }
            _ => {}
        }
    }
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
pub(crate) fn play_keys(
    world: &WorldSpec,
    saves: Option<&Saves>,
    seed: Option<u64>,
    paint: Paint,
    mut keys: impl Iterator<Item = io::Result<Key>>,
    output: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut log = render::Log::new(paint);
    let answer =
        |question: &StartQuestion, output: &mut _| ask_keys(question, &mut keys, output, paint);
    let Some(mut engine) = start(world, saves, seed, &mut log, output, answer)? else {
        writeln!(output)?;
        return Ok(());
    };
    let (mut leave_dialogue, mut open) = (false, None);
    // What the last command printed, shown on whichever screen play is on,
    // and messages that are no command (help, typos, saving), shown apart.
    let (mut pending, mut notice) = (Vec::new(), Vec::new());
    let mut screen: Option<FightScreen> = None;
    'scene: loop {
        settle(&engine, &mut screen, &mut notice, &mut pending, output)?;
        let mut menu = Menu::new(&engine, leave_dialogue, open.take());
        let mut lines = 0;
        match &screen {
            Some(fight) => {
                menu = menu.without_header();
                fight.draw(output, &engine, &menu, paint)?;
            }
            None => lines = menu.write(output, true, paint)?,
        }
        loop {
            output.flush()?;
            let key = match keys.next().transpose()? {
                None | Some(Key::Quit) => break 'scene,
                Some(key) => key,
            };
            let command = match menu.handle(key) {
                Outcome::Ignore => continue,
                Outcome::Redraw => {
                    match &screen {
                        Some(fight) => fight.draw(output, &engine, &menu, paint)?,
                        None => {
                            erase(output, lines)?;
                            lines = menu.write(output, true, paint)?;
                        }
                    }
                    continue;
                }
                Outcome::Back => {
                    erase(output, lines)?;
                    leave_dialogue = true;
                    continue 'scene;
                }
                Outcome::Help => {
                    writeln!(notice, "{}", input::help(world))?;
                    continue 'scene;
                }
                Outcome::Run(command) => {
                    let label = menu
                        .entries()
                        .iter()
                        .find(|e| e.pick == Pick::Run(command.clone()));
                    // The fight screen shows what happened instead of what was chosen.
                    if screen.is_none() {
                        erase(output, lines)?;
                        if let Some(entry) = label {
                            writeln!(output, "> {}", entry.label)?;
                        }
                    }
                    command
                }
                Outcome::Typed => {
                    let Some(line) = read_typed(&mut keys, output)? else {
                        break 'scene;
                    };
                    match input::parse(world, &line) {
                        Ok(input::Input::Quit) => break 'scene,
                        Ok(input::Input::Command(command)) => command,
                        Ok(input::Input::Select(number)) => match menu.choose(number) {
                            Outcome::Run(command) => command,
                            Outcome::Redraw => {
                                open = menu.open;
                                continue 'scene;
                            }
                            _ => {
                                writeln!(notice, "{}", menu::NOT_LISTED)?;
                                continue 'scene;
                            }
                        },
                        Ok(input::Input::Help) => {
                            writeln!(notice, "{}", input::help(world))?;
                            continue 'scene;
                        }
                        Ok(input::Input::Blank) => continue 'scene,
                        Ok(input::Input::MapZoom { zoom, place }) => {
                            zoomed_map(&engine, zoom, &place, &mut notice)?;
                            continue 'scene;
                        }
                        Ok(request @ (input::Input::Save | input::Input::Load(_))) => {
                            persist(&mut engine, saves, &mut log, request, &mut notice)?;
                            leave_dialogue = false;
                            continue 'scene;
                        }
                        Err(message) => {
                            writeln!(notice, "Invalid command: {message}.")?;
                            continue 'scene;
                        }
                    }
                }
            };
            open = menu.stays_open(&command);
            // Stepping back from a conversation lasts until the player speaks again.
            leave_dialogue &= !matches!(command, Command::Talk(_) | Command::ChooseDialogue(_));
            // A restored save may be mid-conversation; show its choices again.
            if apply(&mut engine, saves, &mut log, command, &mut pending)? {
                leave_dialogue = false;
            }
            continue 'scene;
        }
    }
    // Quitting mid-fight still returns the terminal to its normal screen.
    match screen.take() {
        Some(fight) => fight.leave(output, &pending)?,
        None => {
            output.write_all(&notice)?;
            output.write_all(&pending)?;
        }
    }
    writeln!(output)?;
    Ok(())
}

/// Shows what the last command printed: on the normal screen, or as the
/// fight screen's latest turn. Starting a fight enters the fight screen;
/// ending one (or dying) leaves it with the final turn. A notice is shown
/// once, and in a fight never counts as a turn.
fn settle(
    engine: &Engine<'_>,
    screen: &mut Option<FightScreen>,
    notice: &mut Vec<u8>,
    pending: &mut Vec<u8>,
    output: &mut impl Write,
) -> io::Result<()> {
    let text = String::from_utf8_lossy(notice).into_owned();
    notice.clear();
    match (screen.take(), fight::active(engine)) {
        (None, false) => {
            output.write_all(text.as_bytes())?;
            output.write_all(pending)?;
        }
        (None, true) => {
            output.write_all(text.as_bytes())?;
            let mut fight = FightScreen::enter(output)?;
            fight.record(pending);
            *screen = Some(fight);
        }
        (Some(mut fight), true) => {
            fight.record(pending);
            fight.notice = text;
            *screen = Some(fight);
        }
        (Some(fight), false) => {
            fight.leave(output, pending)?;
            output.write_all(text.as_bytes())?;
        }
    }
    pending.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saves::Kind;
    use realmkit_engine::Engine;
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
        play_keys(
            &world,
            None,
            None,
            Paint::default(),
            keys.into_iter().map(Ok),
            &mut output,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        for passage in [
            "> 1. Talk to Elder Mara",
            "Esc back",
            ": status",
            "HP 40/40",
            "Level 2",
            "She opens the chapel gate",
            "Your journey through the demo is complete.",
        ] {
            assert!(text.contains(passage), "missing {passage:?} in {text}");
        }
        // The fight has the alternate screen to itself: bars, the next
        // turns, and the latest turn marked.
        let (before, fight) = text.split_once("\u{1b}[?1049h").expect(&text);
        let (fight, after) = fight.split_once("\u{1b}[?1049l").expect(&text);
        assert!(before.ends_with("> Engage The Ash Wolf\n"), "{before}");
        for passage in ["The Ash Wolf  ██████████", "Next: You", "› "] {
            assert!(fight.contains(passage), "missing {passage:?} in {fight}");
        }
        // Scrollback keeps the opening line and the final turn only.
        assert!(after.starts_with("You face The Ash Wolf.\n"), "{after}");
        assert!(after.contains("Level 2"), "{after}");
        assert_eq!(after.matches("damage").count(), 1, "{after}");
    }

    #[test]
    fn start_questions_are_answered_by_keys_before_play() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/quiet-archive"
        ))
        .unwrap();
        let mut output = Vec::new();
        // Letters do nothing here; the arrows move to the second answer.
        let keys = [Char('i'), Down, Enter, Char('c'), Quit];
        play_keys(
            &world,
            None,
            None,
            Paint::default(),
            keys.into_iter().map(Ok),
            &mut output,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains(
            "What brings you to the archive?\n> I was a copyist's apprentice, and I keep my own pen.\n"
        ));
        assert!(text.contains("  Errand: I was a copyist's apprentice"));
        // Quitting at the question ends play without starting it.
        let mut output = Vec::new();
        let keys = [Quit];
        play_keys(
            &world,
            None,
            None,
            Paint::default(),
            keys.into_iter().map(Ok),
            &mut output,
        )
        .unwrap();
        assert!(!String::from_utf8(output)
            .unwrap()
            .contains("The Reading Room"));
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
        play_keys(
            &world,
            None,
            None,
            Paint::default(),
            keys.into_iter().map(Ok),
            &mut output,
        )
        .unwrap();
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
        let error = play_keys(
            &world,
            None,
            None,
            Paint::default(),
            keys.into_iter(),
            &mut Vec::new(),
        )
        .unwrap_err();
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
    fn a_level_up_mentions_mp_from_a_technique() {
        // No MP in the level table: only Azure Breath's First Layer gives any.
        let mut world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect")).unwrap();
        let combat = world.world.combat.as_mut().unwrap();
        combat.levels.iter_mut().for_each(|l| l.stats.mp = 0);
        combat.skills.iter_mut().for_each(|s| s.cost = 0);
        world
            .characters
            .iter_mut()
            .find(|c| c.id == "dummy")
            .unwrap()
            .combat
            .as_mut()
            .unwrap()
            .xp = 20;
        let mut output = Vec::new();
        let input = format!("north\nengage dummy\n{}", "attack dummy\n".repeat(8));
        play(&world, None, None, input.as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Level 2! Health and MP restored."), "{text}");
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

    #[test]
    fn technique_xp_from_a_fight_is_one_line_even_after_fleeing() {
        let world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect")).unwrap();
        let mut output = Vec::new();
        let input = "talk qing\nchoose 1\nnorth\nengage dummy\nuse palm_drifting dummy\nflee\n";
        play(&world, None, None, input.as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("You get away.\nTechnique XP: Cloud Palm +10\n"),
            "{text}"
        );
        assert!(!text.contains("\nCloud Palm +10"), "{text}");
    }

    #[test]
    fn loading_a_save_mid_fight_forgets_the_rolled_back_technique_xp() {
        let world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect")).unwrap();
        let dir = std::env::temp_dir().join(format!("realmkit-tally-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let saves = Saves::open(&dir).unwrap();
        let mut output = Vec::new();
        let input = "talk qing\nchoose 1\nnorth\nengage dummy\nsave\nuse palm_drifting dummy\nload 2\nflee\n";
        play(&world, Some(&saves), None, input.as_bytes(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let fled = text.find("You get away.").expect(&text);
        assert!(!text[fled..].contains("Technique XP"), "{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_terminal_gets_emphasis_and_plain_play_gets_none() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let play = |paint| {
            let mut output = Vec::new();
            let keys = [Enter, Char(':'), Char('c'), Enter, Quit];
            play_keys(
                &world,
                None,
                None,
                paint,
                keys.into_iter().map(Ok),
                &mut output,
            )
            .unwrap();
            String::from_utf8(output).unwrap()
        };
        let styled = play(Paint { styled: true });
        // The location name is bold, and the key hints are dimmed.
        assert!(
            styled.contains("\u{1b}[1mAshbell Village\u{1b}[0m"),
            "{styled:?}"
        );
        assert!(styled.contains("\u{1b}[2m↑/↓ select"), "{styled:?}");
        // Plain play keeps only the cursor movement that redraws the menu.
        let plain = play(Paint::default());
        assert!(
            !plain.contains("\u{1b}[1m") && !plain.contains("\u{1b}[2m"),
            "{plain:?}"
        );
    }

    #[test]
    fn a_typo_in_a_fight_is_a_notice_not_a_turn() {
        let world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap();
        // East to the rat warren, then engage the rat.
        let mut keys = vec![Char('e'), Char(':')];
        keys.extend("engage rat".chars().map(Char));
        keys.extend([Enter, Char(':'), Char('z'), Enter, Quit]);
        let mut output = Vec::new();
        let (world, keys) = (&world, keys.into_iter().map(Ok));
        play_keys(world, None, Some(1), Paint::default(), keys, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let frames: Vec<_> = text.split("\u{1b}[2J").collect();
        let last = frames.last().unwrap();
        assert!(last.contains("Invalid command"), "{last}");
        assert!(!last.contains("› Invalid"), "{last}");
        assert!(last.contains("› You face A Warren Rat."), "{last}");
    }
}
