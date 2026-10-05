//! Evidence: discovered by effects, known for good, apart from items.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Condition, Direction::*, RouteOutcome};

#[test]
fn evidence_is_discovered_once_and_gates_what_follows() {
    let world = archive();
    let mut engine = reader(&world);
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::EvidenceDiscovered {
        evidence: "stitched_map".into()
    }));
    assert!(engine.state().evidence.contains("stitched_map"));
    // Asking again finds nothing new.
    engine.execute(Talk("copyist".into())).unwrap();
    let again = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(!again
        .iter()
        .any(|e| matches!(e, Event::EvidenceDiscovered { .. })));
    // Pell hears it only from someone who has seen the map.
    engine.execute(Move(South)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    assert!(texts(&engine).contains(&"The copyist has the map. He was only mending it.".into()));
    let mut unseen = engine.snapshot();
    unseen.state.evidence.clear();
    let mut unseen = Engine::restore(&world, unseen).unwrap();
    unseen.execute(Talk("archivist".into())).unwrap();
    assert!(!texts(&unseen).contains(&"The copyist has the map. He was only mending it.".into()));
}

#[test]
fn saves_know_only_evidence_something_could_discover() {
    let mut world = archive();
    world
        .world
        .evidence
        .push(realmkit_spec::EvidenceDefinition {
            id: "loose_page".into(),
            name: "A loose page".into(),
            description: "Nobody mentions it.".into(),
            source: String::new(),
            facts: Vec::new(),
            interpretations: Vec::new(),
            item: Some("pen".into()),
        });
    let mut snapshot = reader(&world).snapshot();
    snapshot.state.evidence.insert("loose_page".into());
    assert!(Engine::restore(&world, snapshot.clone()).is_err());
    snapshot.state.evidence = ["stitched_map".into()].into();
    assert!(Engine::restore(&world, snapshot).is_ok());
}

#[test]
fn carrying_an_item_does_not_make_its_evidence_known() {
    let mut world = archive();
    world.world.evidence[0].item = Some("pen".into());
    let engine = Engine::start(&world, 0, &["apprentice".into()]).unwrap();
    assert_eq!(engine.state().player.inventory.get("pen"), Some(&1));
    assert!(engine.state().evidence.is_empty());
}

#[test]
fn evidence_behind_a_choice_this_playthrough_never_had_is_refused() {
    // Only a scholar can ask Pell about the river's maps, and only that
    // question reveals the vault's catalogue.
    let mut world = archive();
    world
        .world
        .evidence
        .push(realmkit_spec::EvidenceDefinition {
            id: "catalogue".into(),
            name: "The vault's catalogue".into(),
            description: "Every river map, listed.".into(),
            source: String::new(),
            facts: Vec::new(),
            interpretations: Vec::new(),
            item: None,
        });
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    let scholar = pell.nodes[0]
        .choices
        .iter_mut()
        .find(|c| c.text.starts_with("I study"))
        .unwrap();
    scholar
        .effects
        .push(realmkit_spec::Effect::DiscoverEvidence {
            evidence: "catalogue".into(),
        });
    let mut reader = reader(&world).snapshot();
    reader.state.evidence.insert("catalogue".into());
    assert!(Engine::restore(&world, reader).is_err());
    let mut scholar = Engine::start(&world, 0, &["scholar".into()])
        .unwrap()
        .snapshot();
    scholar.state.evidence.insert("catalogue".into());
    assert!(Engine::restore(&world, scholar).is_ok());
}

#[test]
fn later_knowledge_changes_the_reading_but_not_the_facts() {
    let mut world = archive();
    // An outcome on the same flag shows the reading is reported first.
    world.world.outcomes.push(RouteOutcome {
        id: "vault_opened".into(),
        name: "The vault opens".into(),
        text: "The end.".into(),
        when: Condition::Flag {
            flag: "vault_open".into(),
        },
    });
    let mut engine = reader(&world);
    assert_eq!(engine.evidence_reading("stitched_map"), None);
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("copyist".into())).unwrap();
    let found = engine.execute(ChooseDialogue(1)).unwrap();
    // Newly found evidence is discovered, not reinterpreted.
    assert!(!found
        .iter()
        .any(|e| matches!(e, Event::EvidenceReinterpreted { .. })));
    assert_eq!(engine.evidence_reading("stitched_map"), Some(0));
    let facts = world.evidence("stitched_map").unwrap().facts.clone();
    // Pell's thanks open the vault, and with it a kinder reading.
    engine.execute(Move(South)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    let thanks = engine.execute(ChooseDialogue(1)).unwrap();
    let reinterpreted = Event::EvidenceReinterpreted {
        evidence: "stitched_map".into(),
        reading: 1,
    };
    let reached = Event::OutcomeReached {
        outcome: "vault_opened".into(),
    };
    let position = |event: &Event| thanks.iter().position(|e| e == event).unwrap();
    assert!(position(&reinterpreted) < position(&reached));
    assert_eq!(engine.evidence_reading("stitched_map"), Some(1));
    assert_eq!(world.evidence("stitched_map").unwrap().facts, facts);
    // Derived, so a reloaded save reads the same and says nothing new.
    let json = serde_json::to_string(&engine.snapshot()).unwrap();
    let mut reloaded = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(reloaded.evidence_reading("stitched_map"), Some(1));
    let moved = reloaded.execute(Move(North)).unwrap();
    assert!(!moved
        .iter()
        .any(|e| matches!(e, Event::EvidenceReinterpreted { .. })));
}

#[test]
fn facts_fill_in_once_each_and_only_for_known_evidence() {
    use realmkit_spec::Fact;
    let mut world = archive();
    let gated = |text: &str, flag: &str| Fact::Gated {
        text: text.into(),
        when: Condition::Flag { flag: flag.into() },
    };
    let facts = &mut world.world.evidence[0].facts;
    facts.push(gated("The thread is the copyist's own.", "pen_borrowed"));
    facts.push(gated("Pell knew all along.", "vault_open"));
    let mut engine = reader(&world);
    engine.execute(Talk("archivist".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(North)).unwrap();
    // A fact's condition holding reveals nothing while the evidence is unknown.
    engine.execute(Talk("copyist".into())).unwrap();
    let borrowed = engine.execute(ChooseDialogue(2)).unwrap();
    assert!(engine.evidence_facts("stitched_map").is_empty());
    let learned = |events: &[Event]| {
        events
            .iter()
            .filter(|e| matches!(e, Event::EvidenceFactLearned { .. }))
            .count()
    };
    assert_eq!(learned(&borrowed), 0);
    // Discovery brings every fact that holds by then, as part of discovering.
    engine.execute(Talk("copyist".into())).unwrap();
    let found = engine.execute(ChooseDialogue(1)).unwrap();
    assert_eq!(engine.evidence_facts("stitched_map"), [0, 1, 2]);
    assert_eq!(learned(&found), 0);
    // A later one arrives on its own, once.
    engine.execute(Move(South)).unwrap();
    engine.execute(Talk("archivist".into())).unwrap();
    let thanks = engine.execute(ChooseDialogue(1)).unwrap();
    let fact = Event::EvidenceFactLearned {
        evidence: "stitched_map".into(),
        fact: 3,
    };
    assert_eq!(learned(&thanks), 1);
    let position = |event: &Event| thanks.iter().position(|e| e == event).unwrap();
    assert!(
        position(&fact)
            < position(&Event::EvidenceReinterpreted {
                evidence: "stitched_map".into(),
                reading: 1,
            })
    );
    assert_eq!(engine.evidence_facts("stitched_map"), [0, 1, 2, 3]);
    let later = engine.execute(Move(North)).unwrap();
    assert_eq!(learned(&later), 0);
    // Derived, never saved: a reloaded save knows the same facts.
    let json = serde_json::to_string(&engine.snapshot()).unwrap();
    let reloaded = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(reloaded.evidence_facts("stitched_map"), [0, 1, 2, 3]);
    assert!(!json.contains("fact"));
}
