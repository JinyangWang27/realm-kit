//! Evidence: discovered by effects, known for good, apart from items.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::Direction::*;

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
    assert!(engine
        .dialogue_choices()
        .contains(&"The copyist has the map. He was only mending it."));
    let mut unseen = engine.snapshot();
    unseen.state.evidence.clear();
    let mut unseen = Engine::restore(&world, unseen).unwrap();
    unseen.execute(Talk("archivist".into())).unwrap();
    assert!(!unseen
        .dialogue_choices()
        .contains(&"The copyist has the map. He was only mending it."));
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
