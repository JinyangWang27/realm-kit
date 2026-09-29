//! Worlds and helpers shared by the gameplay tests.
#![allow(dead_code)]

use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

pub fn demo() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/demo-world"
    ))
    .unwrap()
}

pub fn archive() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/quiet-archive"
    ))
    .unwrap()
}

pub fn combat<'e>(engine: &'e Engine<'_>) -> &'e CombatState {
    engine.state().combat.as_ref().unwrap()
}

pub fn vitals(engine: &Engine<'_>) -> Vitals {
    engine.player_vitals().unwrap()
}

pub fn foe(engine: &Engine<'_>) -> Participant {
    engine.encounter().unwrap().participants[1].clone()
}

pub fn offered(engine: &Engine<'_>) -> Vec<(Command, bool)> {
    engine
        .actions()
        .into_iter()
        .map(|a| (a.command, a.available))
        .collect()
}

pub fn duel() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/duel")).unwrap()
}

pub fn cast(skill: &str) -> Command {
    UseSkill {
        skill: skill.into(),
        target: "witch".into(),
    }
}

pub fn combatant<'w>(world: &'w mut WorldSpec, id: &str) -> &'w mut CombatProfile {
    world
        .characters
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
}

pub fn arena() -> WorldSpec {
    WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap()
}

/// Attacks the first fighting opponent until the encounter ends.
pub fn fight_out(engine: &mut Engine<'_>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(encounter) = engine.encounter() {
        if engine.is_dead() {
            break;
        }
        let foe = encounter.participants[1..]
            .iter()
            .find(|p| p.fighting())
            .unwrap()
            .character
            .clone();
        events.extend(engine.execute(Attack(foe)).unwrap());
    }
    events
}

/// The arena with the pit open, for tests of the ogre.
pub fn open_pit() -> WorldSpec {
    let mut world = arena();
    world.locations[0]
        .exits
        .get_mut(&Down)
        .unwrap()
        .requires
        .clear();
    world
}
