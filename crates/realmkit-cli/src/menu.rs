use crate::{input, render::direction_name};
use realmkit_engine::{Command, Engine};
use realmkit_spec::Resource;
use std::io::{self, Write};

// Fixed interface words live here, apart from authored world text, so a locale
// table can replace them later without touching menu logic.
const TALK: &str = "Talk to";
const ATTACK: &str = "Attack";
const TRAVEL: &str = "Travel";
const LOCKED: &str = "[locked]";
const ON: &str = "on";
const ENGAGE: &str = "Engage";
const MP: &str = "MP";
const RAGE: &str = "rage";
const REST: &str = "Rest";
const FLEE: &str = "Flee";
const YIELDED: &str = "yielded";
const INVENTORY: &str = "Inventory";
const CHARACTER: &str = "Character";
const QUESTS: &str = "Quests";
const KEYS_HINT: &str = "↑/↓ select · Enter confirm · number choose · : command";
const ESC_HINT: &str = " · Esc back";
pub const LINE_HINT: &str = "Enter a number, or type help for commands.";
pub const NOT_LISTED: &str = "Choose one of the listed numbers.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Enter,
    Esc,
    Backspace,
    Char(char),
    Quit,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Ignore,
    Redraw,
    Run(Command),
    Back,
    Typed,
    Help,
}

pub struct Entry {
    pub command: Command,
    pub label: String,
}

/// A menu keeps the entries it was built with, so numbers always refer to what
/// the player saw; the engine rechecks legality when a command runs.
pub struct Menu {
    pub entries: Vec<Entry>,
    pub cursor: usize,
    dialogue: bool,
    /// In a fight: everyone's vitals and the projected turn order.
    header: Vec<String>,
}

/// The engine projects turns as if every action took a basic action's time.
const NEXT: &str = "Projected turns";
const TURNS_SHOWN: usize = 5;

/// "You HP 36/40 · rage 2 | The Ash Wolf HP 13/20" and the next few turns.
fn encounter_lines(engine: &Engine<'_>) -> Vec<String> {
    let Some(encounter) = engine.encounter() else {
        return Vec::new();
    };
    let world = engine.world();
    let name = |id: &str| world.character(id).unwrap().name.as_str();
    let resources = world.combat().unwrap().resources;
    let rage = resources.rage_per_action > 0 || resources.rage_per_max_hp > 0;
    let vitals: Vec<_> = encounter
        .participants
        .iter()
        .map(|p| {
            let max = match &world.character(&p.character).unwrap().combat {
                Some(profile) => profile.stats,
                None => engine.player_stats().unwrap(),
            };
            let mut line = format!("{} HP {}/{}", name(&p.character), p.hp, max.hp);
            if max.mp > 0 {
                line += &format!(" · {MP} {}/{}", p.mp, max.mp);
            }
            if rage {
                line += &format!(" · {RAGE} {}", p.rage);
            }
            if p.yielded {
                line += &format!(" · {YIELDED}");
            }
            line
        })
        .collect();
    let order: Vec<_> = engine
        .turn_order(TURNS_SHOWN)
        .iter()
        .map(|id| name(id))
        .collect();
    vec![vitals.join(" | "), format!("{NEXT}: {}", order.join(", "))]
}

impl Menu {
    /// Dialogue choices take focus unless the player stepped back with Esc.
    pub fn new(engine: &Engine<'_>, leave_dialogue: bool) -> Self {
        let choices = engine.dialogue_choices();
        let dialogue = !leave_dialogue && !choices.is_empty();
        let entries = if dialogue {
            choices
                .into_iter()
                .enumerate()
                .map(|(i, text)| Entry {
                    command: Command::ChooseDialogue(i + 1),
                    label: text.into(),
                })
                .collect()
        } else {
            let world = engine.world();
            let here = world.location(&engine.state().player.location).unwrap();
            engine
                .actions()
                .into_iter()
                .filter_map(|action| {
                    let label = match &action.command {
                        Command::Move(direction) => format!(
                            "{TRAVEL} {} — {}{}",
                            direction_name(*direction),
                            world
                                .location(&here.exits[direction].destination)
                                .unwrap()
                                .name,
                            if action.available {
                                String::new()
                            } else {
                                format!(" {LOCKED}")
                            }
                        ),
                        // Like a locked exit, choosing it explains why it cannot be used.
                        Command::UseSkill { skill, target } => {
                            let skill = world.skill(skill).unwrap();
                            let resource = match skill.resource {
                                Resource::Mp => MP,
                                Resource::Rage => RAGE,
                            };
                            format!(
                                "{} {ON} {}{}{}",
                                skill.name,
                                world.character(target).unwrap().name,
                                if skill.cost > 0 {
                                    format!(" — {} {resource}", skill.cost)
                                } else {
                                    String::new()
                                },
                                if action.available {
                                    String::new()
                                } else {
                                    format!(" [not enough {resource}]")
                                }
                            )
                        }
                        // Other unavailable actions (e.g. after death) are not offered.
                        _ if !action.available => return None,
                        Command::Rest => REST.into(),
                        Command::Flee => FLEE.into(),
                        Command::Engage(id) => {
                            format!("{ENGAGE} {}", world.character(id).unwrap().name)
                        }
                        Command::Talk(id) => {
                            format!("{TALK} {}", world.character(id).unwrap().name)
                        }
                        Command::Attack(id) => {
                            format!("{ATTACK} {}", world.character(id).unwrap().name)
                        }
                        Command::Inventory => INVENTORY.into(),
                        Command::Status => CHARACTER.into(),
                        Command::Quests => QUESTS.into(),
                        _ => return None,
                    };
                    Some(Entry {
                        command: action.command,
                        label,
                    })
                })
                .collect()
        };
        Self {
            entries,
            cursor: 0,
            dialogue,
            header: encounter_lines(engine),
        }
    }

    /// One-based selection, as shown to the player.
    pub fn select(&self, number: usize) -> Option<&Entry> {
        number.checked_sub(1).and_then(|i| self.entries.get(i))
    }

    /// Writes the menu and returns how many lines it occupies.
    // ponytail: assumes labels fit one terminal row; measure widths if long labels wrap.
    pub fn write(&self, output: &mut impl Write, interactive: bool) -> io::Result<u16> {
        writeln!(output)?;
        for line in &self.header {
            writeln!(output, "{line}")?;
        }
        for (i, entry) in self.entries.iter().enumerate() {
            let marker = if interactive && i == self.cursor {
                ">"
            } else {
                " "
            };
            writeln!(output, "{marker} {}. {}", i + 1, entry.label)?;
        }
        if interactive {
            let esc = if self.dialogue { ESC_HINT } else { "" };
            writeln!(output, "\n{KEYS_HINT}{esc}")?;
            Ok((self.entries.len() + self.header.len()) as u16 + 3)
        } else {
            Ok((self.entries.len() + self.header.len()) as u16 + 1)
        }
    }

    pub fn handle(&mut self, key: Key) -> Outcome {
        let last = self.entries.len().saturating_sub(1);
        match key {
            Key::Up => {
                self.cursor = if self.cursor == 0 {
                    last
                } else {
                    self.cursor - 1
                }
            }
            Key::Down => {
                self.cursor = if self.cursor == last {
                    0
                } else {
                    self.cursor + 1
                }
            }
            Key::Enter => {
                return self
                    .select(self.cursor + 1)
                    .map_or(Outcome::Ignore, |e| Outcome::Run(e.command.clone()))
            }
            Key::Esc if self.dialogue => return Outcome::Back,
            Key::Char(':') => return Outcome::Typed,
            // ponytail: digits 1–9 choose instantly; longer menus need arrows or `:`.
            Key::Char(c @ '1'..='9') => {
                return self
                    .select(c as usize - '0' as usize)
                    .map_or(Outcome::Ignore, |e| Outcome::Run(e.command.clone()))
            }
            // Letter shortcuts (movement, panels) apply only outside dialogue focus.
            Key::Char(c) if !self.dialogue => {
                return match input::parse(&c.to_string()) {
                    Ok(input::Input::Command(command)) => Outcome::Run(command),
                    Ok(input::Input::Help) => Outcome::Help,
                    _ => Outcome::Ignore,
                }
            }
            _ => return Outcome::Ignore,
        }
        Outcome::Redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use realmkit_spec::{Direction, WorldSpec};

    fn demo() -> WorldSpec {
        WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap()
    }

    #[test]
    fn labels_use_authored_names_and_mark_locked_exits() {
        let world = demo();
        let engine = Engine::new(&world).unwrap();
        let labels: Vec<_> = Menu::new(&engine, false)
            .entries
            .into_iter()
            .map(|e| e.label)
            .collect();
        assert_eq!(
            labels,
            [
                "Talk to Elder Mara",
                "Travel north — The Pine Track",
                "Travel east — The Roofless Chapel [locked]",
                "Rest",
                "Inventory",
                "Character",
                "Quests"
            ]
        );
    }

    #[test]
    fn keys_move_the_cursor_choose_and_leave_dialogue() {
        let world = demo();
        let mut engine = Engine::new(&world).unwrap();
        let mut menu = Menu::new(&engine, false);
        assert_eq!(menu.handle(Key::Up), Outcome::Redraw);
        assert_eq!(menu.cursor, 6);
        assert_eq!(menu.handle(Key::Down), Outcome::Redraw);
        assert_eq!(menu.handle(Key::Down), Outcome::Redraw);
        assert_eq!(
            menu.handle(Key::Enter),
            Outcome::Run(Command::Move(Direction::North))
        );
        assert_eq!(menu.handle(Key::Char('9')), Outcome::Ignore);
        assert_eq!(menu.handle(Key::Esc), Outcome::Ignore);
        assert_eq!(menu.handle(Key::Char(':')), Outcome::Typed);
        assert_eq!(
            menu.handle(Key::Char('n')),
            Outcome::Run(Command::Move(Direction::North))
        );
        assert_eq!(menu.handle(Key::Char('c')), Outcome::Run(Command::Status));

        engine.execute(Command::Talk("elder".into())).unwrap();
        let mut menu = Menu::new(&engine, false);
        assert_eq!(menu.entries[0].label, "What troubles the village?");
        assert_eq!(menu.handle(Key::Char('n')), Outcome::Ignore);
        assert_eq!(
            menu.handle(Key::Char('2')),
            Outcome::Run(Command::ChooseDialogue(2))
        );
        assert_eq!(menu.handle(Key::Esc), Outcome::Back);
        assert_eq!(
            Menu::new(&engine, true).entries[0].label,
            "Talk to Elder Mara"
        );
    }
}
