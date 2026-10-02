//! Prosperity, merchants' stock, workshops and the trading proficiency,
//! whose numbers come from
//! `python3 -m scripts.combat_sim economy examples/marches --seed 7`.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn wallet<'e>(engine: &'e Engine<'_>) -> &'e EconomyState {
    engine.state().economy.as_ref().unwrap()
}

fn buy(good: &str, quantity: u64) -> Command {
    Buy {
        good: good.into(),
        quantity,
    }
}

fn sell(good: &str, quantity: u64) -> Command {
    Sell {
        good: good.into(),
        quantity,
    }
}

const MARKETS: [&str; 3] = ["greyford", "ashmere", "vellmarket"];
const GOODS: [&str; 4] = ["grain", "wool", "cloth", "eels"];

/// Each market's prosperity, stock of each good and purse.
fn shelves(engine: &Engine<'_>) -> Vec<(u32, [u64; 4], u64)> {
    let wallet = wallet(engine);
    MARKETS
        .iter()
        .map(|m| {
            let stock = &wallet.stock[*m];
            (
                wallet.prosperity[*m],
                GOODS.map(|g| stock.goods[g]),
                stock.currency,
            )
        })
        .collect()
}

/// A new playthrough moved on to just after the nth daily price tick.
fn after_ticks(world: &WorldSpec, ticks: u64) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    // The first tick is at minute 1,440; play starts at 480.
    engine.execute(Wait(1_440 * ticks - 480)).unwrap();
    engine
}

/// A playthrough at `location` holding `currency`, as a save could have it.
fn standing<'w>(world: &'w WorldSpec, location: &str, currency: u64) -> Engine<'w> {
    let mut snapshot = Engine::new_with_seed(world, 7).unwrap().snapshot();
    snapshot.state.player.location = location.into();
    snapshot.state.economy.as_mut().unwrap().currency = currency;
    Engine::restore(world, snapshot).unwrap()
}

#[test]
fn prosperity_and_restocking_match_the_simulator() {
    let world = marches();
    // New Game stocks every market at its targets, drawing nothing:
    // Greyford makes grain and is fed Ashmere's grain and eels.
    let engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(
        shelves(&engine)[..2],
        [(48, [20, 0, 0, 18], 588), (50, [16, 0, 0, 23], 600)]
    );
    assert_eq!(
        shelves(&after_ticks(&world, 1)),
        [
            (47, [35, 0, 0, 29], 582),
            (50, [16, 0, 0, 26], 600),
            (41, [0, 35, 17, 0], 546)
        ]
    );
    // Vellmarket lacks grain and Greyford cloth, so both decline a point a day.
    assert_eq!(
        shelves(&after_ticks(&world, 3)),
        [
            (45, [35, 0, 0, 24], 570),
            (50, [9, 0, 0, 19], 600),
            (40, [0, 18, 16, 0], 540)
        ]
    );
}

#[test]
fn a_world_without_the_new_parts_keeps_its_old_prices() {
    // The Format 14 marches: no prosperity, stock, workshops or trading, no
    // village feeding its town, and the old starting prices. The tick's
    // numbers are exactly the ones pinned before these parts existed.
    let mut world = marches();
    world.dialogues.retain(|d| d.id != "maddoc");
    for character in &mut world.characters {
        if character.id == "maddoc" {
            character.dialogue = None;
        }
    }
    let combat = world.world.combat.as_mut().unwrap();
    combat
        .levels
        .iter_mut()
        .for_each(|l| l.proficiency_points = 0);
    let economy = world.world.economy.as_mut().unwrap();
    (economy.prosperity, economy.stock) = (None, None);
    (economy.workshops, economy.trading) = (None, None);
    let old = [
        [760, 1086, 1309, 728],
        [586, 1052, 1232, 517],
        [1601, 745, 712, 1301],
    ];
    for (market, prices) in economy.markets.iter_mut().zip(old) {
        (market.prosperity, market.town) = (None, None);
        market.prices = GOODS.iter().map(|g| g.to_string()).zip(prices).collect();
    }
    let engine = after_ticks(&world, 3);
    let prices = &wallet(&engine).prices;
    assert_eq!(
        MARKETS.map(|m| GOODS.map(|g| prices[m][g])),
        [
            [739, 1102, 1300, 741],
            [576, 1060, 1242, 557],
            [1646, 738, 698, 1310]
        ]
    );
    assert!(!world.random_stock());
    assert_eq!(engine.state().rng.unwrap().stock, None);
}

#[test]
fn restocking_draws_its_own_stream_and_never_moves_prices() {
    let world = marches();
    let mut unstocked = marches();
    unstocked.world.economy.as_mut().unwrap().stock = None;
    let prices = |world: &WorldSpec| wallet(&after_ticks(world, 5)).prices.clone();
    assert_eq!(prices(&world), prices(&unstocked));
}

#[test]
fn merchants_sell_what_they_hold_and_buy_what_their_purse_covers() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    // Greyford holds 20 grain: the 21st is refused, and nothing changes.
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(buy("grain", 21)),
        Err(EngineError::OutOfStock(good)) if good == "grain"
    ));
    assert_eq!(engine.state(), &before);
    let events = engine.execute(buy("grain", 5)).unwrap();
    let Event::Bought { cost, .. } = events[0] else {
        panic!("{events:?}");
    };
    let greyford = &wallet(&engine).stock["greyford"];
    assert_eq!(
        (greyford.goods["grain"], greyford.currency),
        (15, 588 + cost)
    );
    // A sale moves units back onto the shelf and coins out of the purse.
    let events = engine.execute(sell("grain", 2)).unwrap();
    let Event::Sold { earned, .. } = events[0] else {
        panic!("{events:?}");
    };
    let greyford = &wallet(&engine).stock["greyford"];
    assert_eq!(
        (greyford.goods["grain"], greyford.currency),
        (17, 588 + cost - earned)
    );
    // Wool is sold out here, so buying it is listed but unavailable.
    assert!(offered(&engine).contains(&(buy("wool", 1), false)));
    assert_eq!(engine.quote("wool").unwrap().stock, Some(0));
    // An empty purse buys nothing.
    let mut broke = engine.snapshot();
    let stock = broke
        .state
        .economy
        .as_mut()
        .unwrap()
        .stock
        .get_mut("greyford");
    stock.unwrap().currency = 3;
    let mut broke = Engine::restore(&world, broke).unwrap();
    assert!(offered(&broke).contains(&(sell("grain", 1), false)));
    let before = broke.state().clone();
    assert!(matches!(
        broke.execute(sell("grain", 1)),
        Err(EngineError::MerchantCannotPay)
    ));
    assert_eq!(broke.state(), &before);
    // The next restock refills the purse, whatever the trade left in it.
    engine.execute(Wait(1_440 - 480)).unwrap();
    let (prosperity, _, purse) = shelves(&engine)[0];
    assert_eq!((prosperity, purse), (47, 582));
}

#[test]
fn a_weavery_bought_from_maddoc_earns_at_the_weekly_settlement() {
    let world = marches();
    let mut engine = standing(&world, "vellmarket", 1_000);
    engine.execute(Talk("maddoc".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::WorkshopBought {
        workshop: "weavery".into(),
        location: "vellmarket".into(),
        cost: 150,
    }));
    assert_eq!(wallet(&engine).currency, 850);
    assert_eq!(wallet(&engine).workshops["vellmarket"]["weavery"], 1);
    // Owning one hides the offer and shows the buy-back instead.
    engine.execute(Talk("maddoc".into())).unwrap();
    assert_eq!(
        engine.dialogue_choices(),
        ["Take the weavery back. (75 silver)", "Just trading."]
    );
    assert!(engine.holds(&Condition::Workshop {
        workshop: "weavery".into(),
        location: Some("vellmarket".into()),
    }));
    assert!(!engine.holds(&Condition::Workshop {
        workshop: "weavery".into(),
        location: Some("greyford".into()),
    }));
    // The first settlement is at minute 10,080, after that day's price tick:
    // 6 cloth less 9 wool less 120 overhead at Vellmarket's prices.
    let events = engine.execute(Wait(10_080 - 480)).unwrap();
    assert!(
        events.contains(&Event::WorkshopsEarned { amount: 115 }),
        "{events:?}"
    );
    let earned = events
        .iter()
        .filter(|e| matches!(e, Event::WorkshopsEarned { .. }))
        .count();
    assert_eq!(earned, 1);
    // Selling it back pays the resale.
    engine.execute(Talk("maddoc".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::WorkshopSold {
        workshop: "weavery".into(),
        location: "vellmarket".into(),
        earned: 75,
    }));
    assert!(wallet(&engine).workshops.is_empty());
    assert_eq!(wallet(&engine).currency, 850 + 115 + 75);
}

#[test]
fn a_losing_workshop_takes_what_the_player_has_and_reports_the_rest() {
    let mut world = marches();
    let economy = world.world.economy.as_mut().unwrap();
    economy.workshops.as_mut().unwrap().kinds[0].overhead = 1_000;
    let mut engine = standing(&world, "vellmarket", 200);
    engine.execute(Talk("maddoc".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    let events = engine.execute(Wait(10_080 - 480)).unwrap();
    // 1,000 overhead against 115 + 120 earned: 765 lost, of which the
    // 50 silver left after buying it is all that can be paid.
    assert!(
        events.contains(&Event::WorkshopsLost {
            amount: 50,
            shortfall: 715,
        }),
        "{events:?}"
    );
    assert_eq!(wallet(&engine).currency, 0);
}

#[test]
fn workshops_are_bought_in_towns_within_their_limit() {
    let mut world = marches();
    // The reeve and Maddoc sell weaveries with no conditions.
    for id in ["reeve", "maddoc"] {
        let dialogue = world.dialogues.iter_mut().find(|d| d.id == id).unwrap();
        dialogue.nodes[0].choices.insert(
            0,
            DialogueChoice {
                text: "Buy a weavery.".into(),
                next: None,
                requires: None,
                effects: vec![Effect::BuyWorkshop {
                    workshop: "weavery".into(),
                }],
            },
        );
    }
    // Ashmere is a village.
    let mut engine = standing(&world, "ashmere", 1_000);
    engine.execute(Talk("reeve".into())).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(ChooseDialogue(1)),
        Err(EngineError::NoWorkshopHere)
    ));
    assert_eq!(engine.state(), &before);
    // One per town.
    let mut engine = standing(&world, "vellmarket", 1_000);
    engine.execute(Talk("maddoc".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Talk("maddoc".into())).unwrap();
    assert!(matches!(
        engine.execute(ChooseDialogue(1)),
        Err(EngineError::WorkshopLimit)
    ));
    // And never on credit.
    let mut engine = standing(&world, "vellmarket", 149);
    engine.execute(Talk("maddoc".into())).unwrap();
    assert!(matches!(
        engine.execute(ChooseDialogue(1)),
        Err(EngineError::NotEnoughCurrency)
    ));
}

#[test]
fn trading_is_trained_with_level_points_and_narrows_the_spread() {
    let world = marches();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    // Level 1 grants one point; Greyford grain at 533 with a 15% spread.
    assert_eq!(engine.unspent_proficiency_points(), 1);
    assert_eq!(engine.quote("grain").unwrap().buy, buy_price(20, 533, 15));
    let train = Train {
        proficiency: Proficiency::Trading,
        points: 1,
    };
    assert!(offered(&engine).contains(&(train.clone(), true)));
    let events = engine.execute(train.clone()).unwrap();
    assert_eq!(
        events,
        [Event::ProficiencyTrained {
            proficiency: Proficiency::Trading,
            rank: 1
        }]
    );
    assert_eq!(engine.proficiency_rank(Proficiency::Trading), 1);
    // 15% narrowed by 4% is 14.4%, rounded down to 14%.
    let quote = engine.quote("grain").unwrap();
    assert_eq!(
        (quote.buy, quote.sell),
        (buy_price(20, 533, 14), sell_price(20, 533, 14))
    );
    assert!(!offered(&engine)
        .iter()
        .any(|(c, _)| matches!(c, Train { .. })));
    assert!(matches!(
        engine.execute(train.clone()),
        Err(EngineError::NotEnoughPoints)
    ));
    // Ranks taught on top of trained ones stop at the top rank.
    let mut taught = engine.snapshot();
    taught.state.combat.as_mut().unwrap().level = 2;
    taught.state.combat.as_mut().unwrap().xp = 30;
    let held = taught
        .state
        .proficiencies
        .get_mut(&Proficiency::Trading)
        .unwrap();
    held.taught = 2;
    let mut taught = Engine::restore(&world, taught).unwrap();
    assert_eq!(taught.unspent_proficiency_points(), 1);
    assert!(matches!(
        taught.execute(train),
        Err(EngineError::ProficiencyCap)
    ));
}

#[test]
fn an_effect_teaches_trading_and_a_condition_reads_it() {
    let mut world = marches();
    let wenna = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "wenna")
        .unwrap();
    wenna.nodes[0].choices.insert(
        0,
        DialogueChoice {
            text: "Teach me to haggle.".into(),
            next: None,
            requires: Some(Condition::Not {
                condition: Box::new(Condition::Proficiency {
                    proficiency: Proficiency::Trading,
                    rank: 3,
                }),
            }),
            effects: vec![Effect::RaiseProficiency {
                proficiency: Proficiency::Trading,
                ranks: 2,
            }],
        },
    );
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("wenna".into())).unwrap();
    let events = engine.execute(ChooseDialogue(1)).unwrap();
    assert!(events.contains(&Event::ProficiencyRaised {
        proficiency: Proficiency::Trading,
        rank: 2
    }));
    // Teaching spends no points: the level's point still trains rank 3.
    engine
        .execute(Train {
            proficiency: Proficiency::Trading,
            points: 1,
        })
        .unwrap();
    assert!(engine.holds(&Condition::Proficiency {
        proficiency: Proficiency::Trading,
        rank: 3
    }));
    engine.execute(Talk("wenna".into())).unwrap();
    assert_eq!(engine.dialogue_choices()[0], "Tell me about the fen.");
    // A second lesson would pass the top rank, so it is refused whole.
    let mut world = world.clone();
    let wenna = world
        .dialogues
        .iter_mut()
        .find(|d| d.id == "wenna")
        .unwrap();
    wenna.nodes[0].choices[0].requires = None;
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Talk("wenna".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Talk("wenna".into())).unwrap();
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(ChooseDialogue(1)),
        Err(EngineError::ProficiencyCap)
    ));
    assert_eq!(engine.state(), &before);
}

#[test]
fn a_save_keeps_the_new_economy_state_and_rejects_impossible_ones() {
    let world = marches();
    let mut engine = standing(&world, "vellmarket", 1_000);
    engine
        .execute(Train {
            proficiency: Proficiency::Trading,
            points: 1,
        })
        .unwrap();
    engine.execute(Talk("maddoc".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(buy("wool", 3)).unwrap();
    engine.execute(Wait(2 * 1_440)).unwrap();
    let good = engine.snapshot();
    let json = serde_json::to_string(&good).unwrap();
    let mut resumed = Engine::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(
        resumed.execute(Wait(8 * 1_440)).unwrap(),
        engine.execute(Wait(8 * 1_440)).unwrap()
    );
    assert_eq!(resumed.state(), engine.state());

    type Corrupt = fn(&mut GameState);
    let corruptions: Vec<Corrupt> = vec![
        |s| {
            s.economy
                .as_mut()
                .unwrap()
                .prosperity
                .insert("ashmere".into(), 101);
        },
        |s| {
            s.economy.as_mut().unwrap().prosperity.remove("ashmere");
        },
        |s| {
            let stock = s
                .economy
                .as_mut()
                .unwrap()
                .stock
                .get_mut("ashmere")
                .unwrap();
            stock.goods.remove("eels");
        },
        |s| {
            let stock = s
                .economy
                .as_mut()
                .unwrap()
                .stock
                .get_mut("ashmere")
                .unwrap();
            stock.goods.insert("eels".into(), STOCK_BOUND + 1);
        },
        |s| {
            s.economy.as_mut().unwrap().stock.remove("greyford");
        },
        |s| {
            let workshops = &mut s.economy.as_mut().unwrap().workshops;
            workshops.insert("ashmere".into(), [("weavery".into(), 1)].into());
        },
        |s| {
            let workshops = &mut s.economy.as_mut().unwrap().workshops;
            workshops.insert("vellmarket".into(), [("weavery".into(), 2)].into());
        },
        |s| {
            let workshops = &mut s.economy.as_mut().unwrap().workshops;
            workshops.insert("greyford".into(), [("tannery".into(), 1)].into());
        },
        |s| {
            let workshops = &mut s.economy.as_mut().unwrap().workshops;
            workshops.insert("greyford".into(), [("weavery".into(), 0)].into());
        },
        |s| {
            let held = s.proficiencies.get_mut(&Proficiency::Trading).unwrap();
            held.taught = 3;
        },
        |s| {
            let held = s.proficiencies.get_mut(&Proficiency::Trading).unwrap();
            held.trained = 2;
        },
        |s| {
            let held = s.proficiencies.get_mut(&Proficiency::Trading).unwrap();
            held.trained = 0;
        },
        |s| s.rng.as_mut().unwrap().stock = None,
    ];
    for (i, corrupt) in corruptions.into_iter().enumerate() {
        let mut snapshot = good.clone();
        corrupt(&mut snapshot.state);
        assert!(
            matches!(
                Engine::restore(&world, snapshot),
                Err(EngineError::InvalidSave(_))
            ),
            "corruption {i} was accepted"
        );
    }
}
