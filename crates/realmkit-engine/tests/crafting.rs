//! Forging pieces from recipes and improving them tier by tier.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::{Direction::*, *};

fn smithy() -> WorldSpec {
    WorldSpec::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/smithy"
    ))
    .unwrap()
}

/// Learns from Bran, beats `ingots` beetles in the mine, and stands at the anvil.
fn at_the_anvil(world: &WorldSpec, ingots: u64) -> Engine<'_> {
    let mut engine = Engine::new(world).unwrap();
    engine.execute(Talk("bran".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(Move(Down)).unwrap();
    for _ in 0..ingots {
        engine.execute(Engage("beetle".into())).unwrap();
        fight_out(&mut engine);
    }
    engine.execute(Move(Up)).unwrap();
    engine.execute(Move(East)).unwrap();
    engine
}

fn ingots(engine: &Engine<'_>) -> u64 {
    engine
        .state()
        .player
        .inventory
        .get("iron_ingot")
        .copied()
        .unwrap_or(0)
}

fn tier(engine: &Engine<'_>, piece: u64) -> usize {
    combat(engine).gear[&piece].tier
}

fn smithing(engine: &Engine<'_>) -> TechniqueState {
    combat(engine).techniques["smithing"]
}

#[test]
fn forging_spends_the_inputs_once_and_makes_a_new_piece() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 5);
    assert_eq!(ingots(&engine), 5);
    let events = engine.execute(Forge("iron_sword".into())).unwrap();
    assert_eq!(ingots(&engine), 3);
    assert!(events.contains(&Event::ItemsSpent {
        item: "iron_ingot".into(),
        quantity: 2
    }));
    assert!(events.contains(&Event::Forged {
        recipe: "iron_sword".into(),
        gear: 1
    }));
    let piece = &combat(&engine).gear[&1];
    assert_eq!(
        (piece.item.as_str(), piece.equipped, piece.tier),
        ("iron_sword", false, 0)
    );
    // Forging trains the technique it names.
    assert_eq!(smithing(&engine).xp, 10);
}

#[test]
fn a_recipe_is_hidden_until_known_and_refused_without_its_needs() {
    let world = smithy();
    // Untaught: the recipe is neither offered nor usable.
    let mut engine = Engine::new(&world).unwrap();
    engine.execute(Move(East)).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Forge(_))));
    assert!(matches!(
        engine.execute(Forge("iron_sword".into())),
        Err(EngineError::UnknownRecipe(_))
    ));

    let mut engine = at_the_anvil(&world, 1);
    let before = engine.state().clone();
    // Too few ingots, and mail needs a Journeyman: offered but unavailable.
    assert!(offered(&engine).contains(&(Forge("iron_sword".into()), false)));
    assert!(offered(&engine).contains(&(Forge("iron_mail".into()), false)));
    assert!(matches!(
        engine.execute(Forge("iron_sword".into())),
        Err(EngineError::NotEnoughMaterials(item)) if item == "iron_ingot"
    ));
    assert!(matches!(
        engine.execute(Forge("iron_mail".into())),
        Err(EngineError::RequirementsUnmet)
    ));
    // Away from the anvil nothing is forged either.
    engine.execute(Move(West)).unwrap();
    let away = engine.state().clone();
    assert!(matches!(
        engine.execute(Forge("iron_sword".into())),
        Err(EngineError::NoStation(station)) if station == "anvil"
    ));
    assert_eq!(engine.state(), &away);
    assert_eq!(before.player.inventory, away.player.inventory);
}

#[test]
fn improving_replaces_the_tier_bonus_and_leaves_other_copies_alone() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    // The second forge makes a Journeyman, who can improve.
    let events = engine.execute(Forge("iron_sword".into())).unwrap();
    assert!(events.contains(&Event::TechniqueRankUp {
        technique: "smithing".into(),
        rank: 2
    }));
    engine.execute(Equip(1)).unwrap();
    let base = engine.player_stats().unwrap().patk;
    let events = engine.execute(Improve(1)).unwrap();
    assert!(events.contains(&Event::Improved { gear: 1, tier: 1 }));
    // Fine replaces the sword's +4 with +6: two more, not six.
    assert_eq!(engine.player_stats().unwrap().patk, base + 2);
    assert_eq!((tier(&engine, 1), tier(&engine, 2)), (1, 0));
    assert_eq!(ingots(&engine), 0);
    // Superior needs a Master; nothing changes when refused.
    let before = engine.state().clone();
    assert!(matches!(
        engine.execute(Improve(1)),
        Err(EngineError::RequirementsUnmet | EngineError::NotEnoughMaterials(_))
    ));
    assert_eq!(engine.state(), &before);
    assert!(matches!(
        engine.execute(Improve(9)),
        Err(EngineError::NoSuchGear(9))
    ));
}

#[test]
fn an_improved_mail_keeps_its_lighter_penalty() {
    let mut world = smithy();
    // Make mail an apprentice's work so the test needs only ingots.
    let combat = world.world.combat.as_mut().unwrap();
    combat.recipes[1].requires = None;
    let mail = world
        .items
        .iter_mut()
        .find(|i| i.id == "iron_mail")
        .unwrap();
    mail.equipment.as_mut().unwrap().tiers[0].requires = None;
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_mail".into())).unwrap();
    engine.execute(Equip(1)).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 90);
    engine.execute(Improve(1)).unwrap();
    assert_eq!(engine.player_stats().unwrap().speed, 95);
    // The top tier has nothing above it.
    assert!(matches!(
        engine.execute(Improve(1)),
        Err(EngineError::NoHigherTier(1))
    ));
}

#[test]
fn crafting_saves_and_loads_and_impossible_crafting_state_is_rejected() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    engine.execute(Forge("iron_sword".into())).unwrap();
    engine.execute(Improve(1)).unwrap();
    let snapshot = engine.snapshot();
    let restored = Engine::restore(&world, snapshot.clone()).unwrap();
    assert_eq!(restored.state(), engine.state());

    let broken = |change: fn(&mut GameState)| {
        let mut bad = snapshot.clone();
        change(&mut bad.state);
        Engine::restore(&world, bad).is_err()
    };
    // A tier the item does not have.
    assert!(broken(|s| s
        .combat
        .as_mut()
        .unwrap()
        .gear
        .get_mut(&1)
        .unwrap()
        .tier = 3));
    // A technique nothing teaches.
    assert!(broken(|s| {
        let combat = s.combat.as_mut().unwrap();
        combat
            .techniques
            .insert("archery".into(), TechniqueState { rank: 1, xp: 0 });
    }));
}

#[test]
fn nothing_is_forged_in_a_fight() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 0);
    engine.execute(Move(West)).unwrap();
    engine.execute(Move(Down)).unwrap();
    engine.execute(Engage("beetle".into())).unwrap();
    assert!(matches!(
        engine.execute(Forge("iron_sword".into())),
        Err(EngineError::InEncounter)
    ));
}

#[test]
fn materials_never_exceed_their_grants() {
    let mut world = smithy();
    // Beetles that stay dead: their ingots are a finite grant.
    world.world.combat.as_mut().unwrap().groups[0].repeatable = false;
    let engine = at_the_anvil(&world, 1);
    let snapshot = engine.snapshot();
    let with = |count: u64| {
        let mut changed = snapshot.clone();
        changed
            .state
            .player
            .inventory
            .insert("iron_ingot".into(), count);
        Engine::restore(&world, changed).is_ok()
    };
    assert!(with(1));
    assert!(!with(2), "no crafting makes ingots");
}

#[test]
fn improving_clamps_vitals_only_after_training() {
    let mut world = smithy();
    let combat = world.world.combat.as_mut().unwrap();
    combat.recipes[1].requires = None;
    // Journeyman gives back the HP that Fine mail no longer does.
    combat.techniques[0].ranks[1].passive = [(Stat::Hp, 10)].into();
    let mail = world
        .items
        .iter_mut()
        .find(|i| i.id == "iron_mail")
        .unwrap();
    let gear = mail.equipment.as_mut().unwrap();
    gear.bonuses.insert(Stat::Hp, 10);
    gear.tiers[0].requires = None;
    gear.tiers[0].trains = Some(TechniqueGrant {
        technique: "smithing".into(),
        rank: None,
        xp: 10,
    });
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_mail".into())).unwrap();
    engine.execute(Equip(1)).unwrap();
    engine.execute(Move(West)).unwrap();
    engine.execute(Rest).unwrap();
    engine.execute(Move(East)).unwrap();
    let full = vitals(&engine).hp;
    assert_eq!(full, engine.player_stats().unwrap().hp);
    engine.execute(Improve(1)).unwrap();
    assert_eq!(smithing(&engine).rank, 2);
    assert_eq!(engine.player_stats().unwrap().hp, full);
    assert_eq!(
        vitals(&engine).hp,
        full,
        "the same maximum keeps the same HP"
    );
}

#[test]
fn a_save_cannot_hold_more_crafting_than_its_materials_paid_for() {
    let mut world = smithy();
    // One beetle that stays dead, one ingot a sword, and one more a Fine tier.
    let combat = world.world.combat.as_mut().unwrap();
    combat.groups[0].repeatable = false;
    combat.recipes[0].inputs[0].quantity = 1;
    let mut engine = at_the_anvil(&world, 1);
    engine.execute(Forge("iron_sword".into())).unwrap();
    let snapshot = engine.snapshot();
    assert!(Engine::restore(&world, snapshot.clone()).is_ok());
    let loads = |change: fn(&mut GameState)| {
        let mut changed = snapshot.clone();
        change(&mut changed.state);
        Engine::restore(&world, changed).is_ok()
    };
    // A second sword, a Fine tier, or the spent ingot back: each needs an
    // ingot no play could have had.
    assert!(!loads(|s| {
        let combat = s.combat.as_mut().unwrap();
        let sword = combat.gear[&1].clone();
        combat.gear.insert(2, sword);
        combat.next_gear = 3;
    }));
    assert!(!loads(|s| s
        .combat
        .as_mut()
        .unwrap()
        .gear
        .get_mut(&1)
        .unwrap()
        .tier = 1));
    assert!(!loads(|s| {
        s.player.inventory.insert("iron_ingot".into(), 1);
    }));
}

#[test]
fn a_save_cannot_drop_materials_nothing_spent() {
    let mut world = smithy();
    world.world.combat.as_mut().unwrap().groups[0].repeatable = false;
    let engine = at_the_anvil(&world, 1);
    // The one ingot, unspent: only crafting takes materials away.
    let mut dropped = engine.snapshot();
    dropped.state.player.inventory.remove("iron_ingot");
    assert!(Engine::restore(&world, dropped).is_err());
}

#[test]
fn a_save_cannot_hold_crafting_its_player_never_qualified_for() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    // An Apprentice with one sword and the lesson learned.
    let snapshot = engine.snapshot();
    assert!(Engine::restore(&world, snapshot.clone()).is_ok());
    let loads = |state: &GameState, change: fn(&mut GameState)| {
        let mut changed = engine.snapshot();
        changed.state = state.clone();
        change(&mut changed.state);
        Engine::restore(&world, changed).is_ok()
    };
    // Fine needs a Journeyman; mail needs one to forge.
    assert!(!loads(&snapshot.state, |s| {
        s.combat.as_mut().unwrap().gear.get_mut(&1).unwrap().tier = 1;
    }));
    assert!(!loads(&snapshot.state, |s| {
        let combat = s.combat.as_mut().unwrap();
        combat.gear.insert(
            2,
            Gear {
                item: "iron_mail".into(),
                equipped: false,
                tier: 0,
                enchantment: None,
            },
        );
        combat.next_gear = 3;
    }));
    // A sword forged before Bran's lesson.
    let fresh = Engine::new(&world).unwrap().state().clone();
    assert!(!loads(&fresh, |s| {
        let combat = s.combat.as_mut().unwrap();
        combat.gear.insert(
            1,
            Gear {
                item: "iron_sword".into(),
                equipped: false,
                tier: 0,
                enchantment: None,
            },
        );
        combat.next_gear = 2;
    }));
}

#[test]
fn material_bounds_count_only_recipes_the_player_qualified_for() {
    let mut world = smithy();
    let combat = world.world.combat.as_mut().unwrap();
    // One beetle, five ingots; a one-ingot sword, and a Master's five-ingot one.
    combat.groups[0].repeatable = false;
    combat.recipes[0].inputs[0].quantity = 1;
    let mut masterwork = combat.recipes[0].clone();
    masterwork.id = "masterwork_sword".into();
    masterwork.inputs[0].quantity = 5;
    masterwork.requires = Some(Condition::Technique {
        technique: "smithing".into(),
        rank: 3,
    });
    combat.recipes.push(masterwork);
    let beetle = world
        .characters
        .iter_mut()
        .find(|c| c.id == "beetle")
        .unwrap();
    beetle.combat.as_mut().unwrap().loot[0].quantity = 5;
    let mut engine = at_the_anvil(&world, 1);
    engine.execute(Forge("iron_sword".into())).unwrap();
    assert_eq!(ingots(&engine), 4);
    assert!(Engine::restore(&world, engine.snapshot()).is_ok());
    // An Apprentice's sword cost one ingot, not five: the other four remain.
    let mut dropped = engine.snapshot();
    dropped.state.player.inventory.remove("iron_ingot");
    assert!(Engine::restore(&world, dropped).is_err());
}

/// From the anvil to the shrine, and Maud's lesson.
fn to_the_altar(engine: &mut Engine<'_>) {
    engine.execute(Move(West)).unwrap();
    engine.execute(Move(North)).unwrap();
    engine.execute(Talk("maud".into())).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
    engine.execute(ChooseDialogue(1)).unwrap();
}

fn keen(piece: u64) -> Command {
    Enchant {
        piece,
        enchantment: "keenness".into(),
    }
}

#[test]
fn forge_equip_improve_enchant_and_load_keep_each_piece_its_own() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    engine.execute(Forge("iron_sword".into())).unwrap();
    to_the_altar(&mut engine);
    let events = engine.execute(keen(2)).unwrap();
    assert!(events.contains(&Event::Enchanted {
        gear: 2,
        enchantment: "keenness".into()
    }));
    assert!(events.contains(&Event::ItemsSpent {
        item: "ember_shard".into(),
        quantity: 1
    }));
    // Back at the anvil, improving keeps the enchantment.
    engine.execute(Move(South)).unwrap();
    engine.execute(Move(East)).unwrap();
    engine.execute(Improve(2)).unwrap();
    let base = engine.player_stats().unwrap().patk;
    engine.execute(Equip(2)).unwrap();
    // Fine's +6 and Keenness's +2, each counted once.
    assert_eq!(engine.player_stats().unwrap().patk, base + 8);
    let piece = &combat(&engine).gear[&2];
    assert_eq!(
        (piece.tier, piece.enchantment.as_deref()),
        (1, Some("keenness"))
    );
    let other = &combat(&engine).gear[&1];
    assert_eq!((other.tier, other.enchantment.as_deref()), (0, None));
    let restored = Engine::restore(&world, engine.snapshot()).unwrap();
    assert_eq!(restored.state(), engine.state());
    assert_eq!(restored.player_stats(), engine.player_stats());
}

#[test]
fn enchanting_is_refused_without_changing_anything() {
    let world = smithy();
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    // Not yet taught: the enchantment does not exist for the player.
    assert!(matches!(
        engine.execute(keen(1)),
        Err(EngineError::UnknownEnchantment(_))
    ));
    to_the_altar(&mut engine);
    let before = engine.state().clone();
    let warding = Enchant {
        piece: 1,
        enchantment: "warding".into(),
    };
    assert!(matches!(
        engine.execute(warding),
        Err(EngineError::DoesNotFit { gear: 1, .. })
    ));
    assert!(matches!(
        engine.execute(keen(7)),
        Err(EngineError::NoSuchGear(7))
    ));
    assert_eq!(engine.state(), &before);
    engine.execute(keen(1)).unwrap();
    assert!(matches!(
        engine.execute(keen(1)),
        Err(EngineError::AlreadyEnchanted(1))
    ));
    // Away from the altar.
    engine.execute(Move(South)).unwrap();
    engine.execute(Move(East)).unwrap();
    engine.execute(Forge("iron_sword".into())).unwrap();
    assert!(matches!(
        engine.execute(keen(2)),
        Err(EngineError::NoStation(station)) if station == "altar"
    ));
    assert!(offered(&engine)
        .iter()
        .all(|(c, _)| !matches!(c, Enchant { .. })));
}

#[test]
fn saves_reject_enchantments_the_rules_could_not_lay() {
    let mut world = smithy();
    // Beetles that stay dead: one shard, spent on one enchantment.
    world.world.combat.as_mut().unwrap().groups[0].repeatable = false;
    world.world.combat.as_mut().unwrap().recipes[0].inputs[0].quantity = 1;
    let mut engine = at_the_anvil(&world, 1);
    engine.execute(Forge("iron_sword".into())).unwrap();
    to_the_altar(&mut engine);
    engine.execute(keen(1)).unwrap();
    let snapshot = engine.snapshot();
    assert!(Engine::restore(&world, snapshot.clone()).is_ok());
    let loads = |change: fn(&mut GameState)| {
        let mut changed = snapshot.clone();
        change(&mut changed.state);
        Engine::restore(&world, changed).is_ok()
    };
    // No such enchantment; one that does not fit; the shard back unspent;
    // and an enchantment laid without knowing the art.
    assert!(!loads(|s| {
        s.combat
            .as_mut()
            .unwrap()
            .gear
            .get_mut(&1)
            .unwrap()
            .enchantment = Some("glory".into());
    }));
    assert!(!loads(|s| {
        s.combat
            .as_mut()
            .unwrap()
            .gear
            .get_mut(&1)
            .unwrap()
            .enchantment = Some("warding".into());
    }));
    assert!(!loads(|s| {
        s.player.inventory.insert("ember_shard".into(), 1);
    }));
    assert!(!loads(|s| {
        s.combat.as_mut().unwrap().techniques.remove("enchanting");
    }));
}

#[test]
fn a_technique_only_crafting_teaches_needs_the_crafting_in_the_save() {
    let mut world = smithy();
    // Maud tells, but does not teach: laying Keenness is the only lesson.
    let maud = world.dialogues.iter_mut().find(|d| d.id == "maud").unwrap();
    maud.nodes[1].choices[0].effects.clear();
    let combat = world.world.combat.as_mut().unwrap();
    combat.enchantments[0].requires = None;
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    to_the_altar(&mut engine);
    engine.execute(keen(1)).unwrap();
    assert!(combat_state_has(&engine, "enchanting"));
    let snapshot = engine.snapshot();
    assert!(Engine::restore(&world, snapshot.clone()).is_ok());
    // The technique, but no enchanted piece to show for it.
    let mut unearned = snapshot.clone();
    let gear = unearned
        .state
        .combat
        .as_mut()
        .unwrap()
        .gear
        .get_mut(&1)
        .unwrap();
    gear.enchantment = None;
    assert!(Engine::restore(&world, unearned).is_err());

    // An enchantment that needs the very technique it would teach explains
    // nothing: it could only be laid by someone who knew it already.
    world.world.combat.as_mut().unwrap().enchantments[0].requires = Some(Condition::Technique {
        technique: "enchanting".into(),
        rank: 1,
    });
    let mut circular = snapshot;
    circular.package_revision = world.revision();
    assert!(Engine::restore(&world, circular).is_err());
}

fn combat_state_has(engine: &Engine<'_>, technique: &str) -> bool {
    combat(engine).techniques.contains_key(technique)
}

#[test]
fn a_save_judges_condition_trees_by_what_could_once_have_held() {
    let mut world = smithy();
    let flag = |flag: &str| Condition::Flag { flag: flag.into() };
    let sword = &mut world.world.combat.as_mut().unwrap().recipes[0];
    // Known only before enchanting is taught; usable with Smithing or that lesson.
    sword.known_when = Some(Condition::All {
        of: vec![
            flag("taught_forging"),
            Condition::Not {
                condition: Box::new(flag("taught_enchanting")),
            },
        ],
    });
    sword.requires = Some(Condition::Any {
        of: vec![
            Condition::Technique {
                technique: "smithing".into(),
                rank: 1,
            },
            flag("taught_enchanting"),
        ],
    });
    let mut engine = at_the_anvil(&world, 5);
    engine.execute(Forge("iron_sword".into())).unwrap();
    let snapshot = engine.snapshot();
    assert!(Engine::restore(&world, snapshot.clone()).is_ok());
    // The `not` held when the sword was forged, even if the flag is set now.
    let mut later = snapshot.clone();
    later.state.flags.insert("taught_enchanting".into());
    assert!(Engine::restore(&world, later).is_ok());

    // A flag that is required outright must still be set: flags never clear.
    let sword = &mut world.world.combat.as_mut().unwrap().recipes[0];
    sword.requires = Some(flag("taught_enchanting"));
    let mut unearned = snapshot;
    unearned.package_revision = world.revision();
    assert!(matches!(
        Engine::restore(&world, unearned),
        Err(EngineError::InvalidSave(why)) if why.contains("crafting")
    ));
}
