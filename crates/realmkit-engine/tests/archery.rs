//! Drawn weapons and projectiles in real-time fights, at the archery range:
//! the draw and its force, flight that outlives the action that loosed it,
//! every way a projectile ends, exactly once, interruption, the end of a
//! fight, saves and replays.

use realmkit_engine::{Command::*, Intent as I, *};
use realmkit_spec::*;

const RANGE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/archery-range");
const HALL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/fencing-hall");

fn range() -> WorldSpec {
    WorldSpec::load(RANGE).unwrap()
}

fn step(n: u32) -> Command {
    Intent(I::Advance(n))
}

fn hit(projectile: u64, target: &str) -> Command {
    Intent(I::ProjectileContact {
        projectile,
        target: target.into(),
    })
}

fn spent(projectile: u64) -> Command {
    Intent(I::ProjectileSpent { projectile })
}

fn run(engine: &mut Engine<'_>, commands: Vec<Command>) -> Vec<Event> {
    commands
        .into_iter()
        .flat_map(|c| engine.execute(c).unwrap())
        .collect()
}

fn start<'w>(world: &'w WorldSpec, foe: &str) -> Engine<'w> {
    let mut engine = Engine::new(world).unwrap();
    engine.execute(EngageRealtime(foe.into())).unwrap();
    engine
}

/// Draws for `steps` and looses, returning what loosing reported.
fn shoot(engine: &mut Engine<'_>, steps: u32) -> Vec<Event> {
    engine.execute(Intent(I::Draw)).unwrap();
    engine.execute(step(steps)).unwrap();
    engine.execute(Intent(I::Loose)).unwrap()
}

fn action(engine: &Engine<'_>, i: usize) -> RealtimeAction {
    engine.realtime().unwrap().fighters[i].action.clone()
}

fn hp(engine: &Engine<'_>, i: usize) -> u32 {
    engine.fighter_vitals(i).unwrap().hp
}

fn live(engine: &Engine<'_>) -> Vec<u64> {
    let shots = &engine.realtime().unwrap().projectiles;
    shots.iter().map(|p| p.id).collect()
}

fn ended(events: &[Event]) -> Vec<(u64, ProjectileEnd)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::ProjectileEnded {
                projectile, cause, ..
            } => Some((*projectile, *cause)),
            _ => None,
        })
        .collect()
}

#[test]
fn drawing_needs_a_drawn_weapon_worn_and_only_a_draw_can_loose() {
    let hall = WorldSpec::load(HALL).unwrap();
    let mut melee = start(&hall, "duelist");
    assert!(matches!(
        melee.execute(Intent(I::Draw)),
        Err(EngineError::NoRangedWeapon)
    ));
    assert!(melee.player_ranged().is_none());

    let world = range();
    let mut engine = start(&world, "butt");
    let ranged = engine.player_ranged().unwrap();
    assert_eq!((ranged.full, ranged.least), (8, 2));
    for intent in [I::Loose, I::LowerDraw] {
        assert!(matches!(
            engine.execute(Intent(intent)),
            Err(EngineError::NotDrawing)
        ));
    }
    assert!(matches!(
        engine.execute(spent(0)),
        Err(EngineError::NoProjectile(0))
    ));
}

#[test]
fn the_draw_settles_the_shots_force_and_holding_longer_changes_nothing() {
    let world = range();
    let force = |steps: u32| {
        let mut engine = start(&world, "butt");
        let events = shoot(&mut engine, steps);
        let Event::Loosed {
            draw_percent,
            power,
            ..
        } = events[0]
        else {
            panic!("{events:?}");
        };
        let shot = engine.realtime().unwrap().projectiles[0].clone();
        let before = hp(&engine, 1);
        engine.execute(hit(0, "butt")).unwrap();
        (draw_percent, power, shot.impact, before - hp(&engine, 1))
    };
    // 50 power at 2 steps of draw, 200 at 8, evenly between.
    assert_eq!(force(2), (25, 50, 0, 3));
    assert_eq!(force(5), (62, 125, 0, 7));
    assert_eq!(force(8), (100, 200, 1, 12));
    assert_eq!(force(40), force(8), "a held draw keeps its force");
}

#[test]
fn a_draw_reports_full_once_and_a_short_one_can_only_be_lowered() {
    let world = range();
    let mut engine = start(&world, "butt");
    engine.execute(Intent(I::Draw)).unwrap();
    engine.execute(step(1)).unwrap();
    assert!(matches!(
        engine.execute(Intent(I::Loose)),
        Err(EngineError::DrawTooShort)
    ));
    assert!(matches!(action(&engine, 0), RealtimeAction::Drawing { .. }));
    assert!(matches!(
        engine.execute(Intent(I::Draw)),
        Err(EngineError::Committed)
    ));
    let full = |e: &Event| matches!(e, Event::DrawFull { .. });
    let early = engine.execute(step(6)).unwrap();
    assert!(!early.iter().any(full), "7 of 8 steps: {early:?}");
    assert_eq!(
        engine
            .execute(step(1))
            .unwrap()
            .iter()
            .filter(|e| full(e))
            .count(),
        1
    );
    assert!(!engine.execute(step(30)).unwrap().iter().any(full));
    let lowered = engine.execute(Intent(I::LowerDraw)).unwrap();
    assert!(matches!(lowered[0], Event::DrawLowered { .. }));
    assert_eq!(action(&engine, 0), RealtimeAction::Ready);
    assert!(live(&engine).is_empty(), "a lowered draw looses nothing");
}

#[test]
fn archery_spends_no_resource() {
    let mut world = range();
    world.world.combat.as_mut().unwrap().levels[0].stats.mp = 20;
    let mut engine = start(&world, "butt");
    let before = engine.player_vitals().unwrap();
    shoot(&mut engine, 8);
    engine.execute(hit(0, "butt")).unwrap();
    engine.execute(step(3)).unwrap();
    assert_eq!(engine.player_vitals().unwrap(), before);
}

#[test]
fn a_shot_outlives_the_action_that_loosed_it_and_several_fly_at_once() {
    let world = range();
    let mut engine = start(&world, "butt");
    shoot(&mut engine, 8);
    assert!(matches!(action(&engine, 0), RealtimeAction::Loosed { until } if until == 11));
    assert!(matches!(
        engine.execute(Intent(I::Draw)),
        Err(EngineError::Committed)
    ));
    let freed = engine.execute(step(3)).unwrap();
    assert!(freed.contains(&Event::Ready {
        character: "archer".into()
    }));
    // The first still flies while the second is drawn and loosed, and then
    // while a melee blow runs, whose window the landing shots do not touch.
    shoot(&mut engine, 2);
    assert_eq!(live(&engine), [0, 1]);
    engine.execute(step(3)).unwrap();
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    engine.execute(step(3)).unwrap();
    let RealtimeAction::Attack { phase, .. } = action(&engine, 0) else {
        panic!("attacking");
    };
    assert_eq!(phase, AttackPhase::Active);
    let events = run(&mut engine, vec![hit(1, "butt")]);
    assert_eq!(ended(&events), [(1, ProjectileEnd::Landed)]);
    let RealtimeAction::Attack { connected, .. } = action(&engine, 0) else {
        panic!("still attacking");
    };
    assert!(!connected, "a shot landing is not the blow landing");
    assert_eq!(live(&engine), [0]);
}

#[test]
fn each_projectile_ends_exactly_once() {
    let world = range();
    let mut engine = start(&world, "butt");
    shoot(&mut engine, 8);
    let before = engine.snapshot();
    let events = run(&mut engine, vec![hit(0, "butt")]);
    assert_eq!(ended(&events), [(0, ProjectileEnd::Landed)]);
    let struck = hp(&engine, 1);
    for again in [hit(0, "butt"), spent(0)] {
        assert!(matches!(
            engine.execute(again),
            Err(EngineError::NoProjectile(0))
        ));
    }
    assert_eq!(hp(&engine, 1), struck, "no second hit");

    // Spent on scenery: harms nobody, and cannot land afterwards.
    let mut wall = Engine::restore(&world, before.clone()).unwrap();
    let events = run(&mut wall, vec![spent(0)]);
    assert_eq!(ended(&events), [(0, ProjectileEnd::Obstructed)]);
    assert_eq!(hp(&wall, 1), 100);
    assert!(wall.execute(hit(0, "butt")).is_err());

    // Flight 20: live at the last step before it runs out, gone at it.
    let mut late = Engine::restore(&world, before.clone()).unwrap();
    assert!(ended(&late.execute(step(19)).unwrap()).is_empty());
    let mut landing = Engine::restore(&world, late.snapshot()).unwrap();
    landing.execute(hit(0, "butt")).unwrap();
    let events = late.execute(step(1)).unwrap();
    assert_eq!(ended(&events), [(0, ProjectileEnd::Expired)]);
    assert!(matches!(
        late.execute(hit(0, "butt")),
        Err(EngineError::NoProjectile(0))
    ));
    assert_eq!(hp(&late, 1), 100);
}

#[test]
fn a_contact_is_bounded_like_any_other() {
    let world = range();
    let mut engine = start(&world, "butt");
    shoot(&mut engine, 8);
    for (target, ok) in [("archer", false), ("nobody", false), ("brawler", false)] {
        assert_eq!(engine.execute(hit(0, target)).is_ok(), ok, "{target}");
    }
    assert_eq!(
        live(&engine),
        [0],
        "a refused report leaves the shot flying"
    );
}

#[test]
fn a_stagger_loses_a_draw_but_not_a_shot_in_the_air() {
    let world = range();
    let mut engine = start(&world, "brawler");
    // Loose first, then draw again while the brawler's blow comes.
    shoot(&mut engine, 2);
    engine.execute(step(3)).unwrap();
    engine.execute(Intent(I::Draw)).unwrap();
    let shots = live(&engine);
    let brawler = run(
        &mut engine,
        vec![Intent(I::Contact {
            attacker: "brawler".into(),
            target: None,
        })],
    );
    assert!(brawler.contains(&Event::Staggered {
        character: "archer".into()
    }));
    assert!(matches!(
        action(&engine, 0),
        RealtimeAction::Staggered { .. }
    ));
    assert_eq!(live(&engine), shots, "nothing was loosed and nothing lost");
    assert!(matches!(
        engine.execute(Intent(I::Loose)),
        Err(EngineError::NotDrawing)
    ));
    assert!(matches!(
        engine.execute(Intent(I::Draw)),
        Err(EngineError::Committed)
    ));
    engine.execute(hit(0, "brawler")).unwrap();
}

#[test]
fn drawing_is_no_commitment() {
    let world = range();
    for begin in [I::Parry, I::Dodge, I::Attack { target: None }, I::Retreat] {
        let mut engine = start(&world, "butt");
        engine.execute(Intent(I::Draw)).unwrap();
        engine.execute(Intent(begin.clone())).unwrap();
        assert!(!matches!(
            action(&engine, 0),
            RealtimeAction::Drawing { .. }
        ));
        assert!(live(&engine).is_empty(), "{begin:?} looses nothing");
    }
    // A raised guard gives way to a draw; a draw is lowered before a guard.
    let mut engine = start(&world, "butt");
    engine.execute(Intent(I::Guard)).unwrap();
    engine.execute(Intent(I::Draw)).unwrap();
    assert!(matches!(
        engine.execute(Intent(I::Guard)),
        Err(EngineError::Committed)
    ));
}

#[test]
fn a_parry_turns_a_shot_aside_without_reaching_the_shooter() {
    let world = range();
    let mut engine = start(&world, "fencer");
    shoot(&mut engine, 8);
    // The fencer parries only blows she sees: force her window by a blow.
    engine
        .execute(Intent(I::Attack { target: None }))
        .unwrap_err();
    engine.execute(step(3)).unwrap();
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    engine.execute(step(3)).unwrap();
    assert!(matches!(action(&engine, 1), RealtimeAction::Parry { .. }));
    let before = hp(&engine, 1);
    let events = run(&mut engine, vec![hit(0, "fencer")]);
    assert_eq!(ended(&events), [(0, ProjectileEnd::Deflected)]);
    assert!(events.contains(&Event::Parried {
        character: "fencer".into(),
        attacker: "archer".into()
    }));
    assert_eq!(hp(&engine, 1), before);
    assert!(
        matches!(action(&engine, 0), RealtimeAction::Attack { .. }),
        "the shooter is out of reach: no stagger"
    );
}

#[test]
fn a_dodge_lets_a_shot_fly_on_and_it_can_still_land_on_someone_else() {
    let mut world = range();
    let brawler = world
        .characters
        .iter_mut()
        .find(|c| c.id == "fencer")
        .unwrap();
    brawler.combat.as_mut().unwrap().reaction = Some(Reaction::Dodge);
    let mut engine = start(&world, "fencer");
    shoot(&mut engine, 8);
    engine.execute(step(3)).unwrap();
    engine.execute(Intent(I::Attack { target: None })).unwrap();
    engine.execute(step(3)).unwrap();
    assert!(matches!(action(&engine, 1), RealtimeAction::Dodge { .. }));
    let events = run(&mut engine, vec![hit(0, "fencer")]);
    assert!(ended(&events).is_empty(), "{events:?}");
    assert!(events.contains(&Event::Evaded {
        character: "fencer".into(),
        attacker: "archer".into()
    }));
    assert_eq!(live(&engine), [0]);
    // Its window closes, and a report on it now lands.
    let open_until = match action(&engine, 1) {
        RealtimeAction::Dodge { open_until, .. } => open_until,
        other => panic!("{other:?}"),
    };
    let now = engine.realtime().unwrap().step;
    engine.execute(step((open_until - now) as u32)).unwrap();
    let events = run(&mut engine, vec![hit(0, "fencer")]);
    assert_eq!(ended(&events), [(0, ProjectileEnd::Landed)]);
}

#[test]
fn the_fight_ending_takes_every_shot_with_it_and_settles_once() {
    let world = range();
    let mut engine = start(&world, "butt");
    let mut settled = 0;
    let count = |events: &[Event]| {
        events
            .iter()
            .filter(|e| matches!(e, Event::EncounterEnded { .. }))
            .count()
    };
    let mut next = 0;
    while engine.realtime().is_some() {
        shoot(&mut engine, 8);
        engine.execute(step(3)).unwrap();
        shoot(&mut engine, 8);
        assert_eq!(live(&engine), [next, next + 1]);
        settled += count(&engine.execute(hit(next, "butt")).unwrap());
        // The last blow may fall on either; the other shot is still flying.
        if engine.realtime().is_none() {
            break;
        }
        settled += count(&engine.execute(hit(next + 1, "butt")).unwrap());
        if engine.realtime().is_some() {
            engine.execute(step(3)).unwrap();
        }
        next += 2;
    }
    assert_eq!(settled, 1);
    assert!(matches!(
        engine.execute(hit(next + 1, "butt")),
        Err(EngineError::NotFighting)
    ));
    assert!(matches!(
        engine.execute(spent(next + 1)),
        Err(EngineError::NotFighting)
    ));
}

#[test]
fn an_inert_fighter_only_stands_there_and_can_be_fought_again() {
    let world = range();
    let mut engine = start(&world, "butt");
    let events = engine.execute(step(200)).unwrap();
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(action(&engine, 1), RealtimeAction::Ready);
    assert_eq!(hp(&engine, 0), 60);
    Engine::restore(&world, engine.snapshot()).expect("it may stay ready in a save");
}

#[test]
fn a_fight_saved_with_shots_in_the_air_resumes_exactly_and_replays_the_same() {
    let world = range();
    let script = |s: u32| {
        vec![
            EngageRealtime("butt".into()),
            Intent(I::Draw),
            step(8),
            Intent(I::Loose),
            step(3),
            Intent(I::Draw),
            step(s),
        ]
    };
    let rest = vec![
        Intent(I::Loose),
        step(1),
        hit(0, "butt"),
        spent(1),
        step(2),
        Intent(I::Draw),
    ];
    let mut straight = Engine::new(&world).unwrap();
    let mut events = run(&mut straight, script(4));
    let mut saved = Engine::new(&world).unwrap();
    run(&mut saved, script(4));
    let json = saved.snapshot().to_json();
    let mut resumed = Engine::restore(&world, SaveSnapshot::from_json(&json).unwrap()).unwrap();
    let left = run(&mut straight, rest.clone());
    assert_eq!(run(&mut resumed, rest.clone()), left);
    assert_eq!(resumed.state(), straight.state());
    // The same commands again, with the advances split, give the same fight.
    let mut split = Engine::new(&world).unwrap();
    let mut again = Vec::new();
    for command in script(4).into_iter().chain(rest) {
        match command {
            Intent(I::Advance(n)) => {
                (0..n).for_each(|_| again.extend(split.execute(step(1)).unwrap()))
            }
            other => again.extend(split.execute(other).unwrap()),
        }
    }
    events.extend(left);
    assert_eq!(again, events);
    assert_eq!(split.realtime(), straight.realtime());
}

#[test]
fn a_save_holds_only_shots_and_draws_the_rules_could_leave() {
    let world = range();
    let mut engine = start(&world, "butt");
    shoot(&mut engine, 8);
    let good = engine.snapshot();
    let tampered = |change: &dyn Fn(&mut RealtimeEncounter)| {
        let mut snapshot = good.clone();
        let combat = snapshot.state.combat.as_mut().unwrap();
        let Stance::Realtime(fight) = &mut combat.stance else {
            panic!("in a fight");
        };
        change(fight);
        Engine::restore(&world, snapshot).is_ok()
    };
    assert!(tampered(&|_| {}));
    assert!(!tampered(&|f| f.projectiles[0].power = 201), "too strong");
    assert!(!tampered(&|f| f.projectiles[0].power = 49), "too weak");
    assert!(!tampered(&|f| f.projectiles[0].impact = 9), "impact");
    assert!(
        !tampered(&|f| f.projectiles[0].shooter = "butt".into()),
        "shooter"
    );
    assert!(
        !tampered(&|f| f.projectiles[0].expires = f.step),
        "out of flight"
    );
    assert!(
        !tampered(&|f| f.projectiles[0].expires = f.step + 21),
        "too long"
    );
    assert!(
        !tampered(&|f| f.next_projectile = 0),
        "IDs ahead of the counter"
    );
    assert!(
        !tampered(&|f| {
            let again = f.projectiles[0].clone();
            f.projectiles.push(again);
        }),
        "one shot twice"
    );
    assert!(
        !tampered(&|f| f.fighters[0].action = RealtimeAction::Drawing { began: f.step + 1 }),
        "drawn from the future"
    );
    assert!(
        !tampered(&|f| f.fighters[0].action = RealtimeAction::Loosed { until: f.step }),
        "over already"
    );
    assert!(
        !tampered(&|f| f.fighters[1].action = RealtimeAction::Drawing { began: 0 }),
        "only the player draws"
    );

    // With no bow in hand, neither a draw nor a shot can be in a save.
    let mut swapped = good.clone();
    let combat = swapped.state.combat.as_mut().unwrap();
    for gear in combat.gear.values_mut() {
        gear.equipped = gear.item == "staff";
    }
    assert!(Engine::restore(&world, swapped).is_err());
}

#[test]
fn the_menu_offers_a_draw_and_never_a_report() {
    let world = range();
    let mut engine = start(&world, "butt");
    let offered = |engine: &Engine<'_>, intent: I| {
        engine
            .actions()
            .into_iter()
            .find(|a| a.command == Intent(intent.clone()))
            .map(|a| a.available)
    };
    assert_eq!(offered(&engine, I::Draw), Some(true));
    assert_eq!(offered(&engine, I::Loose), None);
    engine.execute(Intent(I::Draw)).unwrap();
    assert_eq!(offered(&engine, I::Loose), Some(true));
    assert_eq!(offered(&engine, I::LowerDraw), Some(true));
    assert_eq!(offered(&engine, I::Draw), None);
    engine.execute(step(8)).unwrap();
    engine.execute(Intent(I::Loose)).unwrap();
    assert_eq!(offered(&engine, I::Draw), Some(false));
    let reported = |a: &Action| {
        matches!(
            a.command,
            Intent(I::ProjectileContact { .. } | I::ProjectileSpent { .. })
        )
    };
    assert!(!engine.actions().iter().any(reported));
}
