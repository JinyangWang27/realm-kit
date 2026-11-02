//! Slots and wearable items.

mod common;

use common::*;
use realmkit_spec::*;

fn gear<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Equipment {
    w.items
        .iter_mut()
        .find(|i| i.id == id)
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
}

#[test]
fn a_drawn_weapon_is_real_time_and_within_bounds() {
    let load = || {
        WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/archery-range"
        ))
        .unwrap()
    };
    assert!(load().diagnostics().is_empty());
    type Change = fn(&mut RangedWeapon);
    let cases: [Change; 8] = [
        |r| r.draw = 0,
        |r| r.min_draw = 0,
        |r| r.min_draw = r.draw + 1,
        |r| r.recovery = 0,
        |r| r.flight = 0,
        |r| r.power = 0,
        |r| r.min_power = r.power + 1,
        |r| r.impact = POISE_BOUND + 1,
    ];
    for change in cases {
        let mut world = load();
        change(gear(&mut world, "short_bow").ranged.as_mut().unwrap());
        assert!(codes(&world).contains(&"invalid_ranged".to_string()));
    }
    let mut untimed = load();
    untimed.world.combat.as_mut().unwrap().realtime = None;
    assert!(codes(&untimed).contains(&"invalid_ranged".to_string()));
    let mut apart = load();
    apart
        .world
        .combat
        .as_mut()
        .unwrap()
        .slots
        .push("back".into());
    gear(&mut apart, "staff").ranged = gear(&mut load(), "short_bow").ranged;
    gear(&mut apart, "staff").slots = vec!["back".into()];
    assert!(codes(&apart).contains(&"competing_weapons".to_string()));
}

#[test]
fn a_drawn_weapons_impact_needs_a_stagger() {
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/archery-range"
    ))
    .unwrap();
    let combat = world.world.combat.as_mut().unwrap();
    let realtime = combat.realtime.as_mut().unwrap();
    (realtime.stagger, realtime.parry) = (0, None);
    combat.skills.iter_mut().for_each(|s| s.impact = 0);
    assert!(codes(&world).contains(&"missing_stagger".to_string()));
    gear(&mut world, "short_bow")
        .ranged
        .as_mut()
        .unwrap()
        .impact = 0;
    assert!(!codes(&world).contains(&"missing_stagger".to_string()));
}

#[test]
fn a_drawn_weapon_needs_attack_in_its_channel() {
    let mut world = WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/archery-range"
    ))
    .unwrap();
    world.world.combat.as_mut().unwrap().cross_share = 0;
    let bow = gear(&mut world, "short_bow");
    bow.ranged.as_mut().unwrap().channel = Channel::Special;
    assert!(codes(&world).contains(&"no_attack".to_string()));
    // Its own bonus gives it some.
    let bow = gear(&mut world, "short_bow");
    bow.bonuses.insert(Stat::Satk, 5);
    assert!(!codes(&world).contains(&"no_attack".to_string()));
}

#[test]
fn an_inert_fighter_does_nothing_at_all() {
    let world = || {
        WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/archery-range"
        ))
        .unwrap()
    };
    type Change = fn(&mut CombatProfile);
    let cases: [Change; 3] = [
        |p| p.skills = vec!["shove".into()],
        |p| p.guard_hold = Some(3),
        |p| p.reaction = Some(Reaction::Parry),
    ];
    for change in cases {
        let mut w = world();
        let butt = w.characters.iter_mut().find(|c| c.id == "butt").unwrap();
        change(butt.combat.as_mut().unwrap());
        assert!(codes(&w).contains(&"invalid_inert".to_string()));
    }
}

#[test]
fn equipment_occupies_declared_slots_within_bounds() {
    let world = arena();
    assert!(world.diagnostics().is_empty(), "{:?}", world.diagnostics());
    let greatsword = world.item("greatsword").unwrap().equipment.clone().unwrap();
    assert_eq!(greatsword.slots, ["main_hand", "off_hand"]);
    assert_eq!(greatsword.basic_time, Some(130));
    let charm = world
        .item("charm_of_warding")
        .unwrap()
        .equipment
        .clone()
        .unwrap();
    assert_eq!(
        charm.modifiers[&Channel::Special],
        Modifier { num: 1, den: 2 }
    );
    let encoded = serde_json::to_string(&world).unwrap();
    assert_eq!(serde_json::from_str::<WorldSpec>(&encoded).unwrap(), world);
    let changes: Vec<fn(&mut WorldSpec)> = vec![
        |w| gear(w, "buckler").slots.clear(),
        |w| gear(w, "buckler").slots.push("off_hand".into()),
        |w| gear(w, "buckler").slots.push("tail".into()),
        |w| {
            gear(w, "buckler")
                .bonuses
                .insert(Stat::Pdef, STAT_BOUND + 1);
        },
        |w| gear(w, "iron_mail").speed_penalty = STAT_BOUND + 1,
        |w| gear(w, "greatsword").basic_time = Some(0),
        |w| {
            gear(w, "charm_of_warding")
                .modifiers
                .insert(Channel::Physical, Modifier { num: 11, den: 1 });
        },
        |w| {
            gear(w, "charm_of_warding")
                .modifiers
                .insert(Channel::Physical, Modifier { num: 1, den: 0 });
        },
        |w| {
            w.world
                .combat
                .as_mut()
                .unwrap()
                .player_equipment
                .push("rat_tail".into())
        },
        |w| {
            let slots = &mut w.world.combat.as_mut().unwrap().slots;
            slots.push(slots[0].clone())
        },
        // Each wearable one is its own piece, so grants stay small.
        |w| {
            let wolf = w
                .characters
                .iter_mut()
                .find(|c| c.id == "black_wolf")
                .unwrap();
            wolf.combat.as_mut().unwrap().loot[0].quantity = GEAR_STACK_BOUND + 1;
        },
        |w| {
            let slots = &mut w.world.combat.as_mut().unwrap().slots;
            slots.extend((0..SLOT_BOUND).map(|i| format!("extra_{i}")));
        },
        // A special-channel weapon for a player with no special attack at all.
        |w| {
            let combat = w.world.combat.as_mut().unwrap();
            combat.cross_share = 0;
            combat.levels.iter_mut().for_each(|l| l.stats.satk = 0);
            gear(w, "greatsword").basic_channel = Some(Channel::Special);
        },
        // Two weapons that can be worn together would compete for the attack.
        |w| {
            gear(w, "charm_of_warding").basic_time = Some(90);
        },
        // Every best piece at once could lift defence past the stat bound.
        |w| {
            gear(w, "iron_mail").bonuses.insert(Stat::Pdef, 9_990);
        },
    ];
    for (index, change) in changes.into_iter().enumerate() {
        let mut world = arena();
        change(&mut world);
        assert!(world.validate().is_err(), "invalid equipment case {index}");
    }
    // Immunity is a modifier of 0 over anything.
    let mut immune = arena();
    gear(&mut immune, "charm_of_warding")
        .modifiers
        .insert(Channel::Physical, Modifier { num: 0, den: 1 });
    assert!(immune.validate().is_ok());
    // Nothing can be worn in a world without combat.
    let mut quiet = archive();
    quiet.items[0].equipment = Some(arena().item("buckler").unwrap().equipment.clone().unwrap());
    assert!(codes(&quiet).contains(&"combat_disabled".to_string()));
}

#[test]
fn a_two_handed_bonus_counts_once_in_the_worst_case() {
    // 9,000 attack on a piece filling two slots is 9,000 worn, not 18,000.
    let mut world = arena();
    gear(&mut world, "greatsword")
        .bonuses
        .insert(Stat::Patk, 9_000);
    assert!(world.validate().is_ok(), "{:?}", world.diagnostics());
}
