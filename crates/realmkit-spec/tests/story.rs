//! Story phases, main quests and quest prerequisites.

mod common;

use common::*;
use realmkit_spec::*;

fn phase(id: &str) -> Phase {
    Phase {
        id: id.into(),
        name: id.into(),
    }
}

fn done(quest: &str) -> Condition {
    Condition::Quest {
        quest: quest.into(),
        status: QuestStatus::Completed,
    }
}

/// The archive in two phases, the second entered when Pell hears the map
/// is found, and a second quest that follows the first.
fn chained() -> WorldSpec {
    let mut w = archive();
    w.world.phases = vec![phase("visit"), phase("vault")];
    let pell = w.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices[1].effects.push(Effect::EnterPhase {
        phase: "vault".into(),
    });
    let mut second = w.quests[0].clone();
    second.id = "catalogue".into();
    second.requires = Some(done("lost_map"));
    w.quests.push(second);
    w
}

#[test]
fn a_chained_world_is_valid_and_roundtrips() {
    let world = chained();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    let json = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&json).unwrap(), world);
}

#[test]
fn phases_and_prerequisites_are_checked() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| w.world.phases.push(phase("visit")), "duplicate_id"),
        // Nothing enters it, so the story could never get there.
        (
            |w| w.world.phases.push(phase("epilogue")),
            "unreachable_phase",
        ),
        (
            |w| {
                w.quests[1].requires = Some(Condition::Phase {
                    phase: "missing".into(),
                })
            },
            "missing_reference",
        ),
        (
            |w| {
                w.dialogues[0].nodes[0].choices[0]
                    .effects
                    .push(Effect::EnterPhase {
                        phase: "missing".into(),
                    })
            },
            "missing_reference",
        ),
        (
            |w| {
                w.world.start_questions[0].options[0]
                    .effects
                    .push(Effect::EnterPhase {
                        phase: "vault".into(),
                    })
            },
            "invalid_effect",
        ),
        (
            |w| w.quests[1].requires = Some(done("missing")),
            "missing_reference",
        ),
        // Each waits for the other.
        (
            |w| w.quests[0].requires = Some(done("catalogue")),
            "quest_cycle",
        ),
        (
            |w| w.quests[1].requires = Some(done("catalogue")),
            "quest_cycle",
        ),
        // The main story may not wait on a side story.
        (|w| w.quests[1].main = true, "main_requires_side"),
    ];
    for (change, code) in cases {
        let mut world = chained();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            codes(&world)
        );
    }
    // Both main, or a side alternative beside a main one, is fine.
    let mut world = chained();
    world.quests[0].main = true;
    world.quests[1].main = true;
    assert!(world.diagnostics().is_empty());
    let mut world = chained();
    world.quests[1].main = true;
    world.quests[1].requires = Some(Condition::Any {
        of: vec![
            done("lost_map"),
            Condition::Phase {
                phase: "vault".into(),
            },
        ],
    });
    assert!(world.diagnostics().is_empty());
}
