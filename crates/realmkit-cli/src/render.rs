use crate::panels;
use crossterm::style::Stylize;
use realmkit_engine::{BattleOutcome, Engine, Event, Outcome};
use realmkit_spec::{CharacterKind, Direction, Id, Stat, TextTemplate};
use std::io::{self, Write};

const CRITICAL: &str = "Critical hit!";
const TECHNIQUE_XP: &str = "Technique XP";
const EVIDENCE: &str = "New evidence";
const REINTERPRETED: &str = "Understanding changed";

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

/// A workshop kind's authored name.
pub fn workshop_name<'w>(world: &'w realmkit_spec::WorldSpec, kind: &str) -> &'w str {
    world
        .economy()
        .and_then(|e| e.workshop(kind))
        .map_or("?", |k| k.name.as_str())
}

/// A piece's name at its tier and with its enchantment: "Iron sword", or
/// "Fine Iron sword of Keenness" from the authored templates.
pub fn piece_name(world: &realmkit_spec::WorldSpec, gear: &realmkit_engine::Gear) -> String {
    let Some(item) = world.item(&gear.item) else {
        return "?".into();
    };
    let tier = gear
        .tier
        .checked_sub(1)
        .and_then(|i| item.equipment.as_ref()?.tiers.get(i));
    let name = match tier {
        Some(tier) => interpolate(&tier.name, &[("item", &item.name)]).unwrap_or(item.name.clone()),
        None => item.name.clone(),
    };
    // The enchantment names the piece as it is at its tier.
    match gear.enchantment.as_ref().and_then(|e| world.enchantment(e)) {
        Some(enchantment) => interpolate(&enchantment.name, &[("item", &name)]).unwrap_or(name),
        None => name,
    }
}

/// "#3 Fine Iron mail": a piece's instance number and its name at its tier.
pub fn gear_name(engine: &Engine<'_>, gear: u64) -> String {
    let piece = engine
        .state()
        .combat
        .as_ref()
        .and_then(|c| c.gear.get(&gear));
    let name = piece.map_or("?".into(), |g| piece_name(engine.world(), g));
    format!("#{gear} {name}")
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

/// A soldier's name at a line's 1-based level, as the line authors it.
pub fn soldier(world: &realmkit_spec::WorldSpec, line: &str, level: usize) -> String {
    world
        .troops()
        .and_then(|t| t.line(line))
        .map_or_else(|| line.to_string(), |l| l.name_at(level).to_string())
}

/// An amount in the world's currency, such as "120 silver".
pub fn money(world: &realmkit_spec::WorldSpec, amount: u64) -> String {
    let amount = amount.to_string();
    match world.economy() {
        Some(economy) => {
            interpolate(&economy.currency.format, &[("amount", &amount)]).unwrap_or(amount)
        }
        None => amount,
    }
}

/// "45 min", "1 h 30 min", "2 d 4 h".
pub fn duration(minutes: u64) -> String {
    let (days, hours, mins) = (minutes / 1_440, minutes % 1_440 / 60, minutes % 60);
    let parts: Vec<String> = [(days, "d"), (hours, "h"), (mins, "min")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, unit)| format!("{n} {unit}"))
        .collect();
    if parts.is_empty() {
        "0 min".into()
    } else {
        parts.join(" ")
    }
}

/// The world's clock at `minute`, from its authored template.
pub fn clock(world: &realmkit_spec::WorldSpec, minute: u64) -> Option<String> {
    let time = world.world.time.as_ref()?;
    let values = realmkit_spec::clock_values(minute);
    let values: Vec<(&str, &str)> = values.iter().map(|(k, v)| (*k, v.as_str())).collect();
    interpolate(&time.clock, &values).ok()
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
    // After travel the location shows the clock, so the time is repeated
    // only to date something that happened on the way.
    let travelled = events.iter().any(|e| matches!(e, Event::Moved { .. }));
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
            Event::ProficiencyTrained { proficiency, rank }
            | Event::ProficiencyRaised { proficiency, rank } => writeln!(
                output,
                "{}",
                paint.good(&format!(
                    "{} is now rank {rank}.",
                    world.proficiency_name(*proficiency).unwrap_or("?")
                ))
            )?,
            Event::WorkshopBought {
                workshop,
                location,
                cost,
            } => writeln!(
                output,
                "You buy the {} in {} for {}.",
                workshop_name(world, workshop),
                world.location(location).unwrap().name,
                money(world, *cost)
            )?,
            Event::WorkshopSold {
                workshop,
                location,
                earned,
            } => writeln!(
                output,
                "You sell the {} in {} for {}.",
                workshop_name(world, workshop),
                world.location(location).unwrap().name,
                money(world, *earned)
            )?,
            Event::WorkshopsEarned { amount, forgone } => {
                let mut line = format!("Your workshops earn {}.", money(world, *amount));
                if *forgone > 0 {
                    line += &format!(
                        " {} more is beyond what you can hold.",
                        money(world, *forgone)
                    );
                }
                writeln!(output, "{}", paint.good(&line))?
            }
            Event::WorkshopsLost { amount, shortfall } => {
                let mut line = format!("Your workshops lose {}.", money(world, *amount));
                if *shortfall > 0 {
                    line += &format!(" You could not cover {}.", money(world, *shortfall));
                }
                writeln!(output, "{}", paint.bad(&line))?
            }
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
            Event::Enchanted { gear, .. } => {
                writeln!(output, "It is now {}.", gear_name(engine, *gear))?
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
            Event::Recruited {
                line,
                quantity,
                cost,
            } => writeln!(
                output,
                "Recruited: {} ×{quantity} for {}",
                soldier(world, line, 1),
                money(world, *cost)
            )?,
            Event::Upgraded {
                line, to, quantity, ..
            } => {
                let from = world.troops().unwrap().line(line).unwrap().levels.len();
                writeln!(
                    output,
                    "{} ×{quantity} become {}.",
                    soldier(world, line, from),
                    soldier(world, to, 1)
                )?
            }
            Event::Promoted { line, level, count } => writeln!(
                output,
                "{}",
                paint.good(&format!(
                    "Promoted: {} ×{count} (level {level}).",
                    soldier(world, line, *level)
                ))
            )?,
            Event::WagesPaid { amount } => {
                writeln!(output, "Wages paid: {}", money(world, *amount))?
            }
            Event::Deserted { line, level, count } => writeln!(
                output,
                "{}",
                paint.bad(&format!(
                    "Unpaid, {} ×{count} desert.",
                    soldier(world, line, *level)
                ))
            )?,
            Event::Recovered { line, level, count } => writeln!(
                output,
                "Recovered: {} ×{count}",
                soldier(world, line, *level)
            )?,
            Event::RetinueViewed => panels::retinue(output, engine, paint)?,
            Event::BattleStarted { army, allies } => {
                writeln!(
                    output,
                    "{}",
                    paint.title(&format!("Battle: {}", name(army)))
                )?;
                for ally in allies {
                    writeln!(output, "{} joins your side.", name(ally))?;
                }
            }
            Event::BattleRound {
                round,
                strengths,
                losses,
                morale,
            } => {
                let side = |s: usize| {
                    let mut line = format!("strength {}, lost {}", strengths[s], losses[s]);
                    if world.battle().is_some_and(|b| b.morale.is_some()) {
                        line += &format!(", morale {}", morale[s]);
                    }
                    line
                };
                writeln!(
                    output,
                    "Round {round} — yours: {} · theirs: {}",
                    side(0),
                    side(1)
                )?
            }
            // A pursuit that catches nobody is not worth a line.
            Event::Pursuit { losses: 0, .. } => {}
            Event::Pursuit { by: 0, losses } => {
                writeln!(output, "You run down the fleeing enemy: {losses} fall.")?
            }
            Event::Pursuit { losses, .. } => writeln!(
                output,
                "{}",
                paint.bad(&format!("The enemy cuts down {losses} as you fall back."))
            )?,
            Event::BattleEnded {
                outcome,
                wounded,
                killed,
            } => {
                let verdict = match outcome {
                    BattleOutcome::Victory => paint.good("Victory."),
                    BattleOutcome::Defeat => paint.bad("Defeat."),
                    BattleOutcome::Draw => "Both sides fall back: a draw.".into(),
                };
                writeln!(output, "{verdict}")?;
                let list = |stacks: &[(String, usize, u64)]| -> String {
                    stacks
                        .iter()
                        .map(|(line, level, n)| format!("{} ×{n}", soldier(world, line, *level)))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                if !wounded.is_empty() {
                    writeln!(output, "Wounded: {}", list(wounded))?;
                }
                if !killed.is_empty() {
                    writeln!(output, "Killed: {}", list(killed))?;
                }
            }
            Event::Consumed { item, hp, mp } => {
                let mut gains = Vec::new();
                if *hp > 0 {
                    gains.push(format!("+{hp} HP"));
                }
                if *mp > 0 {
                    gains.push(format!("+{mp} MP"));
                }
                writeln!(
                    output,
                    "You use {}: {}.",
                    world.item(item).unwrap().name,
                    gains.join(", ")
                )?
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
                // A feature does not speak: its lines describe what is seen.
                match npc.kind {
                    CharacterKind::Person => writeln!(output, "{}: {}", npc.name, node.text)?,
                    CharacterKind::Feature => writeln!(output, "{}", node.text)?,
                }
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
            Event::EvidenceDiscovered { evidence } => writeln!(
                output,
                "{}: {}",
                paint.good(EVIDENCE),
                world.evidence(evidence).unwrap().name
            )?,
            Event::EvidenceReinterpreted { evidence, reading } => {
                let evidence = world.evidence(evidence).unwrap();
                writeln!(
                    output,
                    "{} — {}: {}",
                    paint.good(REINTERPRETED),
                    evidence.name,
                    evidence.interpretations[*reading].text
                )?
            }
            Event::OutcomeReached { outcome } => {
                let outcome = world.world.outcomes.iter().find(|o| &o.id == outcome);
                let outcome = outcome.unwrap();
                writeln!(output, "{}\n{}", paint.title(&outcome.name), outcome.text)?
            }
            Event::PhaseEntered { phase } => {
                let phase = &world.world.phases[world.phase_index(phase).unwrap()];
                writeln!(output, "{}", paint.title(&format!("— {} —", phase.name)))?
            }
            Event::InventoryViewed => panels::inventory(output, engine, paint)?,
            Event::StatusViewed => panels::status(output, engine, paint)?,
            Event::QuestsViewed => panels::quests(output, engine, paint)?,
            Event::MapViewed => {
                let view = engine.map_view().unwrap();
                for line in crate::map::frame(world, &view, None).unwrap() {
                    writeln!(output, "{line}")?;
                }
            }
            Event::TimePassed {
                eventful: false, ..
            } if travelled => {}
            Event::TimePassed { minutes, now, .. } => writeln!(
                output,
                "{}",
                paint.dim(&format!(
                    "{} later: {}",
                    duration(*minutes),
                    clock(world, *now).unwrap_or_default()
                ))
            )?,
            Event::CharacterArrived { character } => {
                writeln!(output, "{} arrives.", name(character))?
            }
            Event::CharacterLeft { character } => writeln!(output, "{} leaves.", name(character))?,
            Event::Bought {
                good,
                quantity,
                cost,
            } => writeln!(
                output,
                "Bought: {} ×{quantity} for {}",
                world.item(good).unwrap().name,
                money(world, *cost)
            )?,
            Event::Sold {
                good,
                quantity,
                earned,
            } => writeln!(
                output,
                "Sold: {} ×{quantity} for {}",
                world.item(good).unwrap().name,
                money(world, *earned)
            )?,
            Event::CurrencyReceived { amount } => {
                writeln!(output, "Received: {}", money(world, *amount))?
            }
            Event::CurrencyPaid { amount } => writeln!(output, "Paid: {}", money(world, *amount))?,
            Event::MarketViewed => panels::market(output, engine, paint)?,
            Event::Moved { .. } | Event::DialogueEnded | Event::StoryFlagSet { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_journey_is_dated_only_when_something_happened_on_the_way() {
        let world = realmkit_spec::WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/marches"
        ))
        .unwrap();
        let engine = Engine::new(&world).unwrap();
        let render = |batch: &[Event]| {
            let mut output = Vec::new();
            events(&mut output, &engine, batch, Paint::default()).unwrap();
            String::from_utf8(output).unwrap()
        };
        let moved = Event::Moved {
            from: "greyford".into(),
            to: "ashmere".into(),
        };
        let passed = |eventful| Event::TimePassed {
            minutes: 120,
            now: 600,
            eventful,
        };
        let quiet = render(&[moved.clone(), passed(false)]);
        assert!(!quiet.contains("later"), "{quiet}");
        let eventful = render(&[moved, Event::CurrencyReceived { amount: 5 }, passed(true)]);
        assert!(
            eventful.contains("Received: 5 silver\n2 h later: Day 1, 10:00"),
            "{eventful}"
        );
    }

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
