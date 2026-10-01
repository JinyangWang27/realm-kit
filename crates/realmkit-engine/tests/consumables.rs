//! Using items that restore HP and MP, exploring and as a fight turn.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn draughts(engine: &Engine<'_>) -> u64 {
    engine
        .state()
        .player
        .inventory
        .get("healing_draught")
        .copied()
        .unwrap_or(0)
}

fn buy_draughts(quantity: u64) -> Command {
    Buy {
        good: "healing_draught".into(),
        quantity,
    }
}

/// The arena with enough marks for a few draughts (it starts with 10).
fn arena_with_marks(marks: u64) -> WorldSpec {
    let mut world = arena();
    world.world.economy.as_mut().unwrap().currency.start = marks;
    world
}

/// Bitten by the wolf pack, which strikes before the player's first turn,
/// then fled back to the gate.
fn bitten(engine: &mut Engine<'_>) {
    engine.execute(Move(Direction::North)).unwrap();
    engine.execute(Engage("grey_wolf".into())).unwrap();
    engine.execute(Flee).unwrap();
    engine.execute(Move(Direction::South)).unwrap();
    assert!(vitals(engine).hp < engine.player_stats().unwrap().hp);
}

/// An arena player carrying `n` draughts, hurt by the wolves.
/// `world` needs at least `8 × n` starting marks.
fn hurt_with_draughts(world: &WorldSpec, n: u64) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    engine.execute(buy_draughts(n)).unwrap();
    bitten(&mut engine);
    engine
}

#[test]
fn a_draught_restores_hp_up_to_the_maximum_and_is_spent() {
    let world = arena();
    let mut engine = hurt_with_draughts(&world, 1);
    let max = engine.player_stats().unwrap().hp;
    let before = vitals(&engine).hp;
    let events = engine.execute(Use("healing_draught".into())).unwrap();
    let gained = (max - before).min(30);
    assert_eq!(
        events,
        [Event::Consumed {
            item: "healing_draught".into(),
            hp: gained,
            mp: 0
        }]
    );
    assert_eq!(vitals(&engine).hp, before + gained);
    assert_eq!(draughts(&engine), 0);
    assert!(!engine
        .state()
        .player
        .inventory
        .contains_key("healing_draught"));
}

#[test]
fn using_is_refused_when_it_would_restore_nothing_or_nothing_is_carried() {
    let mut world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(buy_draughts(1)).unwrap();
    let before = engine.state().clone();
    // Full health: nothing to restore, and nothing is spent.
    assert!(matches!(
        engine.execute(Use("healing_draught".into())),
        Err(EngineError::NothingToRestore)
    ));
    assert_eq!(engine.state(), &before);
    assert!(matches!(
        engine.execute(Use("rat_tail".into())),
        Err(EngineError::NotConsumable(_))
    ));
    assert!(matches!(
        engine.execute(Use("missing".into())),
        Err(EngineError::NotConsumable(_))
    ));
    // An MP-only draught in a world whose player has no MP restores nothing.
    world
        .items
        .iter_mut()
        .find(|i| i.id == "healing_draught")
        .unwrap()
        .consumable = Some(Consumable { hp: 0, mp: 10 });
    let mut engine = hurt_with_draughts(&world, 1);
    assert_eq!(engine.player_stats().unwrap().mp, 0);
    assert!(matches!(
        engine.execute(Use("healing_draught".into())),
        Err(EngineError::NothingToRestore)
    ));
    // None carried.
    let plain = arena();
    let mut engine = hurt_with_draughts(&plain, 1);
    engine.execute(Use("healing_draught".into())).unwrap();
    assert!(matches!(
        engine.execute(Use("healing_draught".into())),
        Err(EngineError::NotEnoughMaterials(_))
    ));
}

#[test]
fn using_in_a_fight_spends_the_turn_and_the_opponents_act() {
    let world = arena_with_marks(100);
    let fight = |world| {
        let mut engine = hurt_with_draughts(world, 2);
        engine.execute(Move(Direction::North)).unwrap();
        engine.execute(Engage("grey_wolf".into())).unwrap();
        engine
    };
    let mut engine = fight(&world);
    let now = engine.encounter().unwrap().now;
    let events = engine.execute(Use("healing_draught".into())).unwrap();
    assert!(matches!(events[0], Event::Consumed { .. }));
    // The wolves answered before the player's next turn, one basic action later.
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::DamageReceived { .. })));
    assert!(engine.encounter().unwrap().now > now);
    assert_eq!(draughts(&engine), 1);
    // Replays are exact.
    let mut again = fight(&world);
    assert_eq!(
        again.execute(Use("healing_draught".into())).unwrap(),
        events
    );
    assert_eq!(again.state(), engine.state());
    // A save in the middle loads.
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn draughts_are_offered_when_carried_and_unavailable_at_full_health() {
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Use(_))));
    engine.execute(buy_draughts(1)).unwrap();
    assert!(offered(&engine).contains(&(Use("healing_draught".into()), false)));
    let engine = hurt_with_draughts(&world, 1);
    assert!(offered(&engine).contains(&(Use("healing_draught".into()), true)));
}

#[test]
fn a_save_with_granted_consumables_eaten_loads() {
    // Loot that is consumable may be eaten away below what the defeat
    // granted. Make the draught the rat's only, once-only loot: not a ware,
    // not a good, not a repeatable group's loot, since those already make
    // any count acceptable and would hide the rule under test.
    let mut world = arena();
    let economy = world.world.economy.as_mut().unwrap();
    economy.markets[0].wares.clear();
    economy.goods.clear();
    let combat = world.world.combat.as_mut().unwrap();
    combat
        .groups
        .iter_mut()
        .find(|g| g.id == "warren")
        .unwrap()
        .repeatable = false;
    combatant(&mut world, "rat").loot = vec![ItemStack {
        item: "healing_draught".into(),
        quantity: 1,
    }];
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    bitten(&mut engine);
    engine.execute(Move(Direction::East)).unwrap();
    engine.execute(Engage("rat".into())).unwrap();
    fight_out(&mut engine);
    assert_eq!(draughts(&engine), 1);
    engine.execute(Use("healing_draught".into())).unwrap();
    assert_eq!(draughts(&engine), 0);
    Engine::restore(&world, engine.snapshot()).unwrap();
}
