//! Language overlays and the rules revision they share.

mod common;

use common::*;
use realmkit_spec::*;

#[test]
fn every_language_shares_one_revision() {
    let world = archive_in_all_languages();
    assert!(world.diagnostics().is_empty());
    let chinese = world.in_language("zh-Hans").unwrap();
    assert_eq!(chinese.world.language, "zh-Hans");
    assert_eq!(chinese.world.name, "寂静档案馆");
    assert!(chinese.translations.is_empty());
    assert_eq!(chinese.revision(), world.revision());
    // The base language is the package as authored, without its overlays.
    let english = world.in_language("en").unwrap();
    assert_eq!(english.world.name, "The Quiet Archive");
    assert_eq!(english.revision(), world.revision());
    assert!(matches!(
        world.in_language("fr"),
        Err(SpecError::UnknownLanguage(tag)) if tag == "fr"
    ));
}

#[test]
fn an_overlay_is_complete_exact_and_another_language() {
    type Change = fn(&mut WorldSpec);
    fn overlay(w: &mut WorldSpec) -> &mut std::collections::BTreeMap<String, String> {
        w.translations.get_mut("zh-Hans").unwrap()
    }
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| {
                overlay(w).remove("world.name");
            },
            "missing_translation",
        ),
        (
            |w| {
                overlay(w).insert("world.motto".into(), "安静".into());
            },
            "unused_translation",
        ),
        // A new line in the base needs its translation too.
        (
            |w| {
                w.dialogues[0].nodes[0].choices.push(DialogueChoice {
                    text: "Good night.".into(),
                    ..Default::default()
                })
            },
            "missing_translation",
        ),
        (
            |w| {
                w.translations.clear();
            },
            "missing_translation",
        ),
        (
            |w| w.world.translations.push("en".into()),
            "invalid_translation",
        ),
        (
            |w| w.world.translations.push("zh-Hans".into()),
            "invalid_translation",
        ),
        (
            |w| w.world.translations.push("../zh".into()),
            "invalid_translation",
        ),
    ];
    for (i, (change, code)) in cases.into_iter().enumerate() {
        let mut world = archive_in_all_languages();
        change(&mut world);
        let found = codes(&world);
        assert!(found.contains(&code.to_string()), "case {i}: {found:?}");
    }
}

#[test]
fn templates_are_checked_in_each_language() {
    let mut world = caravan_trail();
    let mut overlay = world.texts();
    overlay.insert(
        "world.combat.narrative.attack.0".into(),
        "{attacker} frappe {nope}.".into(),
    );
    world.world.translations.push("fr".into());
    world.translations.insert("fr".into(), overlay);
    let diagnostics = world.diagnostics();
    assert_eq!(codes(&world), ["invalid_template"]);
    assert!(
        diagnostics[0].message.starts_with("in fr: "),
        "{diagnostics:?}"
    );
}

#[test]
fn an_overlay_must_be_listed_and_keys_must_be_unambiguous() {
    // An overlay the package does not list would never load back.
    let mut world = archive();
    world.translations.insert("fr".into(), world.texts());
    assert_eq!(codes(&world), ["unused_translation"]);
    // A choice whose ID is "1" and the un-IDed choice at position 1 would
    // share a key, so one translation would land on both.
    let mut world = archive();
    let pell = world.dialogues.iter_mut().find(|d| d.id == "pell").unwrap();
    pell.nodes[0].choices[0].id = Some("1".into());
    assert_eq!(codes(&world), ["ambiguous_text_key"]);
}

#[test]
fn a_listed_language_without_its_file_is_a_diagnostic() {
    let dir = std::env::temp_dir().join(format!("realmkit-missing-overlay-{}", std::process::id()));
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/quiet-archive");
    std::fs::create_dir_all(&dir).unwrap();
    for file in PACKAGE_FILES {
        std::fs::copy(format!("{source}/{file}"), dir.join(file)).unwrap();
    }
    let error = WorldSpec::load(&dir).unwrap_err();
    std::fs::remove_dir_all(&dir).unwrap();
    let SpecError::Validation(diagnostics) = error else {
        panic!("{error}");
    };
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "missing_translation" && d.message.contains("text/zh-Hans.json")));
}
