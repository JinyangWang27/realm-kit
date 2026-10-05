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
