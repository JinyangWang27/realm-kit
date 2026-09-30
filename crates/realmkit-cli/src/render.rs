use crate::panels;
use crossterm::style::Stylize;
use realmkit_engine::{Engine, Event, Outcome};
use realmkit_spec::{Direction, Id, Stat, TextTemplate};
use std::io::{self, Write};

const CRITICAL: &str = "Critical hit!";
const TECHNIQUE_XP: &str = "Technique XP";

/// Single-pass interpolation: inserted values are data, never template syntax.
fn interpolate(template: &TextTemplate, values: &[(&str, &str)]) -> io::Result<String> {
    let mut output = String::new();
    let mut rest = template.0.as_str();
    while let Some(open) = rest.find('{') {
        output.push_str(&rest[..open]);
        let tail = &rest[open + 1..];
        let close = tail.find('}').ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "unclosed template placeholder")
        })?;
        let value = values
            .iter()
            .find(|(key, _)| *key == &tail[..close])
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "unknown template placeholder")
            })?
            .1;
        output.push_str(value);
        rest = &tail[close + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn hit(
    output: &mut impl Write,
    text: &TextTemplate,
    attacker: &str,
    target: &str,
    amount: u32,
) -> io::Result<()> {
    let amount = amount.to_string();
    let values = [
        ("attacker", attacker),
        ("target", target),
        ("damage", &amount),
    ];
    writeln!(output, "{}", interpolate(text, &values)?)
}

/// "#3 Iron mail": a piece's instance number and its item's name.
pub fn gear_name(engine: &Engine<'_>, gear: u64) -> String {
    let item = engine
        .state()
        .combat
        .as_ref()
        .and_then(|c| c.gear.get(&gear))
        .and_then(|g| engine.world().item(&g.item));
    format!("#{gear} {}", item.map_or("?", |i| i.name.as_str()))
}

/// A stat's display name; special stats use the world's special name.
pub fn stat_name(world: &realmkit_spec::WorldSpec, stat: Stat) -> String {
    let special = world
        .combat()
        .map_or("Special", |c| c.special_name.as_str());
    match stat {
        Stat::Hp => "HP".into(),
        Stat::Mp => "MP".into(),
        Stat::Patk => "Attack".into(),
        Stat::Pdef => "Defence".into(),
        Stat::Satk => format!("{special} attack"),
        Stat::Sdef => format!("{special} defence"),
        Stat::Speed => "Speed".into(),
    }
}

pub fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::North => "north",
        Direction::South => "south",
        Direction::East => "east",
        Direction::West => "west",
        Direction::Up => "up",
        Direction::Down => "down",
    }
}

/// Emphasis for a terminal; plain text for pipes, line mode and `NO_COLOR`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Paint {
    pub styled: bool,
}

impl Paint {
    fn apply(
        self,
        text: &str,
        style: fn(String) -> crossterm::style::StyledContent<String>,
    ) -> String {
        if self.styled {
            style(text.into()).to_string()
        } else {
            text.into()
        }
    }
    /// Names and headings.
    pub fn title(self, text: &str) -> String {
        self.apply(text, |t| t.bold())
    }
    /// Growth: level-ups, new ranks and techniques.
    pub fn good(self, text: &str) -> String {
        self.apply(text, |t| t.green().bold())
    }
    pub fn critical(self, text: &str) -> String {
        self.apply(text, |t| t.yellow().bold())
    }
    pub fn bad(self, text: &str) -> String {
        self.apply(text, |t| t.red().bold())
    }
    /// Hints and other secondary text.
    pub fn dim(self, text: &str) -> String {
        self.apply(text, |t| t.dim())
    }
}

/// Remembers technique XP gained during a fight, which is shown as one line
/// when the fight ends instead of after every hit.
#[derive(Default)]
pub struct Log {
    technique_xp: Vec<(Id, u64)>,
    pub paint: Paint,
}

impl Log {
    pub fn new(paint: Paint) -> Self {
        Self {
            technique_xp: Vec::new(),
            paint,
        }
    }

    /// Forgets the tally, for when a save replaces the playthrough.
    pub fn reset(&mut self) {
        self.technique_xp.clear();
    }

    /// Renders one command's events, holding back technique XP until the
    /// fight ends. A fight the player dies in never ends; the next one
    /// starts a fresh tally.
    pub fn events(
        &mut self,
        output: &mut impl Write,
        engine: &Engine<'_>,
        batch: &[Event],
    ) -> io::Result<()> {
        let ended = batch
            .iter()
            .any(|e| matches!(e, Event::EncounterEnded { .. }));
        let fighting = ended || engine.encounter().is_some();
        let mut shown = Vec::new();
        for event in batch {
            match event {
                Event::EncounterStarted { .. } => {
                    self.technique_xp.clear();
                    shown.push(event.clone());
                }
                Event::TechniqueXpGained { technique, amount } if fighting => {
                    match self.technique_xp.iter_mut().find(|(t, _)| t == technique) {
                        Some((_, total)) => *total = total.saturating_add(*amount),
                        None => self.technique_xp.push((technique.clone(), *amount)),
                    }
                }
                _ => shown.push(event.clone()),
            }
        }
        events(output, engine, &shown, self.paint)?;
        if ended && !self.technique_xp.is_empty() {
            let world = engine.world();
            let gains: Vec<String> = self
                .technique_xp
                .drain(..)
                .map(|(id, amount)| format!("{} +{amount}", world.technique(&id).unwrap().name))
                .collect();
            writeln!(output, "{TECHNIQUE_XP}: {}", gains.join(", "))?;
        }
        Ok(())
    }
}

pub fn events(
    output: &mut impl Write,
    engine: &Engine<'_>,
    events: &[Event],
    paint: Paint,
) -> io::Result<()> {
    let world = engine.world();
    let name = |id: &str| &world.character(id).unwrap().name;
    let player = name(&world.world.player);
    for event in events {
        match event {
            Event::LocationViewed { location } => {
                panels::location(output, engine, location, paint)?
            }
            Event::DamageDealt {
                target,
                amount,
                variant,
                skill,
                critical,
            } => {
                if *critical {
                    writeln!(output, "{}", paint.critical(CRITICAL))?;
                }
                let narrative = &world.combat().unwrap().narrative;
                let text = skill.as_ref().map_or(&narrative.attack[*variant], |id| {
                    &world.skill(id).unwrap().text
                });
                hit(output, text, player, name(target), *amount)?;
            }
            Event::DamageReceived {
                source,
                amount,
                variant,
                skill,
                critical,
            } => {
                if *critical {
                    writeln!(output, "{}", paint.critical(CRITICAL))?;
                }
                let narrative = &world.combat().unwrap().narrative;
                let text = skill.as_ref().map_or(&narrative.hurt[*variant], |id| {
                    &world.skill(id).unwrap().text
                });
                hit(output, text, name(source), player, *amount)?;
            }
            // Costs are shown in the menu and remaining MP in the status line.
            Event::ResourceSpent { .. } => {}
            Event::PointsAllocated { stat, points } => {
                let plural = if *points == 1 { "point" } else { "points" };
                writeln!(
                    output,
                    "{points} {plural} into {}.",
                    stat_name(world, *stat)
                )?
            }
            Event::PointsRefunded => writeln!(output, "Your stat points are refunded.")?,
            Event::Equipped { gear } => {
                writeln!(output, "You equip {}.", gear_name(engine, *gear))?
            }
            Event::ItemsSpent { item, quantity } => writeln!(
                output,
                "Spent: {} ×{quantity}",
                world.item(item).unwrap().name
            )?,
            Event::Forged { gear, .. } => {
                writeln!(output, "You forge {}.", gear_name(engine, *gear))?
            }
            Event::Improved { gear, .. } => {
                writeln!(output, "It is now {}.", gear_name(engine, *gear))?
            }
            Event::Unequipped { gear } => writeln!(
                output,
                "{} goes back in your pack.",
                gear_name(engine, *gear)
            )?,
            Event::TechniqueLearned { technique } => {
                let name = &world.technique(technique).unwrap().name;
                writeln!(output, "{}", paint.good(&format!("You learn {name}.")))?
            }
            Event::TechniqueRankUp { technique, rank } => {
                let technique = world.technique(technique).unwrap();
                let name = &technique.ranks[rank - 1].name;
                writeln!(
                    output,
                    "{}",
                    paint.good(&format!("{}: {name}!", technique.name))
                )?
            }
            Event::TechniqueXpGained { technique, amount } => writeln!(
                output,
                "{} +{amount}",
                world.technique(technique).unwrap().name
            )?,
            Event::TechniquesViewed => panels::techniques(output, engine, paint)?,
            Event::EncounterStarted { opponents } => {
                let names: Vec<_> = opponents.iter().map(|id| name(id).as_str()).collect();
                writeln!(output, "You face {}.", names.join(", "))?
            }
            Event::FleeStarted => writeln!(output, "You turn to run.")?,
            Event::Yielded { character } if *character == world.world.player => {
                writeln!(output, "You yield.")?
            }
            Event::Yielded { character } => writeln!(output, "{} yields.", name(character))?,
            Event::EncounterEnded { outcome } => writeln!(
                output,
                "{}",
                match outcome {
                    Outcome::Victory => "The fight is over.",
                    Outcome::Yielded => "You lose the bout.",
                    Outcome::Fled => "You get away.",
                }
            )?,
            Event::Rested => {
                let mp = engine.player_stats().is_some_and(|s| s.mp > 0);
                let restored = if mp { "Health and MP" } else { "Health" };
                writeln!(output, "You rest. {restored} restored.")?
            }
            Event::EnemyDefeated { monster } => writeln!(
                output,
                "{}",
                interpolate(
                    &world.combat().unwrap().narrative.victory,
                    &[("target", name(monster))]
                )?
            )?,
            Event::PlayerDied => writeln!(
                output,
                "{}",
                paint.bad(&world.combat().unwrap().narrative.death)
            )?,
            Event::ItemReceived { item, quantity } => writeln!(
                output,
                "Received: {} ×{}",
                world.item(item).unwrap().name,
                quantity
            )?,
            // A reward without XP says nothing about XP.
            Event::ExperienceGranted { amount: 0 } => {}
            Event::ExperienceGranted { amount } => writeln!(output, "+{amount} XP")?,
            Event::LevelUp { level, mp_restored } => {
                let restored = if *mp_restored {
                    "Health and MP"
                } else {
                    "Health"
                };
                let line = format!("Level {level}! {restored} restored.");
                writeln!(output, "{}", paint.good(&line))?
            }
            // Choices are shown by the menu, which also numbers them.
            Event::Dialogue { npc, node, .. } => {
                let npc = world.character(npc).unwrap();
                let dialogue = world.dialogue(npc.dialogue.as_ref().unwrap()).unwrap();
                let node = dialogue.nodes.iter().find(|n| &n.id == node).unwrap();
                writeln!(output, "{}: {}", npc.name, node.text)?;
            }
            Event::QuestAccepted { quest } => {
                writeln!(output, "{}", world.quest(quest).unwrap().introduction)?
            }
            Event::QuestProgressed { quest } => {
                writeln!(output, "{}", world.quest(quest).unwrap().progress)?
            }
            Event::QuestCompleted { quest } => {
                writeln!(output, "{}", world.quest(quest).unwrap().completion)?
            }
            Event::InventoryViewed => panels::inventory(output, engine, paint)?,
            Event::StatusViewed => panels::status(output, engine, paint)?,
            Event::QuestsViewed => panels::quests(output, engine, paint)?,
            Event::Moved { .. } | Event::DialogueEnded | Event::StoryFlagSet { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_preserves_unicode_and_does_not_reinterpret_values() {
        let template = TextTemplate("{attacker}刺向{target}，造成{damage}点伤害。".into());
        assert_eq!(
            interpolate(
                &template,
                &[
                    ("attacker", "你{damage}"),
                    ("target", "山贼"),
                    ("damage", "27")
                ]
            )
            .unwrap(),
            "你{damage}刺向山贼，造成27点伤害。"
        );
    }
}
