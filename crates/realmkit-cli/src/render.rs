use realmkit_engine::{Engine, Event};
use realmkit_spec::{Direction, TextTemplate};
use std::io::{self, Write};

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

pub fn events(output: &mut impl Write, engine: &Engine<'_>, events: &[Event]) -> io::Result<()> {
    let world = engine.world();
    let state = engine.state();
    let name = |id: &str| &world.character(id).unwrap().name;
    let player = name(&world.world.player);
    for event in events {
        match event {
            Event::LocationViewed { location } => {
                let location = world.location(location).unwrap();
                writeln!(output, "{}\n{}", location.name, location.description)?;
                write!(output, "Exits:")?;
                for (direction, exit) in &location.exits {
                    write!(
                        output,
                        " {}{}",
                        direction_name(*direction),
                        if engine.conditions_met(&exit.requires) {
                            ""
                        } else {
                            " (locked)"
                        }
                    )?;
                }
                if location.exits.is_empty() {
                    write!(output, " none")?;
                }
                writeln!(output)?;
                for id in &location.characters {
                    let character = world.character(id).unwrap();
                    if !engine.conditions_met(&character.requires) {
                        continue;
                    }
                    match state.monster_hp.get(id) {
                        Some(0) => {}
                        Some(hp) => writeln!(
                            output,
                            "{} (HP {hp}) — {}",
                            character.name, character.description
                        )?,
                        None => writeln!(output, "{} — {}", character.name, character.description)?,
                    }
                }
            }
            Event::DamageDealt {
                target,
                amount,
                variant,
            } => {
                let amount = amount.to_string();
                writeln!(
                    output,
                    "{}",
                    interpolate(
                        &world.combat().unwrap().narrative.attack[*variant],
                        &[
                            ("attacker", player),
                            ("target", name(target)),
                            ("damage", &amount)
                        ]
                    )?
                )?;
            }
            Event::DamageReceived {
                source,
                amount,
                variant,
            } => {
                let amount = amount.to_string();
                writeln!(
                    output,
                    "{}",
                    interpolate(
                        &world.combat().unwrap().narrative.hurt[*variant],
                        &[
                            ("attacker", name(source)),
                            ("target", player),
                            ("damage", &amount)
                        ]
                    )?
                )?;
            }
            Event::EnemyDefeated { monster } => writeln!(
                output,
                "{}",
                interpolate(
                    &world.combat().unwrap().narrative.victory,
                    &[("target", name(monster))]
                )?
            )?,
            Event::PlayerDied => writeln!(output, "{}", world.combat().unwrap().narrative.death)?,
            Event::ItemReceived { item, quantity } => writeln!(
                output,
                "Received: {} ×{}",
                world.item(item).unwrap().name,
                quantity
            )?,
            Event::ExperienceGranted { amount } => writeln!(output, "+{amount} XP")?,
            Event::LevelUp { level } => writeln!(output, "Level {level}! Health restored.")?,
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
            Event::InventoryViewed => {
                writeln!(output, "Inventory:")?;
                if state.player.inventory.is_empty() {
                    writeln!(output, "  Empty")?;
                }
                for (id, count) in &state.player.inventory {
                    let item = world.item(id).unwrap();
                    writeln!(
                        output,
                        "  {} ×{} [{}] — {}",
                        item.name, count, id, item.description
                    )?;
                }
            }
            Event::StatusViewed => writeln!(
                output,
                "{} — Level {} | HP {}/{} | Attack {} | XP {}",
                player,
                state.player.level,
                state.player.hp,
                state.player.max_hp,
                state.player.attack,
                state.player.xp
            )?,
            Event::QuestsViewed => {
                writeln!(output, "Quests:")?;
                for quest in &world.quests {
                    writeln!(
                        output,
                        "  {} [{}]: {:?}",
                        quest.name, quest.id, state.quests[&quest.id]
                    )?;
                }
            }
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
