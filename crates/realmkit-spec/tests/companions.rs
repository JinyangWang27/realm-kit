//! Named companions: who can be one, and the effects and conditions that
//! name them.

mod common;

use common::*;
use realmkit_spec::*;

fn ford() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/hollow-ford"
    ))
    .unwrap()
}

/// A diagnostic code, and how to break the ford to get it.
type Case = (&'static str, fn(&mut WorldSpec));

fn character<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Character {
    w.characters.iter_mut().find(|c| c.id == id).unwrap()
}

#[test]
fn the_ford_is_valid_and_round_trips() {
    let world = ford();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    assert_eq!(
        character(&mut world.clone(), "oake").companion,
        Some(Companion { caution: Some(40) })
    );
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
}

#[test]
fn a_world_without_companions_keeps_its_revision() {
    // No companion field is written for a character without one.
    let world = arena();
    let encoded = serde_json::to_string(&world.characters).unwrap();
    assert!(!encoded.contains("companion"));
}

#[test]
fn a_companion_is_a_placed_real_time_ally_that_stays_with_the_party() {
    let cases: Vec<Case> = vec![
        ("companion_not_ally", |w| {
            character(w, "wren").combat.as_mut().unwrap().ally = false
        }),
        ("companion_not_ally", |w| character(w, "wren").combat = None),
        ("companion_without_realtime", |w| {
            w.world.combat.as_mut().unwrap().realtime = None
        }),
        ("companion_moves", |w| {
            character(w, "wren").moves = Some(Moves {
                among: vec!["inn".into(), "crossroads".into()],
                schedule: Schedule {
                    at: 600,
                    every: Some(60),
                },
            })
        }),
        ("companion_placement", |w| {
            w.locations[1].characters.push("wren".into())
        }),
        ("companion_placement", |w| {
            w.locations[0].characters.retain(|c| c != "wren")
        }),
        ("invalid_caution", |w| {
            character(w, "oake").companion = Some(Companion { caution: Some(0) })
        }),
        ("invalid_caution", |w| {
            character(w, "oake").companion = Some(Companion { caution: Some(101) })
        }),
    ];
    for (code, break_it) in cases {
        let mut world = ford();
        break_it(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            codes(&world)
        );
        assert!(world.validate().is_err(), "{code}");
    }
}

#[test]
fn party_effects_and_conditions_name_companions_and_only_dialogue_changes_the_party() {
    // Mara is no companion: she cannot join, leave or be asked about.
    let mut world = ford();
    let wren = world.dialogues.iter_mut().find(|d| d.id == "wren").unwrap();
    wren.nodes[0].choices[1].effects = vec![Effect::JoinParty {
        character: "mara".into(),
    }];
    wren.nodes[0].choices[2].requires = Some(Condition::Companion {
        character: "nobody".into(),
    });
    let codes = codes(&world);
    assert!(codes.contains(&"not_a_companion".to_string()), "{codes:?}");
    assert!(
        codes.contains(&"missing_reference".to_string()),
        "{codes:?}"
    );
    // A scheduled event or a start answer cannot recruit or dismiss anyone.
    let mut world = ford();
    world.world.events.push(WorldEvent {
        id: "parting".into(),
        schedule: Schedule {
            at: 600,
            every: None,
        },
        requires: None,
        effects: vec![Effect::LeaveParty {
            character: "wren".into(),
        }],
    });
    assert!(common::codes(&world).contains(&"invalid_effect".to_string()));
    let mut world = ford();
    world.world.start_questions.push(StartQuestion {
        id: "origin".into(),
        name: "Origin".into(),
        text: "Who travels with you?".into(),
        options: vec![StartOption {
            id: "friend".into(),
            text: "A friend.".into(),
            effects: vec![Effect::JoinParty {
                character: "wren".into(),
            }],
        }],
    });
    assert!(common::codes(&world).contains(&"invalid_effect".to_string()));
}
