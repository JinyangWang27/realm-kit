use crate::{
    input,
    render::Paint,
    render::{direction_name, gear_name, stat_name},
};
use realmkit_engine::{Command, Engine};
use realmkit_spec::{Resource, Stat};
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
const TRAIN: &str = "Train";
const RESPEC: &str = "Refund stat points";
const TRAIN_GROUP: &str = "Train stats";
const EQUIPMENT_GROUP: &str = "Equipment";
const OPENS: &str = "›";
const BACK: &str = "Back";
const EQUIP: &str = "Equip";
const YIELDED: &str = "yielded";
const INVENTORY: &str = "Inventory";
const CHARACTER: &str = "Character";
const QUESTS: &str = "Quests";
const TECHNIQUES: &str = "Techniques";
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
    /// Step back from the conversation to the location's actions.
    Back,
    Typed,
    Help,
}

/// Actions gathered behind one entry, so the main menu stays short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Train,
    Equipment,
}

impl Group {
    fn of(command: &Command) -> Option<Self> {
        match command {
            Command::Allocate { .. } | Command::Respec => Some(Self::Train),
            Command::Equip(_) => Some(Self::Equipment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Run(Command),
    Open(Group),
    Back,
}

pub struct Entry {
    pub pick: Pick,
    pub label: String,
}

/// A menu keeps the entries it was built with, so numbers always refer to what
/// the player saw; the engine rechecks legality when a command runs.
pub struct Menu {
    top: Vec<Entry>,
    groups: Vec<(Group, Vec<Entry>)>,
    /// The submenu being shown, if any.
    pub open: Option<Group>,
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

/// The location's or fight's actions: grouped ones go into submenus, each
/// entered where its first action would have been.
fn actions(engine: &Engine<'_>) -> (Vec<Entry>, Vec<(Group, Vec<Entry>)>) {
    let mut top = Vec::new();
    let mut groups: Vec<(Group, Vec<Entry>)> = Vec::new();
    for action in engine.actions() {
        let Some(label) = label(engine, &action) else {
            continue;
        };
        let entry = Entry {
            pick: Pick::Run(action.command.clone()),
            label,
        };
        let Some(group) = Group::of(&action.command) else {
            top.push(entry);
            continue;
        };
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, entries)) => entries.push(entry),
            None => {
                top.push(Entry {
                    pick: Pick::Open(group),
                    label: group_label(engine, group),
                });
                groups.push((group, vec![entry]));
            }
        }
    }
    for (_, entries) in &mut groups {
        entries.push(Entry {
            pick: Pick::Back,
            label: BACK.into(),
        });
    }
    (top, groups)
}

/// "Train stats — 3 points ›", "Equipment ›".
fn group_label(engine: &Engine<'_>, group: Group) -> String {
    match group {
        Group::Train => match engine.unspent_points().unwrap_or(0) {
            0 => format!("{TRAIN_GROUP} {OPENS}"),
            1 => format!("{TRAIN_GROUP} — 1 point {OPENS}"),
            n => format!("{TRAIN_GROUP} — {n} points {OPENS}"),
        },
        Group::Equipment => format!("{EQUIPMENT_GROUP} {OPENS}"),
    }
}

/// What the player sees for an action; `None` hides it.
fn label(engine: &Engine<'_>, action: &realmkit_engine::Action) -> Option<String> {
    let world = engine.world();
    let here = world.location(&engine.state().player.location).unwrap();
    Some(match &action.command {
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
            // A technique's skill carries the name of the rank it belongs to.
            let rank = engine.state().combat.as_ref().and_then(|c| {
                c.techniques.iter().find_map(|(id, learned)| {
                    let rank = &world.technique(id)?.ranks[learned.rank - 1];
                    (rank.skill.as_ref() == Some(&skill.id)).then_some(&rank.name)
                })
            });
            let name = match rank {
                Some(rank) => format!("{} · {rank}", skill.name),
                None => skill.name.clone(),
            };
            let resource = match skill.resource {
                Resource::Mp => MP,
                Resource::Rage => RAGE,
            };
            format!(
                "{} {ON} {}{}{}",
                name,
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
        // "Train Attack: 12 → 13", from the same effective stats the engine uses.
        Command::Allocate { stat, .. } => {
            let now = engine.player_stats().unwrap().get(*stat);
            let value = world.combat().unwrap().stat_points.as_ref().unwrap().values[stat];
            format!(
                "{TRAIN} {}: {now} → {}",
                stat_name(world, *stat),
                now + value
            )
        }
        Command::Respec => RESPEC.into(),
        // "Equip #5 Greatsword: Attack 13 → 21, Defence 11 → 9", from
        // the engine's own calculation on a copy.
        Command::Equip(piece) => {
            let now = engine.player_stats().unwrap();
            let mut probe = engine.clone();
            probe.execute(Command::Equip(*piece)).ok()?;
            let then = probe.player_stats().unwrap();
            let changes: Vec<String> = Stat::ALL
                .into_iter()
                .filter(|s| now.get(*s) != then.get(*s))
                .map(|s| format!("{} {} → {}", stat_name(world, s), now.get(s), then.get(s)))
                .collect();
            let name = gear_name(engine, *piece);
            if changes.is_empty() {
                format!("{EQUIP} {name}")
            } else {
                format!("{EQUIP} {name}: {}", changes.join(", "))
            }
        }
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
        Command::Techniques => TECHNIQUES.into(),
        _ => return None,
    })
}

impl Menu {
    /// Dialogue choices take focus unless the player stepped back with Esc.
    /// `open` keeps a submenu open across commands while it has entries.
    pub fn new(engine: &Engine<'_>, leave_dialogue: bool, open: Option<Group>) -> Self {
        let choices = engine.dialogue_choices();
        let dialogue = !leave_dialogue && !choices.is_empty();
        let (top, groups) = if dialogue {
            let top = choices
                .into_iter()
                .enumerate()
                .map(|(i, text)| Entry {
                    pick: Pick::Run(Command::ChooseDialogue(i + 1)),
                    label: text.into(),
                })
                .collect();
            (top, Vec::new())
        } else {
            actions(engine)
        };
        let open = open.filter(|g| groups.iter().any(|(group, _)| group == g));
        Self {
            top,
            groups,
            open,
            cursor: 0,
            dialogue,
            header: encounter_lines(engine),
        }
    }

    /// The submenu to reopen after `command`: the open one, if the command came from it.
    pub fn stays_open(&self, command: &Command) -> Option<Group> {
        self.open.filter(|g| Group::of(command) == Some(*g))
    }

    /// The entries on screen: the open submenu's, or the main menu's.
    pub fn entries(&self) -> &[Entry] {
        self.groups
            .iter()
            .find(|(group, _)| Some(*group) == self.open)
            .map_or(&self.top, |(_, entries)| entries)
    }

    /// Drops the fight's vitals and turn order, for a screen that shows its own.
    pub fn without_header(mut self) -> Self {
        self.header.clear();
        self
    }

    /// One-based selection, as shown to the player.
    pub fn select(&self, number: usize) -> Option<&Entry> {
        number.checked_sub(1).and_then(|i| self.entries().get(i))
    }

    /// Runs a command, or opens or leaves a submenu.
    pub fn choose(&mut self, number: usize) -> Outcome {
        let Some(entry) = self.select(number) else {
            return Outcome::Ignore;
        };
        match entry.pick.clone() {
            Pick::Run(command) => return Outcome::Run(command),
            Pick::Open(group) => self.open = Some(group),
            Pick::Back => self.open = None,
        }
        self.cursor = 0;
        Outcome::Redraw
    }

    /// Writes the menu and returns how many lines it occupies.
    // ponytail: assumes labels fit one terminal row; measure widths if long labels wrap.
    pub fn write(
        &self,
        output: &mut impl Write,
        interactive: bool,
        paint: Paint,
    ) -> io::Result<u16> {
        writeln!(output)?;
        let mut header = self.header.clone();
        // A submenu is headed by the entry that opened it: "Train stats — 1 point".
        let opener = self
            .top
            .iter()
            .find(|e| Some(&e.pick) == self.open.map(Pick::Open).as_ref());
        if let Some(entry) = opener {
            let title = entry.label.trim_end_matches(OPENS).trim_end();
            header.push(paint.title(title));
        }
        for line in &header {
            writeln!(output, "{line}")?;
        }
        for (i, entry) in self.entries().iter().enumerate() {
            let line = format!("{}. {}", i + 1, entry.label);
            if interactive && i == self.cursor {
                writeln!(output, "> {}", paint.title(&line))?;
            } else {
                writeln!(output, "  {line}")?;
            }
        }
        if interactive {
            let esc = if self.dialogue || self.open.is_some() {
                ESC_HINT
            } else {
                ""
            };
            writeln!(output, "\n{}", paint.dim(&format!("{KEYS_HINT}{esc}")))?;
            Ok((self.entries().len() + header.len()) as u16 + 3)
        } else {
            Ok((self.entries().len() + header.len()) as u16 + 1)
        }
    }

    pub fn handle(&mut self, key: Key) -> Outcome {
        let last = self.entries().len().saturating_sub(1);
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
            Key::Enter => return self.choose(self.cursor + 1),
            Key::Esc if self.open.is_some() => {
                self.open = None;
                self.cursor = 0;
            }
            Key::Esc if self.dialogue => return Outcome::Back,
            Key::Char(':') => return Outcome::Typed,
            // ponytail: digits 1–9 choose instantly; longer menus need arrows or `:`.
            Key::Char(c @ '1'..='9') => return self.choose(c as usize - '0' as usize),
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
        let menu = Menu::new(&engine, false, None);
        let labels: Vec<_> = menu.entries().iter().map(|e| e.label.as_str()).collect();
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
        let mut menu = Menu::new(&engine, false, None);
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
        let mut menu = Menu::new(&engine, false, None);
        assert_eq!(menu.entries()[0].label, "What troubles the village?");
        assert_eq!(menu.handle(Key::Char('n')), Outcome::Ignore);
        assert_eq!(
            menu.handle(Key::Char('2')),
            Outcome::Run(Command::ChooseDialogue(2))
        );
        assert_eq!(menu.handle(Key::Esc), Outcome::Back);
        assert_eq!(
            Menu::new(&engine, true, None).entries()[0].label,
            "Talk to Elder Mara"
        );
    }

    #[test]
    fn training_waits_behind_a_submenu_that_stays_open_while_it_is_used() {
        let world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap();
        let mut engine = Engine::new(&world).unwrap();
        let mut menu = Menu::new(&engine, false, None);
        let labels = |menu: &Menu| -> Vec<String> {
            menu.entries().iter().map(|e| e.label.clone()).collect()
        };
        assert_eq!(
            labels(&menu)[4..],
            [
                "Rest",
                "Train stats — 3 points ›",
                "Inventory",
                "Character",
                "Quests"
            ]
        );
        assert_eq!(menu.handle(Key::Char('6')), Outcome::Redraw);
        assert_eq!(
            labels(&menu),
            [
                "Train HP: 60 → 65",
                "Train Attack: 14 → 15",
                "Train Defence: 12 → 13",
                "Train Speed: 100 → 102",
                "Back"
            ]
        );
        // Esc and Back both return to the main menu.
        assert_eq!(menu.handle(Key::Esc), Outcome::Redraw);
        assert_eq!(menu.open, None);
        menu.choose(6);
        assert_eq!(menu.choose(5), Outcome::Redraw);
        assert_eq!(menu.open, None);

        menu.choose(6);
        let Outcome::Run(train) = menu.handle(Key::Enter) else {
            panic!("the first entry trains HP");
        };
        engine.execute(train.clone()).unwrap();
        let menu = Menu::new(&engine, false, menu.stays_open(&train));
        assert_eq!(labels(&menu)[0], "Train HP: 65 → 70");
        // A command from elsewhere closes it.
        assert_eq!(menu.stays_open(&Command::Rest), None);
    }
}
