//! Named companions: recruiting and parting, travelling with the party,
//! fighting in real time with their own persistent vitals, tactical orders
//! and the targets they choose, saves, replays and worlds without them, at
//! Hollow Ford.

use realmkit_engine::{Command::*, *};
use realmkit_spec::*;
use serde_json::json;

fn ford() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/hollow-ford"
    ))
    .unwrap()
}

fn character<'w>(world: &'w mut WorldSpec, id: &str) -> &'w mut Character {
    world.characters.iter_mut().find(|c| c.id == id).unwrap()
}

/// The ford's bandits come back however often they are beaten, so a test
/// can fight there twice.
fn repeatable(mut world: WorldSpec) -> WorldSpec {
    world.world.combat.as_mut().unwrap().groups[0].repeatable = true;
    world
}

/// A raider at the ford who belongs to nobody: a third side in a fight.
fn with_raider(mut world: WorldSpec) -> WorldSpec {
    let raider = serde_json::from_value(json!({
        "id": "raider",
        "name": "A River Raider",
        "description": "He fights everyone on the bank.",
        "combat": {
            "stats": { "hp": 24, "patk": 8, "pdef": 3, "satk": 0, "sdef": 2, "speed": 100 },
            "xp": 5
        }
    }))
    .unwrap();
    world.characters.push(raider);
    world.locations[2].characters.push("raider".into());
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    world
}

fn run(engine: &mut Engine<'_>, commands: &[Command]) -> Vec<Event> {
    commands
        .iter()
        .flat_map(|c| engine.execute(c.clone()).unwrap())
        .collect()
}

/// Mara vouches, Wren and Brother Oake join, and the party reaches the ford.
fn recruiting() -> Vec<Command> {
    vec![
        Talk("mara".into()),
        ChooseDialogue(1),
        ChooseDialogue(1),
        Talk("wren".into()),
        ChooseDialogue(1),
        Travel("crossroads".into()),
        Talk("oake".into()),
        ChooseDialogue(1),
        Travel("ford".into()),
    ]
}

fn at_the_ford(world: &WorldSpec) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    run(&mut engine, &recruiting());
    engine
}

fn step(n: u32) -> Command {
    Intent(realmkit_engine::Intent::Advance(n))
}

fn join(character: &str) -> Command {
    Intent(realmkit_engine::Intent::Join {
        character: character.into(),
        team: 0,
    })
}

fn order(companion: &str, order: TacticalOrder) -> Command {
    Tactic {
        companion: companion.into(),
        order,
    }
}

fn attack(target: &str) -> TacticalOrder {
    TacticalOrder::Attack {
        target: target.into(),
    }
}

fn defend(ally: &str) -> TacticalOrder {
    TacticalOrder::Defend { ally: ally.into() }
}

fn fight(engine: &Engine<'_>) -> RealtimeEncounter {
    engine.realtime().unwrap().clone()
}

fn fighter(engine: &Engine<'_>, id: &str) -> Fighter {
    let fight = fight(engine);
    fight.fighters[fight.index(id).unwrap()].clone()
}

fn companion(engine: &Engine<'_>, id: &str) -> CompanionState {
    engine.companion(id).unwrap().clone()
}

/// Whom `id` is aiming its current attack at, if it is attacking.
fn aim(engine: &Engine<'_>, id: &str) -> Option<String> {
    match fighter(engine, id).action {
        RealtimeAction::Attack { target, .. } => Some(target),
        _ => None,
    }
}

/// The command was refused for exactly this reason and changed nothing.
fn refused(engine: &mut Engine<'_>, command: Command, expected: EngineError) {
    let before = engine.snapshot().to_json();
    let error = engine.execute(command).unwrap_err();
    assert_eq!(format!("{error:?}"), format!("{expected:?}"));
    assert_eq!(engine.snapshot().to_json(), before);
}

/// One frame of a host whose every blow lands where it was aimed while that
/// fighter still stands; a ready player strikes at `prey` if it still
/// fights, else the first foe still fighting; then the clock moves on.
fn frame(engine: &mut Engine<'_>, prey: &str) -> Vec<Command> {
    let mut sent = Vec::new();
    for f in fight(engine).fighters {
        let active = matches!(
            f.action,
            RealtimeAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                ..
            }
        );
        if !active || engine.realtime().is_none() {
            continue;
        }
        let command = Intent(realmkit_engine::Intent::Contact {
            attacker: f.character.clone(),
            target: None,
        });
        if engine.execute(command.clone()).is_ok() {
            sent.push(command);
        }
    }
    let Some(fight) = engine.realtime().cloned() else {
        return sent;
    };
    if fight.fighters[0].action == RealtimeAction::Ready {
        let standing = |i: usize| fight.fighters[i].team != 0 && fight.fighting(i, engine.state());
        let foe = fight
            .index(prey)
            .filter(|&i| standing(i))
            .or_else(|| (1..fight.fighters.len()).find(|&i| standing(i)))
            .unwrap();
        let strike = Intent(realmkit_engine::Intent::Attack {
            target: Some(fight.fighters[foe].character.clone()),
        });
        engine.execute(strike.clone()).unwrap();
        sent.push(strike);
    }
    engine.execute(step(1)).unwrap();
    sent.push(step(1));
    sent
}

/// Frames until the fight ends, or `limit` frames pass.
fn fight_on(engine: &mut Engine<'_>, prey: &str, limit: usize) -> Vec<Command> {
    let mut sent = Vec::new();
    for _ in 0..limit {
        if engine.realtime().is_none() {
            break;
        }
        sent.extend(frame(engine, prey));
    }
    sent
}

fn count(events: &[Event], wanted: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| wanted(e)).count()
}

#[test]
fn a_companion_joins_only_by_its_authored_conditions_and_parts_where_the_player_stands() {
    let world = ford();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    // Unvouched, Wren only says why not.
    let events = run(&mut engine, &[Talk("wren".into()), ChooseDialogue(1)]);
    assert!(engine.party().is_empty());
    assert_eq!(
        count(&events, |e| matches!(e, Event::JoinedParty { .. })),
        0
    );
    let events = run(&mut engine, &recruiting()[..5]);
    assert!(events.contains(&Event::JoinedParty {
        character: "wren".into()
    }));
    assert_eq!(engine.party(), vec!["wren"]);
    let wren = companion(&engine, "wren");
    assert_eq!((wren.vitals.hp, wren.vitals.mp), (30, 6));
    assert_eq!(wren.order, TacticalOrder::Follow);
    assert!(engine.holds(&Condition::Companion {
        character: "wren".into()
    }));
    // Asked again, the dialogue offers parting instead of a second Wren.
    run(&mut engine, &[Talk("wren".into())]);
    let choices: Vec<_> = engine
        .dialogue_choices()
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert_eq!(choices, ["We part here.", "Nothing."]);
    // She parts at the crossroads and stays there; the inn never sees her.
    run(
        &mut engine,
        &[ChooseDialogue(2), Travel("crossroads".into())],
    );
    let events = run(&mut engine, &[Talk("wren".into()), ChooseDialogue(1)]);
    assert!(events.contains(&Event::LeftParty {
        character: "wren".into()
    }));
    assert!(engine.party().is_empty());
    assert_eq!(
        companion(&engine, "wren").parted_at.as_deref(),
        Some("crossroads")
    );
    run(&mut engine, &[Travel("inn".into())]);
    assert!(!engine.present_here().iter().any(|c| c.id == "wren"));
    run(&mut engine, &[Travel("crossroads".into())]);
    assert!(engine.present_here().iter().any(|c| c.id == "wren"));
    // Asked again where she stays, she rejoins.
    run(&mut engine, &[Talk("wren".into()), ChooseDialogue(1)]);
    assert_eq!(engine.party(), vec!["wren"]);
}

#[test]
fn recruiting_a_companion_twice_is_refused_whole() {
    // A careless choice that recruits whoever is already in the party.
    let mut world = ford();
    let dialogue = world.dialogues.iter_mut().find(|d| d.id == "wren").unwrap();
    dialogue.nodes[0].choices[1].requires = Some(Condition::Flag {
        flag: "vouched".into(),
    });
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    run(&mut engine, &recruiting()[..5]);
    run(&mut engine, &[Talk("wren".into())]);
    refused(
        &mut engine,
        ChooseDialogue(1),
        EngineError::AlreadyInParty("wren".into()),
    );
    assert_eq!(engine.party(), vec!["wren"]);
    assert_eq!(engine.state().companions.len(), 1);
}

#[test]
fn the_party_travels_with_the_player_by_road_and_world_time() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    assert_eq!(engine.state().player.location, "ford");
    assert_eq!(engine.party(), vec!["oake", "wren"]);
    // Both are here, listed after the ford's own, and nowhere they were placed.
    let here: Vec<_> = engine.present_here().iter().map(|c| c.id.clone()).collect();
    assert_eq!(here, ["ford_captain", "ford_lookout", "oake", "wren"]);
    run(&mut engine, &[Travel("crossroads".into())]);
    let here: Vec<_> = engine.present_here().iter().map(|c| c.id.clone()).collect();
    assert_eq!(here, ["oake", "wren"]);
    // A companion is nobody's foe, and never a duel's opening opponent.
    run(&mut engine, &[Travel("ford".into())]);
    refused(
        &mut engine,
        Duel("wren".into()),
        EngineError::NotHostile("wren".into()),
    );
}

#[test]
fn two_companions_fight_beside_the_player_and_keep_what_the_fight_left_them() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    let mut events = run(
        &mut engine,
        &[Duel("ford_captain".into()), join("wren"), join("oake")],
    );
    // Their fighters hold no vitals: the fight reads and changes their own.
    for id in ["wren", "oake"] {
        let f = fighter(&engine, id);
        assert_eq!((f.vitals, f.team, f.joined), (None, 0, 0));
    }
    let mut commands = Vec::new();
    while engine.realtime().is_some() {
        commands.extend(frame(&mut engine, "ford_captain"));
        assert!(commands.len() < 2_000, "the fight should end");
    }
    // Replay the fight's commands from the same start for its events.
    let mut replay = at_the_ford(&world);
    events.extend(run(
        &mut replay,
        &[Duel("ford_captain".into()), join("wren"), join("oake")],
    ));
    events.extend(run(&mut replay, &commands));
    assert_eq!(replay.snapshot().to_json(), engine.snapshot().to_json());
    for id in ["wren", "oake"] {
        assert!(
            count(
                &events,
                |e| matches!(e, Event::Struck { attacker, .. } if attacker == id)
            ) > 0,
            "{id} struck"
        );
    }
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    // Wren was cut down to half and spent her MP; she keeps exactly that.
    let wren = companion(&engine, "wren");
    assert_eq!((wren.vitals.hp, wren.vitals.mp), (15, 0));
    // Rage belonged to the fight, as the player's does in this world.
    assert_eq!((wren.vitals.rage, wren.vitals.rage_remainder), (0, 0));
    // An order to attack someone in a fight ends with it.
    assert_eq!(wren.order, TacticalOrder::Follow);
    // Nothing the fight left them is undone by travel or by parting and
    // joining again; world time recovers MP only, at its fixed minutes.
    run(
        &mut engine,
        &[Travel("crossroads".into()), Travel("inn".into())],
    );
    let wren = companion(&engine, "wren");
    assert_eq!((wren.vitals.hp, wren.vitals.mp), (15, 2));
    run(
        &mut engine,
        &[
            Talk("wren".into()),
            ChooseDialogue(1),
            Talk("wren".into()),
            ChooseDialogue(1),
        ],
    );
    assert_eq!(engine.party(), vec!["oake", "wren"]);
    assert_eq!(companion(&engine, "wren").vitals.hp, 15);
    // Resting at the inn restores the party with the player.
    run(&mut engine, &[Rest]);
    let wren = companion(&engine, "wren");
    assert_eq!((wren.vitals.hp, wren.vitals.mp), (30, 6));
}

#[test]
fn a_companion_brings_its_exact_vitals_into_the_next_fight() {
    // Rage outlasts fights here, so all three resources carry over.
    let mut world = repeatable(ford());
    world.world.combat.as_mut().unwrap().resources.rage_carries = true;
    let mut engine = at_the_ford(&world);
    run(&mut engine, &[Duel("ford_captain".into()), join("wren")]);
    fight_on(&mut engine, "ford_captain", 2_000);
    assert!(engine.realtime().is_none());
    let after = companion(&engine, "wren").vitals;
    assert!(after.hp < 30 && after.rage > 0, "{after:?}");
    // A fight she sits out leaves her as she was.
    run(&mut engine, &[Duel("ford_captain".into())]);
    fight_on(&mut engine, "ford_captain", 2_000);
    assert_eq!(companion(&engine, "wren").vitals, after);
    // Joining the next one, she enters with exactly those vitals.
    run(&mut engine, &[Duel("ford_captain".into()), join("wren")]);
    let i = fight(&engine).index("wren").unwrap();
    assert_eq!(engine.fighter_vitals(i), Some(after));
    assert_eq!(fighter(&engine, "wren").vitals, None);
}

#[test]
fn a_companion_joins_a_fight_already_running() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    run(&mut engine, &[Duel("ford_captain".into())]);
    fight_on(&mut engine, "ford_captain", 6);
    let now = fight(&engine).step;
    assert!(now > 0);
    let events = run(&mut engine, &[join("wren")]);
    assert_eq!(
        events,
        [Event::Joined {
            character: "wren".into(),
            team: 0
        }]
    );
    let wren = fighter(&engine, "wren");
    assert_eq!((wren.joined, wren.action), (now, RealtimeAction::Ready));
    // Nobody joins twice; a temporary copy of her cannot come in either.
    refused(
        &mut engine,
        join("wren"),
        EngineError::AlreadyJoined("wren".into()),
    );
    refused(
        &mut engine,
        Intent(realmkit_engine::Intent::Join {
            character: "oake".into(),
            team: 1,
        }),
        EngineError::InvalidTeam {
            character: "oake".into(),
            team: 1,
        },
    );
    fight_on(&mut engine, "ford_captain", 2_000);
    assert!(engine.realtime().is_none());
}

#[test]
fn only_a_companion_in_the_party_and_on_its_feet_can_join() {
    // Wren is fragile here, and the lookout goes for the weakest.
    let mut world = repeatable(ford());
    character(&mut world, "wren")
        .combat
        .as_mut()
        .unwrap()
        .stats
        .hp = 4;
    let mut engine = at_the_ford(&world);
    run(&mut engine, &[Duel("ford_captain".into()), join("wren")]);
    let mut replay = engine.clone();
    let events = run(&mut replay, &fight_on(&mut engine, "ford_captain", 2_000));
    assert!(events.contains(&Event::AllyDown {
        character: "wren".into()
    }));
    assert!(engine.realtime().is_none());
    assert!(companion(&engine, "wren").downed());
    // Down, she still travels with the party but cannot fight.
    assert_eq!(engine.party(), vec!["oake", "wren"]);
    run(&mut engine, &[Duel("ford_captain".into())]);
    refused(
        &mut engine,
        join("wren"),
        EngineError::Downed("wren".into()),
    );
    fight_on(&mut engine, "ford_captain", 2_000);
    // Parting and joining again does not stand her up; resting does.
    run(
        &mut engine,
        &[
            Travel("crossroads".into()),
            Travel("inn".into()),
            Talk("wren".into()),
            ChooseDialogue(1),
            Talk("wren".into()),
            ChooseDialogue(1),
        ],
    );
    assert!(companion(&engine, "wren").downed());
    run(
        &mut engine,
        &[Rest, Travel("crossroads".into()), Travel("ford".into())],
    );
    run(&mut engine, &[Duel("ford_captain".into()), join("wren")]);
    let i = fight(&engine).index("wren").unwrap();
    assert_eq!(engine.fighter_vitals(i).unwrap().hp, 4);
    // A companion out of the party cannot come in from where it stays.
    let plain = ford();
    let mut engine = at_the_ford(&plain);
    run(
        &mut engine,
        &[
            Talk("oake".into()),
            ChooseDialogue(1),
            Duel("ford_captain".into()),
        ],
    );
    refused(
        &mut engine,
        join("oake"),
        EngineError::NotInParty("oake".into()),
    );
}

#[test]
fn attack_target_aims_at_the_named_foe_and_falls_back_when_it_falls() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    run(&mut engine, &[Duel("ford_captain".into()), join("wren")]);
    // By her own `weakest` targeting Wren would go for the lookout.
    let mut follow = engine.clone();
    run(&mut follow, &[step(1)]);
    assert_eq!(aim(&follow, "wren").as_deref(), Some("ford_lookout"));
    let events = run(&mut engine, &[order("wren", attack("ford_captain"))]);
    // An order starts nothing: it only changes whom she aims at next.
    assert_eq!(
        events,
        [Event::OrderGiven {
            companion: "wren".into(),
            order: attack("ford_captain")
        }]
    );
    assert_eq!(fighter(&engine, "wren").action, RealtimeAction::Ready);
    run(&mut engine, &[step(1)]);
    assert_eq!(aim(&engine, "wren").as_deref(), Some("ford_captain"));
    // The player brings the captain down; Wren then falls back to her
    // targeting, with the order still standing until the fight ends.
    let mut fallback = None;
    for _ in 0..2_000 {
        if engine.realtime().is_none() {
            break;
        }
        frame(&mut engine, "ford_captain");
        let Some(fight) = engine.realtime() else {
            break;
        };
        let captain = fight.index("ford_captain").unwrap();
        if !fight.fighting(captain, engine.state()) {
            if let Some(target) = aim(&engine, "wren").filter(|t| t != "ford_captain") {
                fallback = Some(target);
                assert_eq!(companion(&engine, "wren").order, attack("ford_captain"));
            }
        }
    }
    assert_eq!(fallback.as_deref(), Some("ford_lookout"));
    assert_eq!(companion(&engine, "wren").order, TacticalOrder::Follow);
}

#[test]
fn defend_aims_at_whoever_attacks_the_ward() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[Duel("ford_captain".into()), join("wren"), join("oake")],
    );
    // Oake guards first, so he begins his attack once his guard has held.
    let mut follow = engine.clone();
    run(&mut follow, &[step(4)]);
    assert_eq!(aim(&follow, "oake").as_deref(), Some("ford_captain"));
    // The lookout winds up on Wren, the weakest; defending her, Oake goes
    // for the lookout instead of the first foe.
    run(&mut engine, &[order("oake", defend("wren")), step(4)]);
    assert_eq!(aim(&engine, "ford_lookout").as_deref(), Some("wren"));
    assert_eq!(aim(&engine, "oake").as_deref(), Some("ford_lookout"));
}

#[test]
fn hold_strikes_only_at_threats_and_otherwise_waits() {
    // The lookout goes for the first foe like the captain, so nobody comes
    // at Wren while the player stands.
    let mut world = ford();
    character(&mut world, "ford_lookout")
        .combat
        .as_mut()
        .unwrap()
        .targeting = Targeting::First;
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("wren"),
            order("wren", TacticalOrder::Hold),
            step(6),
        ],
    );
    let wren = fighter(&engine, "wren");
    assert_eq!(wren.action, RealtimeAction::Ready);
    assert_eq!(companion(&engine, "wren").vitals.mp, 6);
    // A save while she waits loads: waiting is something the rules do.
    let snapshot = SaveSnapshot::from_json(&engine.snapshot().to_json()).unwrap();
    Engine::restore(&world, snapshot).unwrap();
    // Following again, she strikes at once.
    run(
        &mut engine,
        &[order("wren", TacticalOrder::Follow), step(1)],
    );
    assert!(aim(&engine, "wren").is_some());
    // Holding, a guarding companion keeps his guard up past his hold.
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("oake"),
            order("oake", TacticalOrder::Hold),
            step(8),
        ],
    );
    assert!(matches!(
        fighter(&engine, "oake").action,
        RealtimeAction::Guard { .. }
    ));
    let snapshot = SaveSnapshot::from_json(&engine.snapshot().to_json()).unwrap();
    Engine::restore(&world, snapshot).unwrap();
}

#[test]
fn a_wary_companion_fights_back_only_whatever_its_order() {
    // Oake is always wary here: below his caution from the start. Both
    // bandits go for the player first.
    let mut world = ford();
    character(&mut world, "oake").companion = Some(Companion { caution: Some(100) });
    character(&mut world, "ford_lookout")
        .combat
        .as_mut()
        .unwrap()
        .targeting = Targeting::First;
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("oake"),
            order("oake", attack("ford_captain")),
            step(8),
        ],
    );
    // Nobody aims at him, so he holds his guard and never commits.
    assert!(matches!(
        fighter(&engine, "oake").action,
        RealtimeAction::Guard { .. }
    ));
    // The same order, unwary, sends him at the captain.
    let plain = ford();
    let mut engine = at_the_ford(&plain);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("oake"),
            order("oake", attack("ford_captain")),
            step(4),
        ],
    );
    assert_eq!(aim(&engine, "oake").as_deref(), Some("ford_captain"));
}

#[test]
fn orders_are_checked_before_anything_changes() {
    let world = with_raider(ford());
    let mut engine = at_the_ford(&world);
    // Standing orders can be given outside a fight, but only to the party.
    run(&mut engine, &[order("wren", TacticalOrder::Hold)]);
    assert_eq!(companion(&engine, "wren").order, TacticalOrder::Hold);
    refused(
        &mut engine,
        order("mara", TacticalOrder::Follow),
        EngineError::NotACompanion("mara".into()),
    );
    refused(
        &mut engine,
        order("wren", defend("wren")),
        EngineError::InvalidWard("wren".into()),
    );
    refused(
        &mut engine,
        order("wren", defend("ford_captain")),
        EngineError::InvalidWard("ford_captain".into()),
    );
    refused(
        &mut engine,
        order("wren", attack("ford_captain")),
        EngineError::NotFighting,
    );
    run(
        &mut engine,
        &[
            order("wren", defend("oake")),
            Duel("ford_captain".into()),
            join("wren"),
        ],
    );
    refused(
        &mut engine,
        order("wren", attack("traveller")),
        EngineError::NotHostile("traveller".into()),
    );
    refused(
        &mut engine,
        order("oake", attack("wren")),
        EngineError::NotHostile("wren".into()),
    );
    // Oake travels with the party but is not in the fight.
    refused(
        &mut engine,
        order("wren", attack("oake")),
        EngineError::InvalidTarget("oake".into()),
    );
    refused(
        &mut engine,
        order("wren", attack("mara")),
        EngineError::InvalidTarget("mara".into()),
    );
    // The raider is here but not in the fight until the host reports him.
    refused(
        &mut engine,
        order("wren", attack("raider")),
        EngineError::InvalidTarget("raider".into()),
    );
    // A companion not yet in the fight may be told whom to go for.
    run(&mut engine, &[order("oake", attack("ford_lookout"))]);
    // A fallen foe is no target.
    let mut engine_after = engine.clone();
    while engine_after.realtime().is_some_and(|f| {
        let i = f.index("ford_lookout").unwrap();
        f.fighting(i, engine_after.state())
    }) {
        frame(&mut engine_after, "ford_lookout");
    }
    assert!(
        engine_after.realtime().is_some(),
        "the captain still fights"
    );
    refused(
        &mut engine_after,
        order("wren", attack("ford_lookout")),
        EngineError::InvalidTarget("ford_lookout".into()),
    );
    // Parting with a companion ends orders to defend it.
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            order("wren", defend("oake")),
            Talk("oake".into()),
            ChooseDialogue(1),
        ],
    );
    assert_eq!(companion(&engine, "wren").order, TacticalOrder::Follow);
    refused(
        &mut engine,
        order("oake", TacticalOrder::Hold),
        EngineError::NotInParty("oake".into()),
    );
}

#[test]
fn companions_fight_on_against_several_teams() {
    let world = with_raider(ford());
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("wren"),
            Intent(realmkit_engine::Intent::Join {
                character: "raider".into(),
                team: 2,
            }),
            order("wren", attack("raider")),
            step(1),
        ],
    );
    assert_eq!(fighter(&engine, "raider").team, 2);
    assert_eq!(aim(&engine, "wren").as_deref(), Some("raider"));
    fight_on(&mut engine, "ford_captain", 4_000);
    assert!(engine.realtime().is_none());
}

#[test]
fn saving_mid_fight_and_after_resumes_exactly() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("wren"),
            join("oake"),
            order("wren", attack("ford_captain")),
            order("oake", defend("wren")),
        ],
    );
    fight_on(&mut engine, "ford_captain", 9);
    let bytes = engine.snapshot().to_json();
    let mut resumed = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    assert_eq!(resumed.snapshot().to_json(), bytes);
    // Both play on identically: vitals, actions, targets, orders and clock.
    let mut original = engine.clone();
    let sent = fight_on(&mut original, "ford_captain", 2_000);
    let events = run(&mut resumed, &sent);
    assert!(events.contains(&Event::EncounterEnded {
        outcome: Outcome::Victory
    }));
    assert_eq!(resumed.snapshot().to_json(), original.snapshot().to_json());
    // After the fight, the companions' state saves and loads as it is.
    let bytes = original.snapshot().to_json();
    let after = Engine::restore(&world, SaveSnapshot::from_json(&bytes).unwrap()).unwrap();
    assert_eq!(after.state().companions, original.state().companions);
}

/// A save to start from, and what to change in it.
type Tamper<'s> = (&'s SaveSnapshot, fn(&mut GameState));

#[test]
fn saves_with_companion_state_the_rules_could_not_leave_are_refused() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    run(
        &mut engine,
        &[
            Duel("ford_captain".into()),
            join("wren"),
            order("wren", attack("ford_captain")),
        ],
    );
    fight_on(&mut engine, "ford_captain", 5);
    let mid = engine.snapshot();
    let mut explore = at_the_ford(&world);
    run(&mut explore, &[order("oake", TacticalOrder::Hold)]);
    let calm = explore.snapshot();
    let tamper: Vec<Tamper> = vec![
        // More HP than she has.
        (&calm, |s| {
            s.companions.get_mut("wren").unwrap().vitals.hp = 31
        }),
        // Someone who is no companion, or was never recruitable.
        (&calm, |s| {
            let wren = s.companions["wren"].clone();
            s.companions.insert("mara".into(), wren);
        }),
        // Parted somewhere that does not exist.
        (&calm, |s| {
            s.companions.get_mut("wren").unwrap().parted_at = Some("nowhere".into())
        }),
        // An order out of the party, or to attack outside a fight.
        (&calm, |s| {
            s.companions.get_mut("oake").unwrap().parted_at = Some("ford".into())
        }),
        (&calm, |s| {
            s.companions.get_mut("wren").unwrap().order = TacticalOrder::Attack {
                target: "ford_captain".into(),
            }
        }),
        (&calm, |s| {
            s.companions.get_mut("wren").unwrap().order = TacticalOrder::Defend {
                ally: "wren".into(),
            }
        }),
        // Rage outside a fight in a world where it belongs to one.
        (&calm, |s| {
            s.companions.get_mut("wren").unwrap().vitals.rage = 2
        }),
        // A companion fighting with a copy of its vitals.
        (&mid, |s| {
            let wren = s.companions["wren"].vitals;
            let Some(Stance::Realtime(fight)) = s.combat.as_mut().map(|c| &mut c.stance) else {
                unreachable!()
            };
            let i = fight.index("wren").unwrap();
            fight.fighters[i].vitals = Some(wren);
        }),
        // A companion in the fight that has left the party.
        (&mid, |s| {
            s.companions.get_mut("wren").unwrap().parted_at = Some("ford".into());
            s.companions.get_mut("wren").unwrap().order = TacticalOrder::Follow;
        }),
        // An order to attack a teammate.
        (&mid, |s| {
            s.companions.get_mut("wren").unwrap().order = TacticalOrder::Attack {
                target: "traveller".into(),
            }
        }),
    ];
    for (index, (base, change)) in tamper.into_iter().enumerate() {
        let mut snapshot = base.clone();
        change(&mut snapshot.state);
        assert!(
            Engine::restore(&world, snapshot).is_err(),
            "tampered save {index} loaded"
        );
    }
    assert!(Engine::restore(&world, mid).is_ok());
    assert!(Engine::restore(&world, calm).is_ok());
}

#[test]
fn the_same_commands_replay_byte_for_byte() {
    let world = with_raider(ford());
    let play = || {
        let mut engine = at_the_ford(&world);
        let mut commands = vec![
            Duel("ford_captain".into()),
            join("wren"),
            order("wren", attack("ford_lookout")),
        ];
        run(&mut engine, &commands);
        commands.extend(fight_on(&mut engine, "ford_captain", 7));
        let late = [
            join("oake"),
            order("oake", defend("wren")),
            Intent(realmkit_engine::Intent::Join {
                character: "raider".into(),
                team: 2,
            }),
        ];
        run(&mut engine, &late);
        commands.extend(late);
        commands.extend(fight_on(&mut engine, "ford_captain", 4_000));
        (commands, engine.snapshot().to_json())
    };
    let (commands, bytes) = play();
    let (again, again_bytes) = play();
    assert_eq!(commands, again);
    assert_eq!(bytes, again_bytes);
    let mut first = at_the_ford(&world);
    let mut second = at_the_ford(&world);
    assert_eq!(run(&mut first, &commands), run(&mut second, &commands));
    assert_eq!(first.snapshot().to_json(), bytes);
}

#[test]
fn companions_are_never_duplicated_or_rewarded() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    let mut events = run(
        &mut engine,
        &[Duel("ford_captain".into()), join("wren"), join("oake")],
    );
    let commands = fight_on(&mut engine, "ford_captain", 2_000);
    let mut replay = at_the_ford(&world);
    run(
        &mut replay,
        &[Duel("ford_captain".into()), join("wren"), join("oake")],
    );
    events.extend(run(&mut replay, &commands));
    // Each foe pays once, the companions nothing; nobody appears twice.
    let xp: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            Event::ExperienceGranted { amount } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(xp, [12, 8]);
    assert_eq!(
        count(&events, |e| matches!(e, Event::EncounterEnded { .. })),
        1
    );
    assert_eq!(engine.state().companions.len(), 2);
    assert_eq!(engine.party(), vec!["oake", "wren"]);
    let defeated = &engine.state().combat.as_ref().unwrap().defeated;
    assert!(!defeated.contains("wren") && !defeated.contains("oake"));
    // The fight is over: no join, order or contact can reopen or repay it.
    refused(&mut engine, join("wren"), EngineError::NotFighting);
    refused(
        &mut engine,
        order("wren", attack("ford_captain")),
        EngineError::NotFighting,
    );
}

#[test]
fn a_companion_may_travel_without_fighting() {
    let world = ford();
    let mut engine = at_the_ford(&world);
    let before = engine.state().companions.clone();
    run(&mut engine, &[Duel("ford_captain".into())]);
    fight_on(&mut engine, "ford_captain", 2_000);
    assert!(engine.realtime().is_none());
    assert_eq!(engine.state().companions, before);
}

#[test]
fn temporary_allies_and_worlds_without_companions_are_as_before() {
    // The practice yard knows no companions: nothing new is saved.
    let yard = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/practice-yard"
    ))
    .unwrap();
    let mut engine = Engine::new_with_seed(&yard, 7).unwrap();
    run(&mut engine, &[Duel("brigand".into()), step(5)]);
    let json = String::from_utf8(engine.snapshot().to_json()).unwrap();
    assert!(!json.contains("companions"));
    assert!(engine.party().is_empty());
    refused(
        &mut engine,
        order("master", TacticalOrder::Follow),
        EngineError::NotACompanion("master".into()),
    );
    // An ally who is no companion still fights with vitals of its own.
    let mut world = ford();
    let warden = serde_json::from_value(json!({
        "id": "warden",
        "name": "The Warden",
        "description": "He keeps the ford, for today.",
        "combat": {
            "stats": { "hp": 30, "patk": 8, "pdef": 4, "satk": 0, "sdef": 3, "speed": 100 },
            "xp": 0,
            "ally": true
        }
    }))
    .unwrap();
    world.characters.push(warden);
    world.locations[2].characters.push("warden".into());
    let mut engine = at_the_ford(&world);
    run(&mut engine, &[Duel("ford_captain".into()), join("warden")]);
    assert_eq!(fighter(&engine, "warden").vitals.unwrap().hp, 30);
    refused(
        &mut engine,
        order("warden", TacticalOrder::Follow),
        EngineError::NotACompanion("warden".into()),
    );
}
