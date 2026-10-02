//! Validation of consumable items and of wares sold at fixed prices.

mod common;

use common::*;
use realmkit_spec::*;

fn item<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Item {
    w.items.iter_mut().find(|i| i.id == id).unwrap()
}

fn gate(w: &mut WorldSpec) -> &mut Market {
    w.world
        .economy
        .as_mut()
        .unwrap()
        .markets
        .iter_mut()
        .find(|m| m.location == "gate")
        .unwrap()
}

#[test]
fn the_arena_sells_a_draught_that_heals_and_roundtrips() {
    let world = arena();
    assert!(codes(&world).is_empty(), "{:?}", world.diagnostics());
    let draught = world.item("healing_draught").unwrap();
    assert_eq!(draught.consumable, Some(Consumable { hp: 30, mp: 0 }));
    let economy = world.economy().unwrap();
    let market = economy.market("gate").unwrap();
    assert_eq!(economy.ware(market, "healing_draught").unwrap().price, 8);
    assert!(economy.ware(market, "rat_tail").is_none());
    let json = serde_json::to_string(draught).unwrap();
    assert_eq!(&serde_json::from_str::<Item>(&json).unwrap(), draught);
    // A plain item serializes without the field.
    let tail = serde_json::to_string(world.item("rat_tail").unwrap()).unwrap();
    assert!(!tail.contains("consumable"), "{tail}");
}

#[test]
fn consumables_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| item(w, "healing_draught").consumable = Some(Consumable { hp: 0, mp: 0 }),
            "invalid_consumable",
        ),
        (
            |w| {
                item(w, "healing_draught").consumable = Some(Consumable {
                    hp: STAT_BOUND + 1,
                    mp: 0,
                })
            },
            "invalid_consumable",
        ),
        (
            |w| {
                item(w, "healing_draught").consumable = Some(Consumable {
                    hp: 0,
                    mp: STAT_BOUND + 1,
                })
            },
            "invalid_consumable",
        ),
        // The arena's fighter can never have MP: an MP restore could never apply.
        (
            |w| item(w, "healing_draught").consumable = Some(Consumable { hp: 5, mp: 5 }),
            "invalid_consumable",
        ),
        // Wearing and eating the same thing is not a thing.
        (
            |w| item(w, "buckler").consumable = Some(Consumable { hp: 5, mp: 0 }),
            "invalid_consumable",
        ),
    ];
    for (change, code) in cases {
        let mut world = arena();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            world.diagnostics()
        );
    }
    // Where the player can have MP, restoring it is fine.
    let mut duel = duel();
    duel.items.push(Item {
        id: "ether".into(),
        name: "Ether".into(),
        description: "It fizzes.".into(),
        equipment: None,
        consumable: Some(Consumable { hp: 0, mp: 5 }),
    });
    assert!(
        !codes(&duel).contains(&"invalid_consumable".to_string()),
        "{:?}",
        duel.diagnostics()
    );
    // Restoring HP needs HP: a world without combat has none.
    let mut archive = archive();
    archive.items[0].consumable = Some(Consumable { hp: 5, mp: 0 });
    assert!(codes(&archive).contains(&"combat_disabled".to_string()));
}

#[test]
fn wares_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| gate(w).wares[0].price = 0, "invalid_price"),
        (
            |w| gate(w).wares[0].price = PRICE_BOUND + 1,
            "invalid_price",
        ),
        (
            |w| gate(w).wares[0].item = "missing".into(),
            "missing_reference",
        ),
        // A good already has a moving price; it cannot also have a fixed one.
        (
            |w| gate(w).wares[0].item = "rat_tail".into(),
            "invalid_ware",
        ),
        (
            |w| {
                let first = gate(w).wares[0].clone();
                gate(w).wares.push(first);
            },
            "invalid_ware",
        ),
    ];
    for (change, code) in cases {
        let mut world = arena();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            world.diagnostics()
        );
    }
}
