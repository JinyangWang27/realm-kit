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
    // At prosperity 50 demand is as authored: halfway between 80% and 120%.
    assert_eq!(economy.supply(vellmarket, cloth, 1_000, 50), (12, 6));
    assert_eq!(economy.supply(vellmarket, wool, 745, 50), (24, 20));
    // Dear wool makes the looms use less: 18 × 1,000 / 1,500 = 12, plus 2.
    assert_eq!(economy.supply(vellmarket, wool, 1_500, 50), (24, 14));
    assert_eq!(economy.supply(vellmarket, wool, 1_001, 50), (24, 19));
    // A poor town wants 80% of its cloth and a rich one 120%, rounded down;
    // the looms' own wool is never scaled.
    assert_eq!(economy.supply(vellmarket, cloth, 1_000, 0), (12, 4));
    assert_eq!(economy.supply(vellmarket, cloth, 1_000, 100), (12, 7));
    assert_eq!(economy.supply(vellmarket, wool, 1_000, 0), (24, 19));
    assert_eq!(scaled(6, [80, 120], 25), 5);
    // Ashmere's grain and eels count towards Greyford's trade.
    let villages: Vec<_> = economy.villages("greyford").map(|m| &m.location).collect();
    assert_eq!(villages, ["ashmere"]);
    // Trading narrows the spread: 15% by 4% a rank, rounded down.
    let greyford = economy.market("greyford").unwrap();
    assert_eq!(
        (0..=3)
            .map(|r| economy.spread(greyford, r))
            .collect::<Vec<_>>(),
        [15, 14, 13, 13]
    );
    assert!(world.random_market());
    assert!(world.random_stock());
    assert_eq!(world.proficiency_max(Proficiency::Trading), Some(3));
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
        // Ashmere: renaming Greyford would strand Ashmere's town, and
        // Vellmarket the weavery's, as well.
        (
            |w| economy(w).markets[1].location = "nowhere".into(),
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
    // The reeve's reward, the troops' wages, upgrade costs and recruit
    // prices, Maddoc's weavery and the levels' trading points.
    let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
    assert_eq!(
        found,
        [
            "economy_disabled".to_string(),
            "proficiencies_disabled".into(),
            "workshops_disabled".into()
        ]
        .into()
    );
    let owners: std::collections::BTreeSet<_> = world
        .diagnostics()
        .into_iter()
        .filter_map(|d| d.entity_id)
        .collect();
    for owner in ["levy", "bowmen", "riders", "ashmere", "maddoc", "marches"] {
        assert!(owners.contains(owner), "{owner}: {owners:?}");
    }
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

#[test]
fn prosperity_stock_workshops_and_trading_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (
            |w| economy(w).prosperity.as_mut().unwrap().base = 101,
            "invalid_amount",
        ),
        (
            |w| economy(w).prosperity.as_mut().unwrap().scarce_above = 50,
            "invalid_amount",
        ),
        (
            |w| economy(w).prosperity.as_mut().unwrap().demand_percent = [80, 1_001],
            "invalid_amount",
        ),
        (
            |w| economy(w).prosperity.as_mut().unwrap().schedule.at = 0,
            "invalid_schedule",
        ),
        (
            |w| economy(w).markets[0].prosperity = Some(101),
            "invalid_amount",
        ),
        (
            |w| {
                economy(w).prosperity = None;
                economy(w).stock.as_mut().unwrap().prosperity_percent = [100, 100];
                economy(w).markets[1].prosperity = None;
                economy(w).markets[2].prosperity = None;
            },
            "invalid_amount",
        ),
        (
            |w| economy(w).stock.as_mut().unwrap().units = STOCK_BOUND / 2,
            "invalid_amount",
        ),
        (
            |w| economy(w).stock.as_mut().unwrap().currency = CURRENCY_BOUND,
            "invalid_amount",
        ),
        (
            |w| economy(w).markets[0].town = Some("vellmarket".into()),
            "invalid_town",
        ),
        (
            |w| economy(w).markets[1].town = Some("ashmere".into()),
            "invalid_town",
        ),
        (
            |w| economy(w).workshops.as_mut().unwrap().limit = 0,
            "invalid_limit",
        ),
        (
            |w| economy(w).workshops.as_mut().unwrap().kinds[0].resale = 151,
            "invalid_resale",
        ),
        (
            |w| economy(w).workshops.as_mut().unwrap().kinds[0].output = 0,
            "invalid_amount",
        ),
        (
            |w| economy(w).workshops.as_mut().unwrap().kinds[0].good = "keep_token".into(),
            "missing_reference",
        ),
        (
            |w| {
                let kinds = &mut economy(w).workshops.as_mut().unwrap().kinds;
                kinds.push(kinds[0].clone());
            },
            "duplicate_id",
        ),
        (
            |w| {
                let maddoc = w.dialogues.iter_mut().find(|d| d.id == "maddoc").unwrap();
                maddoc.nodes[0].choices[0].effects = vec![Effect::BuyWorkshop {
                    workshop: "tannery".into(),
                }];
            },
            "missing_reference",
        ),
        (
            |w| {
                w.world.events[0].effects.push(Effect::RaiseProficiency {
                    proficiency: Proficiency::Trading,
                    ranks: 1,
                })
            },
            "invalid_effect",
        ),
        (
            |w| {
                w.characters[2].requires = Some(Condition::Proficiency {
                    proficiency: Proficiency::Trading,
                    rank: 0,
                })
            },
            "invalid_amount",
        ),
        (
            |w| {
                economy(w).trading.as_mut().unwrap().max = 0;
                let combat = w.world.combat.as_mut().unwrap();
                combat
                    .levels
                    .iter_mut()
                    .for_each(|l| l.proficiency_points = 0);
            },
            "invalid_amount",
        ),
        // Trading stops at rank 3, so rank 4 can never hold or be taught.
        (
            |w| {
                w.characters[2].requires = Some(Condition::Proficiency {
                    proficiency: Proficiency::Trading,
                    rank: 4,
                })
            },
            "invalid_amount",
        ),
        (
            |w| {
                let maddoc = w.dialogues.iter_mut().find(|d| d.id == "maddoc").unwrap();
                maddoc.nodes[0].choices[2].effects = vec![Effect::RaiseProficiency {
                    proficiency: Proficiency::Trading,
                    ranks: 4,
                }];
            },
            "invalid_amount",
        ),
        // Workshops stand only in town markets.
        (
            |w| {
                w.characters[2].requires = Some(Condition::Workshop {
                    workshop: "weavery".into(),
                    location: Some("ashmere".into()),
                })
            },
            "invalid_town",
        ),
        (
            |w| {
                w.characters[2].requires = Some(Condition::Workshop {
                    workshop: "weavery".into(),
                    location: Some("hollin_keep".into()),
                })
            },
            "invalid_town",
        ),
        // Four points for three ranks of trading leave one that nothing takes.
        (
            |w| w.world.combat.as_mut().unwrap().levels[0].proficiency_points = 2,
            "invalid_points",
        ),
        (
            |w| {
                let trading = economy(w).trading.as_mut().unwrap();
                (trading.max, trading.narrow_percent) = (RANK_BOUND + 1, 0);
            },
            "invalid_amount",
        ),
        // Narrowed to 12%, Greyford's spread would let a round trip pay.
        (
            |w| economy(w).trading.as_mut().unwrap().narrow_percent = 6,
            "invalid_spread",
        ),
    ];
    for (index, (change, code)) in cases.into_iter().enumerate() {
        let mut world = marches();
        change(&mut world);
        let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
        let expected: std::collections::BTreeSet<_> = [code.to_string()].into();
        assert_eq!(found, expected, "case {index}");
    }
    // 3 ranks of 34% would take more than the whole spread, which also
    // leaves no margin against a round trip.
    let mut world = marches();
    economy(&mut world).trading.as_mut().unwrap().narrow_percent = 34;
    assert!(codes(&world).contains(&"invalid_amount".to_string()));
    // Stock scaled by prosperity without prosperity is only a warning.
    let mut world = marches();
    economy(&mut world).prosperity = None;
    world
        .world
        .economy
        .as_mut()
        .unwrap()
        .markets
        .iter_mut()
        .for_each(|m| m.prosperity = None);
    let diagnostics = world.diagnostics();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "unused_percent");
    assert_eq!(diagnostics[0].severity, Severity::Warning);
}

#[test]
fn workshops_and_proficiencies_need_their_blocks() {
    let mut world = marches();
    economy(&mut world).workshops = None;
    let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
    assert_eq!(found, ["workshops_disabled".to_string()].into());
    let mut world = marches();
    economy(&mut world).trading = None;
    let found: std::collections::BTreeSet<_> = codes(&world).into_iter().collect();
    assert_eq!(found, ["proficiencies_disabled".to_string()].into());
    // Each part needs world time.
    let mut world = marches();
    world.world.time = None;
    let owners: Vec<_> = world
        .diagnostics()
        .into_iter()
        .filter(|d| d.code == "time_disabled")
        .filter_map(|d| d.entity_id)
        .collect();
    // The tick, prosperity, stock and workshops, all owned by the world.
    assert!(
        owners.iter().filter(|o| *o == "marches").count() >= 4,
        "{owners:?}"
    );
    // Without stock, nothing is drawn for restocking.
    let mut world = marches();
    economy(&mut world).stock = None;
    assert!(world.diagnostics().is_empty());
    assert!(!world.random_stock());
}
