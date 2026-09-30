//! Recipes, stations and improvement tiers.

mod common;

use common::*;
use realmkit_spec::*;

fn smithy() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/smithy"
    ))
    .unwrap()
}

fn recipe<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Recipe {
    let combat = w.world.combat.as_mut().unwrap();
    combat.recipes.iter_mut().find(|r| r.id == id).unwrap()
}

fn tier<'w>(w: &'w mut WorldSpec, item: &str, index: usize) -> &'w mut Tier {
    let item = w.items.iter_mut().find(|i| i.id == item).unwrap();
    &mut item.equipment.as_mut().unwrap().tiers[index]
}

#[test]
fn the_smithy_loads_and_tiers_replace_what_they_set() {
    let world = smithy();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    assert_eq!(world.recipe("iron_sword").unwrap().station, "anvil");
    let mail = world.item("iron_mail").unwrap().equipment.as_ref().unwrap();
    // Tier 0 is the item as defined; a tier replaces the bonuses whole.
    assert_eq!(mail.bonuses_at(0)[&Stat::Pdef], 4);
    assert_eq!(mail.bonuses_at(1)[&Stat::Pdef], 6);
    assert_eq!(
        (mail.speed_penalty_at(0), mail.speed_penalty_at(1)),
        (10, 5)
    );
    // A tier that sets no speed penalty keeps the one before it.
    let sword = world
        .item("iron_sword")
        .unwrap()
        .equipment
        .as_ref()
        .unwrap();
    assert_eq!(sword.speed_penalty_at(2), 0);
}

#[test]
fn recipes_and_tiers_are_checked() {
    type Change = fn(&mut WorldSpec);
    let changes: [(Change, &str); 14] = [
        (
            |w| recipe(w, "iron_sword").station = "kiln".into(),
            "missing_reference",
        ),
        (
            |w| recipe(w, "iron_sword").output = "iron_ingot".into(),
            "invalid_output",
        ),
        (
            |w| recipe(w, "iron_sword").inputs[0].quantity = 0,
            "invalid_quantity",
        ),
        (
            |w| recipe(w, "iron_sword").inputs[0].item = "gold".into(),
            "missing_reference",
        ),
        (
            |w| recipe(w, "iron_mail").id = "iron_sword".into(),
            "duplicate_id",
        ),
        (
            |w| {
                let grant = recipe(w, "iron_sword").trains.as_mut().unwrap();
                grant.rank = Some(2);
            },
            "invalid_grant",
        ),
        (
            |w| recipe(w, "iron_sword").trains.as_mut().unwrap().technique = "archery".into(),
            "missing_reference",
        ),
        (
            |w| {
                recipe(w, "iron_sword").known_when = vec![Condition::Flag {
                    flag: "nope".into(),
                }]
            },
            "missing_reference",
        ),
        (
            |w| tier(w, "iron_sword", 0).name = TextTemplate("Fine {sword}".into()),
            "invalid_template",
        ),
        (
            |w| tier(w, "iron_sword", 0).station = "kiln".into(),
            "missing_reference",
        ),
        (
            |w| tier(w, "iron_sword", 0).cost[0].quantity = 0,
            "invalid_quantity",
        ),
        (
            |w| tier(w, "iron_mail", 0).speed_penalty = Some(STAT_BOUND + 1),
            "invalid_stats",
        ),
        // A tier's bonus counts toward the worst case like the item's own.
        (
            |w| {
                tier(w, "iron_mail", 0).bonuses.insert(Stat::Pdef, 9_995);
            },
            "invalid_points",
        ),
        (
            |w| {
                tier(w, "iron_sword", 1).requires = vec![Condition::Technique {
                    technique: "smithing".into(),
                    rank: 9,
                }]
            },
            "invalid_rank",
        ),
    ];
    for (index, (change, code)) in changes.into_iter().enumerate() {
        let mut world = smithy();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "case {index}: {:?}",
            world.diagnostics()
        );
    }
}

#[test]
fn stations_need_combat() {
    let mut quiet = archive();
    quiet.locations[0].stations = vec!["anvil".into()];
    assert!(codes(&quiet).contains(&"combat_disabled".to_string()));
}

#[test]
fn materials_are_counted_items_listed_once() {
    // The same item twice in one cost would be checked twice against one count.
    let mut twice = smithy();
    let inputs = &mut recipe(&mut twice, "iron_sword").inputs;
    inputs.push(inputs[0].clone());
    assert!(
        codes(&twice).contains(&"duplicate_id".to_string()),
        "{:?}",
        twice.diagnostics()
    );
    // Equipment arrives as pieces, never as a count to spend.
    let mut worn = smithy();
    tier(&mut worn, "iron_mail", 0).cost[0].item = "iron_sword".into();
    assert!(
        codes(&worn).contains(&"invalid_material".to_string()),
        "{:?}",
        worn.diagnostics()
    );
}

#[test]
fn every_tier_of_a_weapon_keeps_an_attack_in_its_channel() {
    // A special-channel sword whose base bonus is its wielder's only special
    // attack, and whose Fine tier drops it.
    let mut world = smithy();
    let combat = world.world.combat.as_mut().unwrap();
    combat.cross_share = 0;
    combat.levels.iter_mut().for_each(|l| l.stats.satk = 0);
    let sword = world
        .items
        .iter_mut()
        .find(|i| i.id == "iron_sword")
        .unwrap();
    let gear = sword.equipment.as_mut().unwrap();
    gear.basic_channel = Some(Channel::Special);
    gear.bonuses = [(Stat::Satk, 5)].into();
    assert!(
        codes(&world).contains(&"no_attack".to_string()),
        "{:?}",
        world.diagnostics()
    );
}
