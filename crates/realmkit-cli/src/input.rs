use realmkit_engine::Command;
use realmkit_spec::{Direction, Stat, WorldSpec};

/// Commands a player can type; fighting and resting only where the world has combat.
pub fn help(world: &WorldSpec) -> String {
    let combat = world.combat();
    let mut attack = String::new();
    if combat.is_some() {
        attack += "engage <character-id>\nattack <character-id>\nuse <skill-id> <character-id>\nflee\nrest — at a safe place\n";
    }
    if combat.is_some_and(|c| !c.slots.is_empty()) {
        attack += "equip <#> — wear a piece from your pack\nunequip <#>\n";
    }
    if combat.is_some_and(|c| !c.recipes.is_empty()) {
        attack += "forge <recipe> — at its station\nimprove <#> — raise a piece one tier\n";
    }
    if combat.is_some_and(|c| !c.enchantments.is_empty()) {
        attack += "enchant <#> <enchantment> — at its station, once per piece\n";
    }
    if combat.is_some_and(|c| !c.techniques.is_empty()) {
        attack += "techniques — learned techniques and their ranks\n";
    }
    if combat.is_some_and(|c| c.stat_points.is_some()) {
        attack += "allocate hp|mp|patk|pdef|satk|sdef|speed [points]\nrespec — refund stat points, where allowed\n";
    }
    format!("<number> — choose from the menu (or arrows and Enter, then : to type a command)\nlook\ngo north|south|east|west|up|down (or n/s/e/w/u/d, h/j/k/l)\n{attack}talk <character-id>\nchoose <number> (or just the number)\naccept <quest-id>\ncomplete <quest-id>\ninventory\nstatus\nquests\nsave\nload [number] — list saves, or restore one\nhelp\nquit")
}

#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    Command(Command),
    /// One-based number from the menu currently on screen.
    Select(usize),
    Help,
    Quit,
    Save,
    /// List saves, or restore the one-based save shown in that list.
    Load(Option<usize>),
    Blank,
}

/// A piece's number as listed in the inventory, with or without its "#".
fn gear(value: &str) -> Result<u64, &'static str> {
    value
        .trim_start_matches('#')
        .parse()
        .map_err(|_| "expected an equipment number such as #3")
}

fn stat(value: &str) -> Option<Stat> {
    Some(match value {
        "hp" => Stat::Hp,
        "mp" => Stat::Mp,
        "patk" => Stat::Patk,
        "pdef" => Stat::Pdef,
        "satk" => Stat::Satk,
        "sdef" => Stat::Sdef,
        "speed" => Stat::Speed,
        _ => return None,
    })
}

fn direction(value: &str) -> Option<Direction> {
    match value {
        "north" | "n" | "k" => Some(Direction::North),
        "south" | "s" | "j" => Some(Direction::South),
        "east" | "e" | "l" => Some(Direction::East),
        "west" | "w" | "h" => Some(Direction::West),
        "up" | "u" => Some(Direction::Up),
        "down" | "d" => Some(Direction::Down),
        _ => None,
    }
}

pub fn parse(line: &str) -> Result<Input, &'static str> {
    let words: Vec<_> = line.split_whitespace().collect();
    let Some(verb) = words.first() else {
        return Ok(Input::Blank);
    };
    let verb = verb.to_ascii_lowercase();
    let command = match (verb.as_str(), &words[1..]) {
        ("help" | "?", []) => return Ok(Input::Help),
        ("quit" | "exit", []) => return Ok(Input::Quit),
        ("save", []) => return Ok(Input::Save),
        ("load", []) => return Ok(Input::Load(None)),
        ("load", [number]) => {
            return Ok(Input::Load(Some(
                number.parse().map_err(|_| "expected a save number")?,
            )))
        }
        ("look", []) => Command::Look,
        ("inventory" | "i", []) => Command::Inventory,
        ("status" | "c", []) => Command::Status,
        ("quests" | "q", []) => Command::Quests,
        ("techniques" | "t", []) => Command::Techniques,
        ("go", [value]) => {
            Command::Move(direction(&value.to_ascii_lowercase()).ok_or("unknown direction")?)
        }
        ("engage", [id]) => Command::Engage((*id).into()),
        ("attack", [id]) => Command::Attack((*id).into()),
        ("use", [skill, target]) => Command::UseSkill {
            skill: (*skill).into(),
            target: (*target).into(),
        },
        ("rest", []) => Command::Rest,
        ("flee", []) => Command::Flee,
        ("respec", []) => Command::Respec,
        ("equip", [piece]) => Command::Equip(gear(piece)?),
        ("unequip", [piece]) => Command::Unequip(gear(piece)?),
        ("forge", [recipe]) => Command::Forge((*recipe).into()),
        ("improve", [piece]) => Command::Improve(gear(piece)?),
        ("enchant", [piece, enchantment]) => Command::Enchant {
            piece: gear(piece)?,
            enchantment: (*enchantment).into(),
        },
        ("allocate", [name, rest @ ..]) if rest.len() <= 1 => Command::Allocate {
            stat: stat(&name.to_ascii_lowercase()).ok_or("unknown stat")?,
            points: match rest {
                [n] => n.parse().map_err(|_| "expected a number of points")?,
                _ => 1,
            },
        },
        ("talk", [id]) => Command::Talk((*id).into()),
        ("accept", [id]) => Command::AcceptQuest((*id).into()),
        ("complete", [id]) => Command::CompleteQuest((*id).into()),
        ("choose", [number]) => {
            Command::ChooseDialogue(number.parse().map_err(|_| "expected a choice number")?)
        }
        (value, []) if direction(value).is_some() => Command::Move(direction(value).unwrap()),
        (value, []) => return Ok(Input::Select(value.parse().map_err(|_| "unknown command")?)),
        _ => return Err("wrong arguments"),
    };
    Ok(Input::Command(command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_directions_and_preserves_entity_ids() {
        for (text, expected) in [
            ("k", Direction::North),
            ("j", Direction::South),
            ("h", Direction::West),
            ("l", Direction::East),
            ("go UP", Direction::Up),
            ("down", Direction::Down),
        ] {
            assert_eq!(parse(text), Ok(Input::Command(Command::Move(expected))));
        }
        assert_eq!(
            parse(" TALK Elder_1 "),
            Ok(Input::Command(Command::Talk("Elder_1".into())))
        );
        assert_eq!(parse("2"), Ok(Input::Select(2)));
        assert_eq!(
            parse("choose 2"),
            Ok(Input::Command(Command::ChooseDialogue(2)))
        );
        assert!(parse("north extra").is_err());
        assert_eq!(parse("save"), Ok(Input::Save));
        assert_eq!(parse("load"), Ok(Input::Load(None)));
        assert_eq!(parse("load 2"), Ok(Input::Load(Some(2))));
        assert!(parse("load two").is_err());
        assert_eq!(
            parse("use Bolt witch"),
            Ok(Input::Command(Command::UseSkill {
                skill: "Bolt".into(),
                target: "witch".into()
            }))
        );
        assert_eq!(parse("rest"), Ok(Input::Command(Command::Rest)));
        assert!(parse("use bolt").is_err());
        assert_eq!(
            parse("allocate HP 2"),
            Ok(Input::Command(Command::Allocate {
                stat: Stat::Hp,
                points: 2
            }))
        );
        assert_eq!(
            parse("allocate speed"),
            Ok(Input::Command(Command::Allocate {
                stat: Stat::Speed,
                points: 1
            }))
        );
        assert!(parse("allocate luck").is_err());
        assert!(parse("allocate hp two").is_err());
        assert_eq!(parse("respec"), Ok(Input::Command(Command::Respec)));
        assert_eq!(parse("equip #3"), Ok(Input::Command(Command::Equip(3))));
        assert_eq!(parse("unequip 3"), Ok(Input::Command(Command::Unequip(3))));
        assert!(parse("equip sword").is_err());
        assert_eq!(
            parse("forge iron_sword"),
            Ok(Input::Command(Command::Forge("iron_sword".into())))
        );
        assert_eq!(parse("improve #1"), Ok(Input::Command(Command::Improve(1))));
        assert_eq!(
            parse("enchant 2 keenness"),
            Ok(Input::Command(Command::Enchant {
                piece: 2,
                enchantment: "keenness".into()
            }))
        );
    }
}
