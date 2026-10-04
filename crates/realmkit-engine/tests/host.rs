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

/// The caravan slice as a browser would fetch it.
fn caravan_files(name: &str) -> std::io::Result<Vec<u8>> {
    let bytes: &[u8] = match name {
        "world.json" => include_bytes!("../../../examples/caravan-trail/world.json"),
        "locations.json" => include_bytes!("../../../examples/caravan-trail/locations.json"),
        "characters.json" => include_bytes!("../../../examples/caravan-trail/characters.json"),
        "items.json" => include_bytes!("../../../examples/caravan-trail/items.json"),
        "quests.json" => include_bytes!("../../../examples/caravan-trail/quests.json"),
        "dialogues.json" => include_bytes!("../../../examples/caravan-trail/dialogues.json"),
        _ => return Err(std::io::ErrorKind::NotFound.into()),
    };
    Ok(bytes.to_vec())
}

/// A graphical client's whole loop over JSON: it picks offered actions and
/// dialogue choices, reads the journal and map views, and saves halfway.
#[test]
fn a_host_plays_the_caravan_slice_through_json_alone() {
    let world = WorldSpec::from_files(caravan_files).unwrap();
    // The start question and its answer travel as plain data too.
    let question = &world.world.start_questions[0];
    let answer: Id = wire(&question.options[1].id);
    let mut host = Engine::start(&world, 7, &[answer]).unwrap();
    let commands: Vec<Command> = serde_json::from_str(
        r#"[{"talk":"iselt"}, {"choose_dialogue":1}, {"travel":"thornwick"},
            {"talk":"city_gate"}, {"choose_dialogue":1}, {"talk":"iselt"}, {"choose_dialogue":1},
            "rest", {"talk":"dravin"}, {"choose_dialogue":1}, {"choose_dialogue":1},
            {"travel":"abandoned_camp"}, {"talk":"cold_camp"}, {"choose_dialogue":1},
            {"travel":"bandit_ridge"}, {"talk":"rask"}, {"choose_dialogue":1}, {"choose_dialogue":1},
            {"travel":"impossible_fork"}, {"talk":"fork_stones"}, {"choose_dialogue":1},
            {"travel":"old_stones"}, {"talk":"stone_ring"}, {"choose_dialogue":1}, "quests", "map"]"#,
    )
    .unwrap();
    let mut saved = None;
    for (i, command) in commands.into_iter().enumerate() {
        // Every command is one the host could have offered.
        let offered = wire(&host.actions());
        let in_dialogue = !host.dialogue_choices().is_empty();
        assert!(
            in_dialogue || offered.iter().any(|a| a.command == command && a.available),
            "{command:?} not offered"
        );
        let events = host.execute(command).unwrap();
        assert_eq!(wire(&events), events);
        if i == 10 {
            saved = Some(host.snapshot().to_json());
        }
    }
    let journal = wire(&host.journal());
    assert_eq!(journal, host.journal());
    assert_eq!(journal.phase.as_deref(), Some("lost_wagons"));
    assert!(journal.evidence.contains(&"bandit_testimony".to_string()));
    let map = wire(&host.map_view().unwrap());
    assert_eq!(map.here, "old_stones");
    assert_eq!(map.places.len(), 6);
    // The save resumes from its bytes alone, with the quest just taken.
    let resumed =
        Engine::restore(&world, SaveSnapshot::from_json(&saved.unwrap()).unwrap()).unwrap();
    assert_eq!(resumed.state().start_choices, ["merchant_child"]);
    assert_eq!(resumed.state().player.location, "thornwick");
    assert_eq!(resumed.journal().phase.as_deref(), Some("lost_wagons"));
}
