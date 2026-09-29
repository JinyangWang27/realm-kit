//! Packages and helpers shared by the validation tests.
#![allow(dead_code)]

use realmkit_spec::*;

pub fn demo() -> WorldSpec {
    // Deserialize directly so validation tests isolate validation from package loading.
    WorldSpec {
        world: serde_json::from_str(include_str!("../../../../examples/demo-world/world.json"))
            .unwrap(),
        locations: serde_json::from_str(include_str!(
            "../../../../examples/demo-world/locations.json"
        ))
        .unwrap(),
        characters: serde_json::from_str(include_str!(
            "../../../../examples/demo-world/characters.json"
        ))
        .unwrap(),
        items: serde_json::from_str(include_str!("../../../../examples/demo-world/items.json"))
            .unwrap(),
        quests: serde_json::from_str(include_str!("../../../../examples/demo-world/quests.json"))
            .unwrap(),
        dialogues: serde_json::from_str(include_str!(
            "../../../../examples/demo-world/dialogues.json"
        ))
        .unwrap(),
    }
}

pub fn archive() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/quiet-archive"
    ))
    .unwrap()
}

pub fn wolf(w: &mut WorldSpec) -> &mut Character {
    w.characters.iter_mut().find(|c| c.id == "wolf").unwrap()
}

pub fn duel() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/duel")).unwrap()
}

pub fn codes(w: &WorldSpec) -> Vec<String> {
    w.diagnostics().into_iter().map(|d| d.code).collect()
}

pub fn arena() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap()
}
