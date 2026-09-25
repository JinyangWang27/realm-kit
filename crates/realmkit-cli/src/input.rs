use realmkit_engine::Command;
use realmkit_spec::Direction;

pub const HELP: &str = "look\ngo north|south|east|west|up|down (or n/s/e/w/u/d, h/j/k/l)\nattack <monster-id>\ntalk <npc-id>\nchoose <number> (or just the number)\naccept <quest-id>\ncomplete <quest-id>\ninventory\nstatus\nquests\nhelp\nquit";

#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    Command(Command),
    Help,
    Quit,
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
        ("look", []) => Command::Look,
        ("inventory" | "i", []) => Command::Inventory,
        ("status" | "c", []) => Command::Status,
        ("quests" | "q", []) => Command::Quests,
        ("go", [value]) => {
            Command::Move(direction(&value.to_ascii_lowercase()).ok_or("unknown direction")?)
        }
        ("attack", [id]) => Command::Attack((*id).into()),
        ("talk", [id]) => Command::Talk((*id).into()),
        ("accept", [id]) => Command::AcceptQuest((*id).into()),
        ("complete", [id]) => Command::CompleteQuest((*id).into()),
        ("choose", [number]) => {
            Command::ChooseDialogue(number.parse().map_err(|_| "expected a choice number")?)
        }
        (value, []) if direction(value).is_some() => Command::Move(direction(value).unwrap()),
        (value, []) => Command::ChooseDialogue(value.parse().map_err(|_| "unknown command")?),
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
        assert_eq!(parse("2"), Ok(Input::Command(Command::ChooseDialogue(2))));
        assert!(parse("north extra").is_err());
    }
}
