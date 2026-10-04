//! A graphical host's view of the engine: a package from memory, commands
//! and events as JSON, and saves as bytes, with no filesystem or terminal.

use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

/// The arena package as a browser would fetch it.
fn arena_files(name: &str) -> std::io::Result<Vec<u8>> {
    let bytes: &[u8] = match name {
        "world.json" => include_bytes!("../../../examples/arena/world.json"),
        "locations.json" => include_bytes!("../../../examples/arena/locations.json"),
        "characters.json" => include_bytes!("../../../examples/arena/characters.json"),
        "items.json" => include_bytes!("../../../examples/arena/items.json"),
        "quests.json" => include_bytes!("../../../examples/arena/quests.json"),
        "dialogues.json" => include_bytes!("../../../examples/arena/dialogues.json"),
        _ => return Err(std::io::ErrorKind::NotFound.into()),
    };
    Ok(bytes.to_vec())
}

fn wire<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> T {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

#[test]
fn a_host_plays_and_saves_through_json_alone() {
    let world = WorldSpec::from_files(arena_files).unwrap();
    let commands = [
        Look,
        Allocate {
            stat: Stat::Patk,
            points: 2,
        },
        Market,
        Buy {
            good: "healing_draught".into(),
            quantity: 1,
        },
        Move(North),
        Engage("grey_wolf".into()),
        Flee,
        Status,
    ];
    let (halfway, rest) = commands.split_at(4);
    let mut direct = Engine::new_with_seed(&world, 7).unwrap();
    let mut host = Engine::new_with_seed(&world, 7).unwrap();
    let run = |direct: &mut Engine, host: &mut Engine, command: &Command| {
        // A host offers what the engine lists, over the same wire.
        assert_eq!(wire(&host.actions()), host.actions());
        let expected = direct.execute(command.clone()).unwrap();
        let events = host.execute(wire(command)).unwrap();
        assert_eq!(wire(&events), expected, "{command:?}");
    };
    for command in halfway {
        run(&mut direct, &mut host, command);
    }
    let bytes = host.snapshot().to_json();
    let mut host = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    for command in rest {
        run(&mut direct, &mut host, command);
    }
    assert_eq!(host.state(), direct.state());
}

#[test]
fn commands_have_a_readable_wire_shape() {
    let parse = |json| serde_json::from_str::<Command>(json).unwrap();
    assert_eq!(parse(r#""look""#), Look);
    assert_eq!(parse(r#"{"move":"north"}"#), Move(North));
    assert_eq!(
        parse(r#"{"buy":{"good":"grain","quantity":2}}"#),
        Buy {
            good: "grain".into(),
            quantity: 2
        }
    );
}

#[test]
fn snapshots_of_another_format_are_refused_by_version() {
    let world = WorldSpec::from_files(arena_files).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_slice(&Engine::new(&world).unwrap().snapshot().to_json()).unwrap();
    json["save_format_version"] = 1.into();
    json["state"] = serde_json::Value::Null;
    let error = SaveSnapshot::from_json(&serde_json::to_vec(&json).unwrap()).unwrap_err();
    assert!(error
        .to_string()
        .contains("unsupported save format version 1"));
}
