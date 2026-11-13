//! Conditional trade charges are derived; prices and saves have no levy state.
use realmkit_engine::{Command, Engine, Event};
use realmkit_spec::WorldSpec;

fn world() -> WorldSpec {
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/marches"
    ))
    .unwrap();
    let economy = world.world.economy.as_mut().unwrap();
    economy.stock = None;
    economy.markets[0] = serde_json::from_value({
        let mut value = serde_json::to_value(&economy.markets[0]).unwrap();
        value["levies"] = serde_json::json!([
            {"id":"handling", "text":"League handling charge", "requires":{"kind":"flag","flag":"letter_delivered"}, "percent":10},
            {"id":"wool_fee", "text":"Wool charge", "requires":{"kind":"flag","flag":"letter_delivered"}, "percent":15, "goods":["wool"]}
        ]);
        value
    }).unwrap();
    world
}

#[test]
fn levies_change_quotes_charges_and_availability_but_not_indices() {
    let world = world();
    let engine = Engine::new_with_seed(&world, 7).unwrap();
    let before = engine.quote("wool").unwrap();
    assert!(before.levies.is_empty());
    let mut save = engine.snapshot();
    save.state.flags.insert("letter_delivered".into());
    let mut engine = Engine::restore(&world, save).unwrap();
    let quote = engine.quote("wool").unwrap();
    assert_eq!(quote.levies.iter().map(|l| l.percent).sum::<u32>(), 25);
    let prices = engine.state().economy.as_ref().unwrap().prices.clone();
    assert!(quote.buy > before.buy);
    assert!(quote.sell < before.sell);
    assert_eq!(engine.quote("grain").unwrap().levies.len(), 1);
    let events = engine
        .execute(Command::Buy {
            good: "wool".into(),
            quantity: 1,
        })
        .unwrap();
    assert!(
        matches!(&events[0], Event::Bought {cost, levies, ..} if *cost == quote.buy && *levies == quote.levies)
    );
    assert_eq!(engine.quote("wool").unwrap().buy, quote.next_buy);
    let quote = engine.quote("wool").unwrap();
    let events = engine
        .execute(Command::Sell {
            good: "wool".into(),
            quantity: 1,
        })
        .unwrap();
    assert!(
        matches!(&events[0], Event::Sold {earned, levies, ..} if *earned == quote.sell && *levies == quote.levies)
    );
    assert_eq!(engine.state().economy.as_ref().unwrap().prices, prices);
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn levy_prices_match_simulator_with_one_final_rounding() {
    use realmkit_engine::{levied_buy_price, levied_sell_price};
    assert_eq!(levied_buy_price(120, 712, 15, 25), 122);
    assert_eq!(levied_sell_price(120, 1309, 15, 25), 102);
    assert_eq!(levied_buy_price(1, 100, 0, 10), 1);
    assert_eq!(levied_sell_price(120, 1309, 15, 100), 0);
}

#[test]
fn a_sale_tests_conditions_before_taking_the_cargo() {
    let mut world = world();
    let levy = &mut world.world.economy.as_mut().unwrap().markets[0].levies[0];
    levy.requires = realmkit_spec::Condition::Item {
        item: "wool".into(),
        quantity: 1,
    };
    let engine = Engine::new_with_seed(&world, 7).unwrap();
    let mut save = engine.snapshot();
    save.state.player.inventory.insert("wool".into(), 1);
    let mut engine = Engine::restore(&world, save).unwrap();
    let quote = engine.quote("wool").unwrap();
    assert_eq!(quote.levies.len(), 1);
    let events = engine
        .execute(Command::Sell {
            good: "wool".into(),
            quantity: 1,
        })
        .unwrap();
    assert!(
        matches!(&events[0], Event::Sold { earned, levies, .. } if *earned == quote.sell && *levies == quote.levies)
    );
    assert!(engine.quote("wool").unwrap().levies.is_empty());
}

#[test]
fn affordability_uses_the_levied_quote_and_refusal_is_atomic() {
    let world = world();
    let engine = Engine::new_with_seed(&world, 7).unwrap();
    let mut save = engine.snapshot();
    save.state.flags.insert("letter_delivered".into());
    let engine = Engine::restore(&world, save).unwrap();
    let price = engine.quote("wool").unwrap().buy;
    let mut save = engine.snapshot();
    save.state.economy.as_mut().unwrap().currency = price - 1;
    let mut engine = Engine::restore(&world, save).unwrap();
    let buy = Command::Buy {
        good: "wool".into(),
        quantity: 1,
    };
    assert!(
        !engine
            .actions()
            .iter()
            .find(|a| a.command == buy)
            .unwrap()
            .available
    );
    let before = engine.snapshot();
    assert!(engine.execute(buy).is_err());
    assert_eq!(before, engine.snapshot());
}

#[test]
fn levy_activation_does_not_change_price_tick_evolution() {
    let world = world();
    let engine = Engine::new_with_seed(&world, 7).unwrap();
    let mut save = engine.snapshot();
    save.state.flags.insert("letter_delivered".into());
    let mut charged = Engine::restore(&world, save).unwrap();
    let mut plain = Engine::new_with_seed(&world, 7).unwrap();
    for _ in 0..3 {
        charged.execute(Command::PassTime(1440)).unwrap();
        plain.execute(Command::PassTime(1440)).unwrap();
        assert_eq!(charged.state().economy, plain.state().economy);
        assert_eq!(charged.state().rng, plain.state().rng);
    }
}

#[test]
fn ford_levy_follows_current_control_without_time_or_saved_levies() {
    use realmkit_spec::{Condition, Effect, Schedule, WorldEvent};
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/turning-ford"
    ))
    .unwrap();
    world.world.events.push(WorldEvent {
        id: "retake".into(),
        schedule: Schedule { at: 1, every: None },
        requires: None,
        effects: vec![Effect::SetControl {
            location: "ford".into(),
            faction: "stone".into(),
        }],
    });
    let mut engine = Engine::new(&world).unwrap();
    assert_eq!(engine.quote("grain").unwrap().buy, 20);
    engine.execute(Command::Talk("envoy".into())).unwrap();
    engine.execute(Command::ChooseDialogue(1)).unwrap();
    assert!(engine.holds(&Condition::Controls {
        location: "ford".into(),
        faction: "reed".into()
    }));
    let quote = engine.quote("grain").unwrap();
    assert_eq!((quote.buy, quote.sell), (22, 18));
    assert_eq!(quote.levies[0].id, "reed_handling");
    assert_eq!(engine.state().time, Some(0));
    let encoded = serde_json::to_string(&engine.snapshot()).unwrap();
    assert!(!encoded.contains("levies"));
    engine.execute(Command::PassTime(1)).unwrap();
    assert_eq!(engine.quote("grain").unwrap().buy, 20);
    assert!(engine.quote("grain").unwrap().levies.is_empty());
}
