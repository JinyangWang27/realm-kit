use realmkit_engine::Command;
use realmkit_spec::Direction;

/// Commands a player can type; fighting and resting only where the world has combat.
pub fn help(combat: bool) -> String {
    let attack = if combat {
        "engage <character-id>\nattack <character-id>\nuse <skill-id> <character-id>\nrest — at a safe place\n"
    } else {
        ""
    };
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
    }
}
