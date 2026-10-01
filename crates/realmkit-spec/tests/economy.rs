//! Validation of currency, goods, producers, markets and the price tick.

mod common;

use common::*;
use realmkit_spec::*;

fn economy(w: &mut WorldSpec) -> &mut Economy {
    w.world.economy.as_mut().unwrap()
}

#[test]
fn the_marches_economy_validates_and_roundtrips() {
    let world = marches();
    let economy = world.economy().unwrap();
    let json = serde_json::to_string(economy).unwrap();
    assert_eq!(&serde_json::from_str::<Economy>(&json).unwrap(), economy);
    // Vellmarket's six looms make 12 cloth and use 18 wool; towns want 6 cloth.
    let vellmarket = economy.market("vellmarket").unwrap();
    let (cloth, wool) = (
        economy.good("cloth").unwrap(),
        economy.good("wool").unwrap(),
    );
    assert_eq!(economy.supply(vellmarket, cloth, 1_000), (12, 6));
    assert_eq!(economy.supply(vellmarket, wool, 745), (24, 20));
    // Dear wool makes the looms use less: 18 × 1,000 / 1,500 = 12, plus 2.
    assert_eq!(economy.supply(vellmarket, wool, 1_500), (24, 14));
    assert_eq!(economy.supply(vellmarket, wool, 1_001), (24, 19));
    assert!(world.random_market());
}

#[test]
fn economy_content_is_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| economy(w).currency.format.0 = "silver".into(),
            "invalid_template",
        ),
        (
            |w| economy(w).currency.start = CURRENCY_BOUND + 1,
            "invalid_amount",
        ),
        (|w| economy(w).index_bounds = [0, 10_000], "invalid_bounds"),
        (
            |w| economy(w).index_bounds = [100, INDEX_BOUND + 1],
            "invalid_bounds",
        ),
        (|w| economy(w).spread_percent = 1_001, "invalid_amount"),
        (|w| economy(w).goods[0].price = 0, "invalid_amount"),
        (
            |w| economy(w).goods[0].item = "missing".into(),
            "missing_reference",
        ),
        (
            |w| economy(w).goods[2].input = Some("cloth".into()),
            "invalid_input",
        ),
        (
            |w| economy(w).goods[2].input = Some("missing".into()),
            "missing_reference",
        ),
        (
            |w| {
                let copy = economy(w).goods[0].clone();
                economy(w).goods.push(copy)
            },
            "duplicate_id",
        ),
        (
            |w| {
                economy(w).producers[0].yields.insert("missing".into(), 1);
            },
            "missing_reference",
        ),
        (|w| economy(w).producers[0].name = " ".into(), "empty_name"),
        (
            |w| economy(w).markets[0].location = "nowhere".into(),
            "missing_reference",
        ),
        (
            |w| economy(w).markets[0].merchant = Some("nobody".into()),
            "missing_reference",
        ),
        (
            |w| {
                economy(w).markets[0].prices.insert("grain".into(), 50);
            },
            "invalid_price",
        ),
        (
            |w| {
                economy(w).markets[0]
                    .producers
                    .insert("fields".into(), QUANTITY_BOUND + 1);
            },
            "invalid_amount",
        ),
        // Enough producers to make more of a good than a tick can count.
        (
            |w| {
                economy(w).producers[0]
                    .yields
                    .insert("grain".into(), QUANTITY_BOUND);
                economy(w).markets[0]
                    .producers
                    .insert("fields".into(), QUANTITY_BOUND);
                economy(w).markets[0].producers.insert("traps".into(), 1);
                economy(w).producers[3].yields.insert("grain".into(), 1);
            },
            "invalid_amount",
        ),
        (|w| economy(w).links[0].percent = 0, "invalid_link"),
        (
            |w| economy(w).links[0].between[1] = "greyford".into(),
            "invalid_link",
        ),
        (
            |w| {
                economy(w).links.push(TradeLink {
                    between: ["ashmere".into(), "vellmarket".into()],
                    percent: 95,
                })
            },
            "invalid_link",
        ),
        // Shares far past 100 are reported, not added up past the integer.
        (
            |w| {
                economy(w).links[0].percent = 3_000_000_000;
                economy(w).links.push(TradeLink {
                    between: ["greyford".into(), "vellmarket".into()],
                    percent: 3_000_000_000,
                })
            },
            "invalid_link",
        ),
        // Without a spread, buying one unit and selling it back pays.
        (|w| economy(w).spread_percent = 0, "invalid_spread"),
        (
            |w| economy(w).markets[1].spread_percent = Some(5),
            "invalid_spread",
        ),
        // A step the spread does not cover: 1.3225 × 100 < 100 + 40.
        (|w| economy(w).trade_step = 40, "invalid_spread"),
        // Maddoc never comes to Greyford, so its market would never open.
        (
            |w| economy(w).markets[0].merchant = Some("maddoc".into()),
            "invalid_merchant",
        ),
        (
            |w| economy(w).tick.as_mut().unwrap().revert_percent = 101,
            "invalid_amount",
        ),
        (
            |w| economy(w).tick.as_mut().unwrap().schedule.at = 480,
            "invalid_schedule",
        ),
        (
            |w| w.characters[2].requires = Some(Condition::Currency { amount: 0 }),
            "invalid_amount",
        ),
        (
            |w| {
                w.world.events[0]
                    .effects
                    .push(Effect::PayCurrency { amount: 5 })
            },
            "invalid_effect",
        ),
    ];
    // A renamed good also strands what referred to it, so compare the codes
    // found, not how often.
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = marches();
        change(&mut world);
        let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
        assert_eq!(found, [code.to_string()].into(), "case {index}");
    }
    // Bounds must hold the base price, which also strands cheaper prices.
    let mut world = marches();
    economy(&mut world).index_bounds = [1_001, 10_000];
    assert!(codes(&world).contains(&"invalid_bounds".to_string()));
}

#[test]
fn currency_needs_an_economy_and_a_price_tick_needs_a_clock() {
    let mut world = marches();
    world.world.economy = None;
    // The reeve's reward.
    assert_eq!(codes(&world), ["economy_disabled"]);
    let mut world = marches();
    world.world.time = None;
    let codes = codes(&world);
    assert!(codes.contains(&"time_disabled".to_string()));
    // Without a tick, prices move only by trade and nothing is drawn for them.
    let mut world = marches();
    economy(&mut world).tick = None;
    assert!(world.diagnostics().is_empty());
    assert!(!world.random_market());
}
