use realmkit_spec::*;
use realmkit_worldgen::WorldDraft;
use std::{fs, path::PathBuf};

fn demo() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/demo-world"
    ))
    .unwrap()
}

#[test]
fn typed_edits_link_locations_and_report_repairable_diagnostics() {
    let world = demo();
    let mut location = world.locations[0].clone();
    location.id = "garden".into();
    location.npcs.clear();
    location.exits.clear();
    let mut draft = WorldDraft::new(world);
    draft.create_location(location.clone()).unwrap();
    assert!(draft.get_world().location("garden").is_some());
    assert!(draft.create_location(location.clone()).is_err());
    location.name = "The Walled Garden".into();
    draft.update_location(location).unwrap();
    assert_eq!(
        draft.get_world().location("garden").unwrap().name,
        "The Walled Garden"
    );
    let exit = Exit {
        destination: "garden".into(),
        requires: vec![],
        blocked_text: "The gate is shut.".into(),
    };
    draft
        .link_locations("village", Direction::West, exit.clone())
        .unwrap();
    assert_eq!(
        draft.get_world().location("village").unwrap().exits[&Direction::West].destination,
        "garden"
    );
    assert!(draft
        .link_locations("missing", Direction::East, exit)
        .is_err());
    let before = draft.get_world().clone();
    assert!(draft
        .link_locations(
            "garden",
            Direction::East,
            Exit {
                destination: "missing".into(),
                requires: vec![],
                blocked_text: String::new()
            }
        )
        .is_err());
    assert_eq!(draft.get_world(), &before);
    assert!(draft.validate_world().is_empty());
    let mut broken = draft.get_world().location("garden").unwrap().clone();
    broken.npcs.push("missing".into());
    draft.update_location(broken).unwrap();
    assert!(draft
        .validate_world()
        .iter()
        .any(|d| d.code == "missing_reference" && d.entity_id.as_deref() == Some("garden")));
}

#[test]
fn source_language_mismatch_is_structured_and_blocks_export() {
    let draft = WorldDraft::from_source(demo(), "zh-Hans");
    let diagnostics = draft.validate_world();
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "source_language_mismatch"));
    let temp = TestDir::new();
    let destination = temp.0.join("invalid");
    assert!(draft.export(&destination).is_err());
    assert!(!destination.exists());
    assert!(WorldDraft::from_source(demo(), "EN")
        .validate_world()
        .is_empty());
}

#[test]
fn exports_a_self_contained_package_and_refuses_to_overwrite() {
    let temp = TestDir::new();
    let destination = temp.0.join("world");
    let draft = WorldDraft::new(demo());
    draft.export(&destination).unwrap();
    assert_eq!(&WorldSpec::load(&destination).unwrap(), draft.get_world());
    let original = fs::read(destination.join("world.json")).unwrap();
    assert!(draft.export(&destination).is_err());
    assert_eq!(fs::read(destination.join("world.json")).unwrap(), original);
}

struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "realmkit-worldgen-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
