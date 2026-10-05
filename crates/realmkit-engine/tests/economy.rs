//! Currency, trade and the price tick, whose numbers come from
//! `python3 -m scripts.combat_sim economy examples/marches --seed 7`.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn index(engine: &Engine<'_>, market: &str, good: &str) -> u32 {
    engine.state().economy.as_ref().unwrap().prices[market][good]
}

/// The marches without merchants' stock, so only currency limits trade.
fn unlimited() -> WorldSpec {
    let mut world = marches();
    world.world.economy.as_mut().unwrap().stock = None;
    world
}

/// The best trader the marches allow: rank 3, so the narrowest spread,
/// trained with the three points of level 3.
fn best_trader(state: &mut GameState) {
    let combat = state.combat.as_mut().unwrap();
    (combat.level, combat.xp) = (3, 80);
    let trained = ProficiencyState {
        trained: 3,
        taught: 0,
    };
    state.proficiencies.insert(Proficiency::Trading, trained);
}

/// Every index at every market after `ticks` daily price ticks from the start.
fn after_ticks(world: &WorldSpec, seed: u64, ticks: u64) -> Vec<[u32; 4]> {
    let mut engine = Engine::new_with_seed(world, seed).unwrap();
    // The first tick is at minute 1,440; play starts at 480.
    engine.execute(Wait(1_440 * ticks - 480)).unwrap();
    ["greyford", "ashmere", "vellmarket"]
        .map(|m| ["grain", "wool", "cloth", "eels"].map(|g| index(&engine, m, g)))
        .to_vec()
}

#[test]
fn the_price_tick_matches_the_simulator() {
    let world = marches();
    assert_eq!(
        after_ticks(&world, 7, 1),
        [
            [530, 1056, 1215, 479],
            [473, 1027, 1132, 414],
            [1303, 781, 735, 1187]
        ]
    );
    assert_eq!(
        after_ticks(&world, 7, 3),
        [
            [491, 1060, 1214, 409],
            [470, 1031, 1148, 402],
            [1353, 767, 712, 1195]
        ]
    );
}

#[test]
fn producers_use_less_of_a_dear_input_as_the_simulator_does() {
    // Wool at twice its price: Vellmarket's looms use 18 × 1,000 / 2,000 = 9,
    // so 24 made against 11 used, and cloth is pulled up after it.
    let mut world = marches();
    let economy = world.world.economy.as_mut().unwrap();
    economy.markets[2].prices.insert("wool".into(), 2_000);
    assert_eq!(after_ticks(&world, 7, 1)[2], [1303, 1926, 849, 1187]);
}

#[test]
fn trade_prices_match_the_simulator() {
    assert_eq!(buy_price(120, 712, 15), 98);
    assert_eq!(sell_price(120, 1_309, 15), 136);
    assert_eq!(buy_price(15, 517, 25), 9);
    assert_eq!(sell_price(15, 1_301, 15), 16);
    // Buying is never free.
    assert_eq!(buy_price(1, 100, 0), 1);
}

#[test]
fn prices_hold_still_until_a_tick_and_the_tick_draws_its_own_stream() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Wait(1_440 - 480 - 1)).unwrap();
    assert_eq!(index(&engine, "vellmarket", "cloth"), 732);
    // Without the economy, Wenna wanders exactly as she does with it.
    let mut plain = marches();
    plain.world.economy = None;
    // Wages, upgrades and recruits are free without currency.
    for line in &mut plain.world.troops.as_mut().unwrap().lines {
        line.levels.iter_mut().for_each(|l| l.wage = None);
        line.upgrades.iter_mut().for_each(|u| u.cost = 0);
    }
    for location in &mut plain.locations {
        for offer in location.recruits.iter_mut().flat_map(|r| &mut r.troops) {
            offer.price = 0;
        }
    }
    for dialogue in &mut plain.dialogues {
        for choice in dialogue.nodes.iter_mut().flat_map(|n| &mut n.choices) {
            choice
                .effects
                .retain(|e| !matches!(e, Effect::GrantCurrency { .. }));
        }
    }
    // Maddoc sells a workshop, and levels grant trading points.
    plain.dialogues.retain(|d| d.id != "maddoc");
    plain.characters.iter_mut().for_each(|c| {
        if c.id == "maddoc" {
            c.dialogue = None;
        }
    });
    let combat = plain.world.combat.as_mut().unwrap();
    combat
        .levels
        .iter_mut()
        .for_each(|l| l.proficiency_points = 0);
    let wander = |world: &WorldSpec| {
        let mut engine = Engine::new_with_seed(world, 7).unwrap();
        engine.execute(Wait(10 * 1_440)).unwrap();
        engine.state().whereabouts.clone()
    };
    assert_eq!(wander(&world), wander(&plain));
}

#[test]
fn buying_and_selling_move_currency_goods_and_the_index() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    assert_eq!(currency(&engine), 100);
    // Eels at 388 with a 25% spread: 7, then 7 at 414 and 8 at 440.
    let events = engine.execute(buy("eels", 3)).unwrap();
    assert_eq!(
        events,
        [Event::Bought {
            good: "eels".into(),
            quantity: 3,
            cost: buy_price(15, 388, 25) + buy_price(15, 414, 25) + buy_price(15, 440, 25)
        }]
    );
    assert_eq!(index(&engine, "ashmere", "eels"), 388 + 3 * 26);
    assert_eq!(engine.state().player.inventory["eels"], 3);
    let spent = 100 - currency(&engine);
    // Selling one back lowers the index by the same step.
    let events = engine.execute(sell("eels", 1)).unwrap();
    let earned = sell_price(15, 466, 25);
    assert_eq!(
        events,
        [Event::Sold {
            good: "eels".into(),
            quantity: 1,
            earned
        }]
    );
    assert_eq!(index(&engine, "ashmere", "eels"), 466 - 26);
    assert_eq!(currency(&engine), 100 - spent + earned);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn refused_trades_change_nothing() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let before = engine.state().clone();
    // Ashmere's merchants hold 16 grain and no cloth.
    let refusals = [
        (buy("grain", 16), "afford"),
        (buy("grain", 17), "left"),
        (buy("cloth", 1), "left"),
        (buy("eels", 0), "units"),
        (buy("eels", TRADE_BOUND + 1), "units"),
        (sell("grain", 1), "enough"),
        (buy("sealed_letter", 1), "not traded"),
    ];
    for (command, why) in refusals {
        let error = engine.execute(command).unwrap_err();
        assert!(error.to_string().contains(why), "{error}");
        assert_eq!(engine.state(), &before);
    }
    // Hollin Keep has no market.
    engine.execute(Travel("greyford".into())).unwrap();
    engine.execute(Travel("hollin_keep".into())).unwrap();
    assert!(matches!(
        engine.execute(buy("eels", 1)),
        Err(EngineError::NoMarket)
    ));
    assert!(matches!(engine.execute(Market), Err(EngineError::NoMarket)));
}

#[test]
fn a_market_with_a_merchant_trades_only_while_the_merchant_is_there() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let trading = |engine: &Engine<'_>| engine.actions().iter().any(|a| a.command == Market);
    assert!(trading(&engine));
    // Hild keeps her stall from 06:00 to 22:00.
    engine.execute(Wait(14 * 60)).unwrap();
    assert!(!trading(&engine));
    assert!(matches!(
        engine.execute(buy("grain", 1)),
        Err(EngineError::NotHere(merchant)) if merchant == "hild"
    ));
    engine.execute(Wait(8 * 60)).unwrap();
    assert!(trading(&engine));
    engine.execute(buy("grain", 1)).unwrap();
}

#[test]
fn the_menu_offers_what_can_be_bought_and_what_is_carried() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let trade: Vec<_> = offered(&engine)
        .into_iter()
        .filter(|(c, _)| matches!(c, Market | Buy { .. } | Sell { .. }))
        .collect();
    assert_eq!(
        trade,
        [
            (Market, true),
            (buy("grain", 1), true),
            // Ashmere makes neither wool nor cloth, so its merchants hold none.
            (buy("wool", 1), false),
            (buy("cloth", 1), false),
            (buy("eels", 1), true),
        ]
    );
    engine.execute(buy("eels", 1)).unwrap();
    assert!(offered(&engine).contains(&(sell("eels", 1), true)));
}

#[test]
fn currency_effects_and_conditions() {
    let mut world = marches();
    world.world.flags.push("toll_paid".into());
    let reeve = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "reeve")
        .unwrap();
    reeve.nodes[0].choices.insert(
        0,
        DialogueChoice {
            text: "Pay the causeway toll.".into(),
            next: None,
            requires: Some(Condition::Currency { amount: 150 }),
            effects: vec![
                Effect::SetFlag {
                    flag: "toll_paid".into(),
                },
                Effect::PayCurrency { amount: 150 },
            ],
            ..Default::default()
        },
    );
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine.execute(Talk("reeve".into())).unwrap();
    // 100 silver is not enough to see the toll.
    assert_eq!(texts(&engine)[0], "I can carry a message.");
    let mut rich = engine.snapshot();
    rich.state.economy.as_mut().unwrap().currency = 160;
    let mut rich = Engine::restore(&world, rich).unwrap();
    assert_eq!(texts(&rich)[0], "Pay the causeway toll.");
    let events = rich.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::CurrencyPaid { amount: 150 }));
    assert_eq!(currency(&rich), 10);

    // Paying more than is held refuses the whole choice, flag and all.
    let reeve = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "reeve")
        .unwrap();
    reeve.nodes[0].choices[0].requires = None;
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine.execute(Talk("reeve".into())).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(ChooseDialogue(1)),
        Err(EngineError::NotEnoughCurrency)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn a_save_keeps_currency_and_prices_and_rejects_impossible_ones() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine.execute(buy("eels", 4)).unwrap();
    engine.execute(Wait(3 * 1_440)).unwrap();
    let good = engine.snapshot();
    let json = serde_json::to_string(&good).unwrap();
    let mut resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(
        resumed.execute(Wait(5 * 1_440)).unwrap(),
        engine.execute(Wait(5 * 1_440)).unwrap()
    );
    assert_eq!(resumed.state(), engine.state());
    // Trade goods come and go, so any count of them loads.
    let mut hoard = good.clone();
    hoard.state.player.inventory.insert("cloth".into(), 500);
    assert!(Engine::restore(&world, hoard).is_ok());

    let mut cheap = good.clone();
    cheap
        .state
        .economy
        .as_mut()
        .unwrap()
        .prices
        .get_mut("ashmere")
        .unwrap()
        .insert("eels".into(), 99);
    let mut missing = good.clone();
    missing
        .state
        .economy
        .as_mut()
        .unwrap()
        .prices
        .remove("vellmarket");
    let mut stray = good.clone();
    stray
        .state
        .economy
        .as_mut()
        .unwrap()
        .prices
        .get_mut("ashmere")
        .unwrap()
        .insert("keep_token".into(), 1_000);
    let mut rich = good.clone();
    rich.state.economy.as_mut().unwrap().currency = CURRENCY_BOUND + 1;
    let mut poor = good.clone();
    poor.state.economy = None;
    let mut unseeded = good.clone();
    unseeded.state.rng.as_mut().unwrap().market = None;
    for (i, snapshot) in [cheap, missing, stray, rich, poor, unseeded]
        .into_iter()
        .enumerate()
    {
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "corruption {i} was accepted"
        );
    }
}

#[test]
fn indices_stay_within_their_bounds_under_heavy_trade() {
    let world = unlimited();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let mut rich = engine.snapshot();
    rich.state.economy.as_mut().unwrap().currency = 1_000_000;
    engine = Engine::restore(&world, rich).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    for _ in 0..3 {
        engine.execute(buy("grain", TRADE_BOUND)).unwrap();
    }
    assert_eq!(index(&engine, "ashmere", "grain"), 10_000);
    for _ in 0..3 {
        engine.execute(sell("grain", TRADE_BOUND)).unwrap();
    }
    assert_eq!(index(&engine, "ashmere", "grain"), 100);
    // A tick keeps them in bounds too.
    engine.execute(Wait(1_440)).unwrap();
    let prices = &engine.state().economy.as_ref().unwrap().prices;
    assert!(prices
        .values()
        .flat_map(|p| p.values())
        .all(|i| (100..=10_000).contains(i)));
}

#[test]
fn a_sale_that_would_pass_the_currency_bound_is_not_offered() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine.execute(buy("eels", 1)).unwrap();
    assert!(offered(&engine).contains(&(sell("eels", 1), true)));
    let mut rich = engine.snapshot();
    rich.state.economy.as_mut().unwrap().currency = CURRENCY_BOUND - 1;
    let mut rich = Engine::restore(&world, rich).unwrap();
    assert!(offered(&rich).contains(&(sell("eels", 1), false)));
    assert!(matches!(
        rich.execute(sell("eels", 1)),
        Err(EngineError::NumericLimit)
    ));
}

#[test]
fn a_defeated_merchant_trades_no_more() {
    let mut world = marches();
    // Hild can fight, and loses.
    let combat: Combat = serde_json::from_str(
        r#"{
            "special_name": "Spirit",
            "timeline": { "action_cost": 100000, "speed_cap": 200 },
            "levels": [{ "xp": 0, "stats": { "hp": 50, "patk": 50, "pdef": 5, "satk": 0, "sdef": 5, "speed": 100 } }],
            "narrative": { "attack": ["{attacker} {target} {damage}"], "hurt": ["{attacker} {target} {damage}"], "victory": "{target}", "death": "x" }
        }"#,
    )
    .unwrap();
    world.world.combat = Some(combat);
    let hild = world
        .characters
        .iter_mut()
        .find(|c| c.id == "hild")
        .unwrap();
    hild.requires = None;
    hild.combat = Some(
        serde_json::from_str(
            r#"{ "stats": { "hp": 5, "patk": 1, "pdef": 0, "satk": 0, "sdef": 0, "speed": 100 }, "xp": 1 }"#,
        )
        .unwrap(),
    );
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(buy("grain", 1)).unwrap();
    engine.execute(Engage("hild".into())).unwrap();
    fight_out(&mut engine);
    assert!(engine
        .state()
        .combat
        .as_ref()
        .unwrap()
        .defeated
        .contains("hild"));
    assert!(!offered(&engine).contains(&(Market, true)));
    assert!(matches!(
        engine.execute(buy("grain", 1)),
        Err(EngineError::NotHere(merchant)) if merchant == "hild"
    ));
}

#[test]
fn buying_is_not_offered_when_the_count_cannot_hold_another_unit() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    let mut full = engine.snapshot();
    full.state.player.inventory.insert("eels".into(), u64::MAX);
    let mut full = Engine::restore(&world, full).unwrap();
    assert!(offered(&full).contains(&(buy("eels", 1), false)));
    assert!(matches!(
        full.execute(buy("eels", 1)),
        Err(EngineError::NumericLimit)
    ));
}

#[test]
fn trading_back_and_forth_never_makes_money() {
    // Even for the best trader, whose spread is narrowest.
    let world = unlimited();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let mut rich = engine.snapshot();
    rich.state.economy.as_mut().unwrap().currency = 100_000_000;
    best_trader(&mut rich.state);
    engine = Engine::restore(&world, rich).unwrap();
    for place in ["greyford", "ashmere", "greyford"] {
        if engine.state().player.location != place {
            engine.execute(Travel(place.into())).unwrap();
        }
        for good in ["grain", "wool", "cloth", "eels"] {
            // Single round trips, and whole stacks, down at the lowest prices too.
            for units in [1, 10, 200, 1_000] {
                let before = currency(&engine);
                engine.execute(buy(good, units)).unwrap();
                engine.execute(sell(good, units)).unwrap();
                assert!(currency(&engine) <= before, "{good} ×{units} at {place}");
            }
        }
    }
}

#[test]
fn selling_first_and_buying_back_never_makes_money_either() {
    // A dear good, so rounding cannot hide a profit of a few units.
    let mut world = unlimited();
    world.world.economy.as_mut().unwrap().goods[3].price = PRICE_BOUND;
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    let mut stocked = engine.snapshot();
    best_trader(&mut stocked.state);
    stocked.state.economy.as_mut().unwrap().currency = CURRENCY_BOUND / 2;
    stocked.state.player.inventory.insert("eels".into(), 2_000);
    // Greyford's 15% spread, narrowed to 13% by the best trader, near the
    // bottom of the index, is the tightest margin: with a gentler buy than
    // sell step, selling one unit at 150 and buying it straight back would pay.
    let greyford = stocked
        .state
        .economy
        .as_mut()
        .unwrap()
        .prices
        .get_mut("greyford");
    greyford.unwrap().insert("eels".into(), 150);
    engine = Engine::restore(&world, stocked).unwrap();
    for units in [1, 2, 10, 200, 1_000] {
        for sell_first in [true, false] {
            let before = currency(&engine);
            let (first, second) = match sell_first {
                true => (sell("eels", units), buy("eels", units)),
                false => (buy("eels", units), sell("eels", units)),
            };
            engine.execute(first).unwrap();
            engine.execute(second).unwrap();
            assert!(
                currency(&engine) <= before,
                "{units} units, selling first: {sell_first}"
            );
        }
    }
}

#[test]
fn a_trade_that_empties_the_conversation_ends_it() {
    let mut world = marches();
    // The reeve speaks only to someone carrying eels.
    let reeve = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "reeve")
        .unwrap();
    for choice in &mut reeve.nodes[0].choices {
        choice.requires = Some(Condition::Item {
            item: "eels".into(),
            quantity: 1,
        });
    }
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Travel("ashmere".into())).unwrap();
    engine.execute(buy("eels", 1)).unwrap();
    engine.execute(Talk("reeve".into())).unwrap();
    assert!(engine.state().dialogue.is_some());
    let events = engine.execute(sell("eels", 1)).unwrap();
    assert!(events.contains(&Event::DialogueEnded));
    assert_eq!(engine.state().dialogue, None);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
}

#[test]
fn wares_sell_at_a_fixed_price_and_gear_arrives_as_pieces() {
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(engine.ware_price("healing_draught"), Some(8));
    assert_eq!(engine.ware_price("rat_tail"), None);
    let events = engine.execute(buy("healing_draught", 1)).unwrap();
    assert!(events.contains(&Event::Bought {
        good: "healing_draught".into(),
        quantity: 1,
        cost: 8
    }));
    assert_eq!(currency(&engine), 2);
    // A fixed price: the next one costs the same, and none can be sold back.
    assert_eq!(engine.ware_price("healing_draught"), Some(8));
    assert!(matches!(
        engine.execute(sell("healing_draught", 1)),
        Err(EngineError::NotTraded(_))
    ));
    assert!(matches!(
        engine.execute(buy("healing_draught", 1)),
        Err(EngineError::NotEnoughCurrency)
    ));
    // Two pieces of mail are two pieces, and the money leaves once.
    let mut rich = arena();
    rich.world.economy.as_mut().unwrap().currency.start = 200;
    let mut engine = Engine::new_with_seed(&rich, 7).unwrap();
    let before: Vec<u64> = combat(&engine).gear.keys().copied().collect();
    engine.execute(buy("iron_mail", 2)).unwrap();
    assert_eq!(currency(&engine), 80);
    let new: Vec<_> = combat(&engine)
        .gear
        .iter()
        .filter(|(id, _)| !before.contains(id))
        .collect();
    assert_eq!(new.len(), 2);
    assert!(new
        .iter()
        .all(|(_, g)| g.item == "iron_mail" && !g.equipped));
    assert_eq!(*new[1].0, *new[0].0 + 1);
    Engine::restore(&rich, engine.snapshot()).unwrap();
}

#[test]
fn wares_are_offered_only_at_an_open_market() {
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert!(offered(&engine).contains(&(buy("healing_draught", 1), true)));
    assert!(offered(&engine).contains(&(buy("iron_mail", 1), false)));
    engine.execute(Move(Direction::East)).unwrap();
    assert!(!offered(&engine)
        .iter()
        .any(|(c, _)| *c == buy("healing_draught", 1)));
    assert!(matches!(
        engine.execute(buy("healing_draught", 1)),
        Err(EngineError::NoMarket)
    ));
    // A market whose merchant is away sells no wares either.
    let mut world = marches();
    let greyford = world
        .world
        .economy
        .as_mut()
        .unwrap()
        .markets
        .iter_mut()
        .find(|m| m.location == "greyford")
        .unwrap();
    greyford.wares.push(Ware {
        item: "keep_token".into(),
        price: 5,
    });
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(engine.ware_price("keep_token"), Some(5));
    engine.execute(Wait(15 * 60)).unwrap(); // 23:00: Hild has gone home
    assert_eq!(engine.ware_price("keep_token"), None);
    assert!(matches!(
        engine.execute(buy("keep_token", 1)),
        Err(EngineError::NotHere(_))
    ));
}

#[test]
fn a_ware_won_as_loot_cannot_vanish_from_a_save() {
    // Wares can only be bought, so a piece a defeat granted is still owed:
    // the ogre's reward stays in the save even though the gate sells mail.
    let mut world = arena();
    let combat = world.world.combat.as_mut().unwrap();
    combat
        .groups
        .iter_mut()
        .find(|g| g.id == "warren")
        .unwrap()
        .repeatable = false;
    combatant(&mut world, "rat").loot = vec![ItemStack {
        item: "iron_mail".into(),
        quantity: 1,
    }];
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Move(Direction::East)).unwrap();
    engine.execute(Engage("rat".into())).unwrap();
    fight_out(&mut engine);
    let mut snapshot = engine.snapshot();
    Engine::restore(&world, snapshot.clone()).unwrap();
    let gear = &mut snapshot.state.combat.as_mut().unwrap().gear;
    let mail = *gear.iter().find(|(_, g)| g.item == "iron_mail").unwrap().0;
    gear.remove(&mail);
    assert!(Engine::restore(&world, snapshot).is_err());
}

#[test]
fn gear_wares_are_bought_at_most_a_grant_at_a_time() {
    // Validation caps any grant of equipment at GEAR_STACK_BOUND pieces;
    // buying is a grant too, so a mistyped count cannot flood the pack.
    let mut rich = arena();
    rich.world.economy.as_mut().unwrap().currency.start = CURRENCY_BOUND;
    let mut engine = Engine::new_with_seed(&rich, 7).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(buy("iron_mail", GEAR_STACK_BOUND + 1)),
        Err(EngineError::InvalidQuantity)
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(buy("iron_mail", GEAR_STACK_BOUND)).unwrap();
    // Counted wares are bounded by trading's own limit only.
    engine.execute(buy("healing_draught", 1_000)).unwrap();
}
