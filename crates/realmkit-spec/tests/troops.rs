//! Validation of troop lines, recruiting, armies and the mass-battle rules.

mod common;

use common::*;
use realmkit_spec::*;

fn troops(w: &mut WorldSpec) -> &mut Troops {
    w.world.troops.as_mut().unwrap()
}

fn battle(w: &mut WorldSpec) -> &mut Battle {
    w.world.battle.as_mut().unwrap()
}

fn line<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut TroopLine {
    troops(w).lines.iter_mut().find(|l| l.id == id).unwrap()
}

fn army<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Army {
    w.characters
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .army
        .as_mut()
        .unwrap()
}

fn ashmere(w: &mut WorldSpec) -> &mut Recruits {
    w.locations
        .iter_mut()
        .find(|l| l.id == "ashmere")
        .unwrap()
        .recruits
        .as_mut()
        .unwrap()
}

#[test]
fn the_marches_raise_levies_and_fight_outlaws() {
    let world = marches();
    assert!(codes(&world).is_empty(), "{:?}", world.diagnostics());
    let troops = world.troops().unwrap();
    let levy = troops.line("levy").unwrap();
    // Names and wages hold until a level authors new ones.
    assert_eq!(
        [1, 2, 3].map(|l| levy.name_at(l)),
        ["Levy", "Levy", "Spearman"]
    );
    assert_eq!([1, 2, 3].map(|l| levy.wage_at(l)), [1, 1, 2]);
    assert!(world.random_battle() && world.stochastic());
    let json = serde_json::to_string(&world.world).unwrap();
    assert_eq!(serde_json::from_str::<World>(&json).unwrap(), world.world);
}

#[test]
fn troop_content_is_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| troops(w).classes[1].mounted = true, "invalid_class"),
        (
            |w| line(w, "levy").class = "missing".into(),
            "missing_reference",
        ),
        (|w| line(w, "levy").levels[0].name = None, "invalid_line"),
        (|w| line(w, "levy").levels[0].xp = 5, "invalid_line"),
        (|w| line(w, "levy").levels[1].xp = 0, "invalid_line"),
        (|w| line(w, "levy").levels[0].stats.hp = 0, "invalid_stats"),
        // A soldier with nothing to strike with in the line's channel.
        (
            |w| {
                let stats = &mut line(w, "levy").levels[0].stats;
                stats.patk = 0;
                stats.satk = 0;
            },
            "no_attack",
        ),
        (
            |w| line(w, "levy").upgrades[0].to = "missing".into(),
            "missing_reference",
        ),
        (
            |w| line(w, "levy").upgrades[0].to = "levy".into(),
            "invalid_line",
        ),
        (|w| troops(w).limit = 0, "invalid_limit"),
        (|w| troops(w).limit = ROSTER_BOUND + 1, "invalid_limit"),
        (|w| troops(w).wounded_percent = 101, "invalid_percent"),
        (
            |w| troops(w).upkeep.as_mut().unwrap().desert_percent = 101,
            "invalid_percent",
        ),
        (|w| w.world.time = None, "time_disabled"),
        (|w| w.world.combat = None, "combat_disabled"),
        (|w| battle(w).frontage = 0, "invalid_battle"),
        (|w| battle(w).roll = [110, 90], "invalid_battle"),
        (
            |w| {
                battle(w)
                    .matchups
                    .insert("missing".into(), Default::default());
            },
            "missing_reference",
        ),
        (
            |w| battle(w).pursuit_class = Some("missing".into()),
            "missing_reference",
        ),
        (
            |w| battle(w).morale.as_mut().unwrap().rout = 100,
            "invalid_percent",
        ),
        (|w| w.world.troops = None, "troops_disabled"),
        (|w| ashmere(w).troops[0].size = 0, "invalid_limit"),
        (
            |w| ashmere(w).troops[0].line = "missing".into(),
            "missing_reference",
        ),
        (|w| army(w, "outlaws").troops[1].level = 3, "invalid_level"),
        (|w| army(w, "outlaws").troops[0].count = 0, "invalid_army"),
        (|w| army(w, "outlaws").troops.clear(), "invalid_army"),
        (|w| w.world.battle = None, "battle_disabled"),
    ];
    for (change, code) in cases {
        let mut world = marches();
        change(&mut world);
        assert!(
            codes(&world).contains(&code.to_string()),
            "{code}: {:?}",
            world.diagnostics()
        );
    }
    // A character leads an army or fights alone.
    let mut world = marches();
    let arena = arena();
    let rat = arena.characters.iter().find(|c| c.id == "rat").unwrap();
    let outlaws = world
        .characters
        .iter_mut()
        .find(|c| c.id == "outlaws")
        .unwrap();
    outlaws.combat = rat.combat.clone();
    outlaws.combat.as_mut().unwrap().group = None;
    assert!(codes(&world).contains(&"invalid_army".to_string()));
}

#[test]
fn rewards_on_an_ally_and_wages_without_upkeep_are_warned_about() {
    let mut world = marches();
    army(&mut world, "warden").xp = 10;
    troops(&mut world).upkeep = None;
    let warnings: Vec<_> = world
        .diagnostics()
        .into_iter()
        .filter(|d| d.severity == Severity::Warning)
        .map(|d| d.code)
        .collect();
    assert!(
        warnings.contains(&"unused_reward".to_string()),
        "{warnings:?}"
    );
    assert!(
        warnings.contains(&"unused_wages".to_string()),
        "{warnings:?}"
    );
    assert!(world.validate().is_ok());
}
