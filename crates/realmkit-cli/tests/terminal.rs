use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn run(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_realmkit"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

const WORLD: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/demo-world");

#[test]
fn terminal_plays_the_complete_authored_journey() {
    let output = run(&["play", WORLD], "go east\ntalk elder\nchoose 1\nchoose 1\nnorth\nengage wolf\nattack wolf\nattack wolf\nattack wolf\nsouth\ntalk elder\nchoose 1\ninventory\nstatus\nquests\neast\ndown\nquit\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "keeps the chapel gate locked",
        "You strike",
        "Level 2",
        "XP 15",
        "Mara's candle",
        "She opens the chapel gate",
        "Your journey through the demo is complete.",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    assert!(!text.contains("{attacker}"));
    assert!(!text.contains("{damage}"));
}

#[test]
fn numbered_menus_alone_finish_the_demo() {
    // 3 first: the locked chapel gate explains itself instead of opening.
    let output = run(
        &["play", WORLD],
        "3\n1\n1\n1\n2\n1\n1\n1\n1\n1\n1\n1\n3\n2\n",
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "1. Talk to Elder Mara",
        "2. Travel north — The Pine Track",
        "3. Travel east — The Roofless Chapel [locked]",
        "The elder keeps the chapel gate locked",
        "1. Engage The Ash Wolf",
        "1. Attack The Ash Wolf",
        "Level 2",
        "She opens the chapel gate",
        "Your journey through the demo is complete.",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    assert!(!text.contains("talk elder"));
    let output = run(&["play", WORLD], "9\nstatus\n");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Choose one of the listed numbers"));
}

#[test]
fn eof_and_bad_input_do_not_crash_or_mutate_gameplay() {
    let output = run(
        &["play", WORLD],
        "\nattack\ngo sideways\nchoose 0\nchoose 999999999999999999999999\nstatus\n",
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("HP 40/40"));
    assert!(text.contains("XP 0"));
    assert!(text.contains("Invalid command"));
    assert!(!text.contains("panicked"));
}

#[test]
fn validates_inspects_and_returns_failure_for_invalid_invocations() {
    let output = run(&["validate", WORLD], "");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("valid"));
    let output = run(&["inspect", WORLD], "");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("4 locations"));
    assert!(!run(&["play", WORLD, "extra"], "").status.success());
    assert!(!run(&["validate", "/realmkit-no-such-world"], "")
        .status
        .success());
    let output = run(&["--help"], "");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("realmkit play"));
}

fn saves_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("realmkit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_str().unwrap().into()
}

/// Responses to each command, split on the prompt; element 0 is the opening.
fn responses(output: &Output) -> Vec<String> {
    let text = String::from_utf8(output.stdout.clone()).unwrap();
    text.split("\n> ").map(String::from).collect()
}

#[test]
fn saving_quitting_and_resuming_matches_uninterrupted_play() {
    let first = "talk elder\nchoose 1\nchoose 1\nnorth\nengage wolf\nattack wolf\n";
    let rest = "attack wolf\nattack wolf\nsouth\ntalk elder\nchoose 1\nstatus\n";
    let uninterrupted = responses(&run(&["play", WORLD], &format!("{first}{rest}")));
    let dir = saves_dir("resume");
    let saved = run(
        &["play", WORLD, "--line", "--saves", &dir],
        &format!("{first}save\nquit\n"),
    );
    assert!(responses(&saved)[7].starts_with("Saved."));
    let resumed = run(&["play", WORLD, "--saves", &dir], &format!("{rest}load\n"));
    assert!(resumed.status.success());
    let resumed = responses(&resumed);
    assert!(resumed[0].contains("Loaded save 2."), "{}", resumed[0]);
    assert_eq!(resumed[1..=6], uninterrupted[7..=12]);
    assert!(resumed[6].contains("XP 15"));
    // Route start, the manual save, and the quest-completion auto-save.
    assert!(
        resumed[7].contains("1. auto-save\n  2. save\n  3. auto-save"),
        "{}",
        resumed[7]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_corrupt_newest_save_is_reported_and_older_saves_are_offered() {
    let dir = saves_dir("corrupt");
    run(&["play", WORLD, "--saves", &dir], "north\nsave\nquit\n");
    std::fs::write(format!("{dir}/1.json"), "{ not json").unwrap();
    let output = run(&["play", WORLD, "--saves", &dir], "load\nload 1\nstatus\n");
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "Save 2 could not be loaded",
        "Type load <number> to restore an older save:\n  1. auto-save",
        "Starting a new game; older saves stay available with load.",
        // The fresh start is auto-saved, so death recovery cannot hit the bad save.
        "  3. auto-save",
        "Loaded save 1.",
        "HP 40/40",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    let output = run(&["play", WORLD, "--saves", &dir], "load\n");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Loaded save 1."));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn resuming_mid_conversation_repeats_the_line_being_answered() {
    let dir = saves_dir("dialogue");
    run(
        &["play", WORLD, "--saves", &dir],
        "talk elder\nsave\nquit\n",
    );
    let output = run(&["play", WORLD, "--saves", &dir], "");
    let text = String::from_utf8(output.stdout).unwrap();
    let resumed = &text[text.find("Loaded save 2.").expect(&text)..];
    assert!(
        resumed.contains("Elder Mara: 'You have been looking at the bell,'"),
        "{resumed}"
    );
    assert!(
        resumed.contains("1. What troubles the village?"),
        "{resumed}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn saving_is_off_without_a_saves_directory() {
    let output = run(&["play", WORLD], "save\n");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Saving is off"));
    assert!(!run(&["play", WORLD, "--saves"], "").status.success());
    assert!(!run(&["validate", WORLD, "--line"], "").status.success());
}

const ARCHIVE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/quiet-archive");

#[test]
fn a_world_without_combat_plays_by_menus_with_no_fighting() {
    let dir = saves_dir("archive");
    // Accept, learn where the map is, report back, and enter the vault.
    let input = "help\nstatus\n1\n1\n1\n2\n1\n1\n2\n1\n1\n3\n";
    let output = run(&["play", ARCHIVE, "--line", "--saves", &dir], input);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "talk <character-id>",
        "\n> You\n",
        "1. Talk to Old Copyist",
        "You know where the map is.",
        "Received: Vault key ×1",
        "Your visit to the archive is complete.",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    for absent in ["attack", "Attack", "HP", "XP", "Level"] {
        assert!(!text.contains(absent), "unexpected {absent:?} in {text}");
    }
    let resumed = run(&["play", ARCHIVE, "--saves", &dir], "load\n");
    assert!(String::from_utf8(resumed.stdout)
        .unwrap()
        .contains("Loaded save 2."));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn format_1_packages_are_refused_with_a_clear_message() {
    let dir = saves_dir("format1");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        format!("{dir}/world.json"),
        r#"{ "format_version": 1, "player_name": "You" }"#,
    )
    .unwrap();
    let output = run(&["validate", &dir], "");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("older packages are not migrated"));
    std::fs::remove_dir_all(dir).unwrap();
}

const DUEL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/duel");

#[test]
fn a_mage_duels_with_skills_rests_and_levels_up() {
    let input =
        "help\nnorth\nengage witch\nuse bolt witch\n3\nuse fireball witch\n2\n2\nstatus\nsouth\nrest\nstatus\n";
    let output = run(&["play", DUEL], input);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "engage <character-id>",
        "use <skill-id> <character-id>",
        "1. Engage The Hedge Witch",
        // The faster witch acts before the player's first command.
        "You face The Hedge Witch.\nThe Hedge Witch hisses a hex at You: 6 damage.",
        // Vitals of both sides, MP only where there is some, and the projected order.
        "You HP 28/34 · MP 24/24 | The Hedge Witch HP 30/30 · MP 5/10\nProjected turns: You, The Hedge Witch, You, The Hedge Witch, You",
        "2. Spark on The Hedge Witch",
        "3. Bolt on The Hedge Witch — 12 MP",
        "You loose a bolt of witchlight. The Hedge Witch takes 11 damage.",
        "3. Bolt on The Hedge Witch — 12 MP [not enough MP]",
        "you have not reached the level for fireball",
        "You flick a spark at The Hedge Witch: 5 damage.",
        "The Hedge Witch rakes You with a cold touch: 4 damage.",
        "concedes the hedge",
        "The fight is over.",
        "Level 2! Health and MP restored.",
        "Witchcraft attack 12 · Witchcraft defence 7",
        "You rest. Health and MP restored.",
        "HP 40/40 · MP 30/30",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
}

#[test]
fn resting_without_mp_mentions_only_health() {
    let text = String::from_utf8(run(&["play", WORLD], "rest\n").stdout).unwrap();
    assert!(text.contains("You rest. Health restored."), "{text}");
    assert!(!text.contains("MP"), "{text}");
}

const ARENA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena");

#[test]
fn the_arena_plays_packs_flight_sparring_and_a_no_flee_boss() {
    let input = [
        "help",
        "north",
        "engage grey_wolf",
        "flee",
        "south",
        "east",
        "engage rat",
        "attack rat",
        "attack rat",
        "engage rat",
        "attack rat",
        "attack rat",
        "inventory",
        "west",
        "down",
        // Hurt players yield sooner: rest before sparring.
        "rest",
        "west",
        "engage holt",
        "attack holt",
        "attack holt",
        "attack holt",
        "attack holt",
        "attack holt",
        "east",
        "down",
        "engage ogre",
        "flee",
    ]
    .join("\n");
    let output = run(&["play", ARENA], &format!("{input}\n"));
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "flee\n",
        // The pack: both wolves join and act before the player's first turn.
        "You face The Grey Wolf, The Black Wolf.",
        "You HP 54/60 · rage 2 | The Grey Wolf HP 24/24 · rage 1 | The Black Wolf HP 24/24 · rage 1",
        "Flee",
        "You turn to run.",
        "You get away.",
        // The warren can be fought again; each rat leaves a tail.
        "Rat tail ×2",
        "Sergeant Holt bars the pit until you best him in the ring.",
        "Sergeant Holt yields.",
        "The Pit Ogre (HP 90)",
        "there is no running from this fight",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    let pit = &text[text.rfind("You face The Pit Ogre.").unwrap()..];
    assert!(!pit.contains(". Flee"), "{pit}");
}

#[test]
fn the_arena_quartermaster_sells_a_draught_that_heals_mid_fight() {
    // The wolves bite before the first turn; the draught is drunk in the
    // second bout, as a turn, with the fight menu offering it.
    let input = [
        "market",
        "buy healing_draught",
        "north",
        "engage grey_wolf",
        "flee",
        "engage grey_wolf",
        "use healing_draught",
        "flee",
        "south",
        "east",
        "engage rat",
        "attack rat",
        "west",
        "sell rat_tail",
        "use healing_draught",
    ]
    .join("\n");
    let output = run(&["play", ARENA, "--seed", "7"], &format!("{input}\n"));
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "Market at The Arena Gate:",
        "Wares:",
        "Healing draught — 8 marks",
        // Gear shows what it would do before it is bought.
        "Iron mail — 60 marks (Defence +6, Speed -10)",
        "Bought: Healing draught ×1 for 8 marks",
        "Use item ›",
        "You use Healing draught: +",
        "Sold: Rat tail ×1 for",
        // Nothing left to drink.
        "you do not have enough healing_draught",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
}

#[test]
fn a_seed_reproduces_a_run_with_random_content() {
    // Spar to open the pit, then trade heavy blows with the ogre: both can crit.
    let mut script = "west\nengage holt\n".to_string() + &"attack holt\n".repeat(5);
    script += "east\ndown\nengage ogre\n";
    script += &"use heavy_blow ogre\nattack ogre\n".repeat(10);
    let seeded = |seed: &str| run(&["play", ARENA, "--seed", seed], &script).stdout;
    let first = seeded("5");
    assert_eq!(first, seeded("5"));
    assert!(String::from_utf8(first).unwrap().contains("Seed: 5\n"));
    // Some seed gives a critical hit on the way through the arena.
    assert!((0..30).any(|seed| {
        String::from_utf8(seeded(&seed.to_string()))
            .unwrap()
            .contains("Critical hit!")
    }));
    // Without --seed the clock picks one and shows it.
    let clocked = String::from_utf8(run(&["play", ARENA], "").stdout).unwrap();
    assert!(clocked.contains("Seed: "), "{clocked}");
    // Worlds without random content have no seed to show.
    let demo = String::from_utf8(run(&["play", WORLD, "--seed", "5"], "").stdout).unwrap();
    assert!(!demo.contains("Seed"), "{demo}");
    assert!(!run(&["play", ARENA, "--seed", "five"], "").status.success());
    assert!(!run(&["play", ARENA, "--seed"], "").status.success());
}

#[test]
fn stat_points_are_spent_from_the_menu_or_typed_and_refunded_at_the_gate() {
    let input =
        "help\nstatus\n6\n2\nallocate hp 2\nstatus\nallocate satk\nallocate hp 9\nrespec\nstatus\n";
    let output = run(&["play", ARENA, "--seed", "1"], input);
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "allocate hp|mp|patk|pdef|satk|sdef|speed [points]",
        "Speed 100\n  XP 0 (level 2 at 10) · 3 points to spend\n",
        "6. Train stats — 3 points ›",
        "\nTrain stats — 3 points\n  1. Train HP",
        "2. Train Attack: 14 → 15",
        "5. Back",
        "1 point into Attack.",
        "2 points into HP.",
        "You — Level 1\n  HP 70/70\n  Attack 15 ·",
        "you cannot spend points on that stat",
        "not enough unspent stat points",
        "Refund stat points",
        "Your stat points are refunded.",
        // After the refund, HP is back within the level's maximum.
        "You — Level 1\n  HP 60/60\n  Attack 14 ·",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    // A world without stat points neither lists nor accepts them.
    let demo = String::from_utf8(run(&["play", WORLD], "help\nallocate hp\n").stdout).unwrap();
    assert!(!demo.contains("allocate hp|"));
    assert!(demo.contains("you cannot spend points on that stat"));
}

const SECT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/sect");

#[test]
fn the_sect_teaches_trains_and_deepens_named_ranks() {
    let walkthrough = std::fs::read_to_string(format!("{SECT}/walkthrough.txt")).unwrap();
    let output = run(&["play", SECT, "--line"], &walkthrough);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    // Line mode and pipes stay plain text.
    assert!(!text.contains('\u{1b}'), "{text:?}");
    for passage in [
        "You — Level 1 · Realm First Layer\n  HP 50/50 · MP 30/30\n",
        "You learn Cloud Palm.",
        "2. Cloud Palm · Drifting Cloud on A Straw Dummy — 5 MP",
        // Technique XP from a fight arrives as one line when it ends.
        "The fight is over.\n+5 XP\nTechnique XP: Cloud Palm +20, Azure Breath +1\n",
        "Cloud Palm: Gathering Storm!",
        "2. Cloud Palm · Gathering Storm on A Straw Dummy — 6 MP",
        "  Cloud Palm — Gathering Storm (mastered)",
        "Azure Breath +15",
        "Azure Breath: Second Layer!",
        "  Azure Breath — Second Layer (17/30 to Third Layer)",
        "Realm Second Layer",
        // XP shows the next level and where it starts.
        "XP 10 (level 2 at 15)",
        "Techniques",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    assert!(!text.contains("Cloud Palm +10\n"), "{text}");
    // A reward without XP says nothing about XP.
    assert!(!text.contains("+0 XP"), "{text}");
    let help = String::from_utf8(run(&["play", SECT], "help\n").stdout).unwrap();
    assert!(help.contains("techniques — learned techniques"));
    let demo = String::from_utf8(run(&["play", WORLD], "help\n").stdout).unwrap();
    assert!(!demo.contains("techniques —"));
}

#[test]
fn gear_is_listed_previewed_worn_and_removed() {
    let fight = "attack grey_wolf\n".repeat(3) + &"attack black_wolf\n".repeat(4);
    let input = format!("help\ninventory\nnorth\nengage grey_wolf\n{fight}south\n7\nequip 5\ninventory\nunequip #5\nequip 9\n");
    let output = run(&["play", ARENA, "--seed", "1"], &input);
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "equip <#> — wear a piece from your pack",
        "  #1 Practice sword [equipped] — Blunt",
        "Received: Greatsword ×1",
        "7. Equipment ›",
        "Equip #5 Greatsword: Attack 15 → 21, Defence 13 → 11",
        "#1 Practice sword goes back in your pack.\n#2 Buckler goes back in your pack.\nYou equip #5 Greatsword.",
        "  #5 Greatsword [equipped] — It needs both hands",
        "#5 Greatsword goes back in your pack.",
        "you have no equipment #9",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    // Worlds without slots neither list nor accept equipping.
    let demo = String::from_utf8(run(&["play", WORLD], "help\nequip 1\n").stdout).unwrap();
    assert!(!demo.contains("equip <#>"));
    assert!(demo.contains("you have no equipment #1"));
}

const SMITHY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/smithy");

#[test]
fn the_smithy_forges_improves_and_keeps_copies_apart() {
    let walkthrough = std::fs::read_to_string(format!("{SMITHY}/walkthrough.txt")).unwrap();
    let output = run(&["play", SMITHY, "--line"], &walkthrough);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "You learn Smithing.",
        "Received: Iron ingot ×1",
        "Spent: Iron ingot ×2\nYou forge #1 Iron sword.\nSmithing +10\n",
        "You forge #2 Iron sword.",
        "Smithing: Journeyman!",
        "Spent: Iron ingot ×1\nIt is now #1 Fine Iron sword.\n",
        "You learn Enchanting.",
        "Spent: Ember shard ×1\nIt is now #1 Fine Iron sword of Keenness.\nEnchanting +10\n",
        "  #1 Fine Iron sword of Keenness [equipped] — Plain, straight and honest.",
        "  #2 Iron sword — Plain, straight and honest.",
        "  Smithing — Journeyman",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    // At the anvil, forging and improving wait behind one entry, and what
    // cannot be done yet says why.
    let input = "talk bran\nchoose 1\nchoose 1\neast\n2\n";
    let text = String::from_utf8(run(&["play", SMITHY, "--line"], input).stdout).unwrap();
    for passage in [
        "2. Smithing ›",
        "1. Forge Iron sword — 2 Iron ingot [needs 2 Iron ingot]",
        "2. Forge Iron mail — 3 Iron ingot [needs Journeyman Smithing]",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    let help = String::from_utf8(run(&["play", SMITHY], "help\n").stdout).unwrap();
    assert!(help.contains("forge <recipe>"), "{help}");
    assert!(help.contains("improve <#>"), "{help}");
    assert!(help.contains("enchant <#> <enchantment>"), "{help}");
    // At the altar, enchanting waits behind its own entry.
    let input = format!(
        "{}west\nnorth\n",
        walkthrough.split("west\nnorth").next().unwrap()
    );
    let input = input.replace("techniques\n", "") + "talk maud\nchoose 1\nchoose 1\n4\n";
    let text = String::from_utf8(run(&["play", SMITHY, "--line"], &input).stdout).unwrap();
    for passage in [
        "4. Enchanting ›",
        "Enchant #1 Fine Iron sword → Fine Iron sword of Keenness (Attack +2) — 1 Ember shard",
        "Enchant #2 Iron sword → Iron sword of Keenness (Attack +2) — 1 Ember shard",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
}

const MARCHES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/marches");

#[test]
fn the_marches_pass_time_on_roads_and_by_waiting() {
    let dir = saves_dir("marches");
    let walkthrough = std::fs::read_to_string(format!("{MARCHES}/walkthrough.txt")).unwrap();
    let output = run(
        &["play", MARCHES, "--line", "--seed", "7", "--saves", &dir],
        &walkthrough,
    );
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "Greyford\nDay 1, 08:00\n",
        "Roads: Ashmere (2 h), Hollin Keep (4 h)\n",
        "2. Travel to Ashmere — 2 h",
        "2. Engage The fen outlaws",
        "4. Travel to Vellmarket — 3 h [locked]",
        "4. Wait — 1 h",
        "Ashmere\nDay 1, 10:00\n",
        "Roads: Greyford (2 h), Vellmarket (3 h, closed)",
        "Hollin Keep\nDay 1, 16:00\n",
        "Meltwater still covers the fen causeway.",
        "16 h later: Day 2, 14:00",
        "Vellmarket\nDay 2, 17:00\n",
        "The Reeve's Letter [carry_letter]: Completed",
        // Trading: eels from the fen sell dearer in Vellmarket, cloth in
        // Greyford. Merchants hold only so much, and trading narrows the spread.
        "6. Proficiencies — 1 point ›",
        "Market at Greyford:\n  Prosperity 48\n  Grain — buy 12 silver · sell 9 silver · stock 20 · carried 0\n",
        "  You: 100 silver · Merchants: 588 silver\n",
        "Trade is now rank 1.",
        "there is no grain left to buy here",
        "Bought: Smoked eels ×6 for 48 silver",
        "Received: 60 silver",
        "Sold: Smoked eels ×6 for 85 silver",
        "Bought: Cloth ×1 for 100 silver",
        "Sold: Cloth ×1 for 126 silver",
        "Inventory:\n  223 silver\n",
        "6. Market ›",
        // Levies raised at Ashmere fight the outlaws beside the keep's guard.
        "6. Recruit ›",
        "2. Engage The fen outlaws (14) — with Warden Cadoc",
        "Recruited: Levy ×6 for 60 silver",
        "Retinue: 6/30\n  Levy (L1) ×6 · XP 0/6\n  Wages due: 6 silver\n",
        "Battle: The fen outlaws\nWarden Cadoc joins your side.",
        "Yours: You HP 40/40 · Levy 6 · Keep guard 6 | Theirs: Outlaw 8 · Hardened outlaw 2 · Poacher 4",
        "  2. Hold the line\n",
        "Round 1 — yours: strength 25, lost 0, morale 100 · theirs: strength 14, lost 1, morale 90",
        "You run down the fleeing enemy: 1 fall.\nVictory.\nWounded: Levy ×1\nKilled: Levy ×2\n",
        "Promoted: Levy ×4 (level 2).",
        "Wages paid: 4 silver\nRecovered: Levy ×1\n",
        // A second victory makes Spearmen of them; two become riders, who
        // flank in a battle saved, fought a round, and loaded back.
        "Promoted: Spearman ×3 (level 3).",
        "Spearman ×2 become Rider.",
        "Yours: You HP 46/46 · Spearman 1 · Rider 2 · Keep guard 6",
        "Loaded save 3.",
        "  3. Flank with riders\n",
        "Round 1 — yours: strength 27, lost 0, morale 100 · theirs: strength 11, lost 3, morale 68",
        // A weavery bought from Maddoc pays out at the week's settlement.
        "You buy the Weavery in Vellmarket for 150 silver.",
        "Your workshops earn 122 silver.",
        "  Trade 1/3 · 1 proficiency point to spend\n  Workshops: Weavery in Vellmarket\n",
        "Market at Vellmarket:\n  Prosperity 40\n",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
    // Travel shows the clock once, at the destination.
    assert!(!text.contains("2 h later"), "{text}");
    let help = String::from_utf8(run(&["play", MARCHES], "help\n").stdout).unwrap();
    assert!(help.contains("travel <location-id>"));
    assert!(help.contains("buy <item> [units]"));
    assert!(help.contains("train trading [points]"));
    // The market submenu prices every good, and shows what is sold out or
    // cannot be afforded.
    let menu = run(
        &["play", MARCHES, "--line", "--seed", "7"],
        "travel ashmere\n8\n2\ntravel greyford\n7\n",
    );
    let menu = String::from_utf8(menu.stdout).unwrap();
    for passage in [
        "Market\n  1. Prices\n  2. Buy Grain — 12 silver (next 13 silver)\n",
        "4. Buy Cloth — 168 silver [sold out]",
        "Bought: Grain ×1 for 12 silver",
        "Sell Grain (1) — 8 silver (next 8 silver)",
    ] {
        assert!(menu.contains(passage), "missing {passage:?} in {menu}");
    }
    assert!(help.contains("wait [minutes]"));
    // The newest save, made mid-battle, resumes the battle; the older one
    // made on the road still loads.
    let resumed = run(&["play", MARCHES, "--line", "--saves", &dir], "load 2\n");
    let resumed = String::from_utf8(resumed.stdout).unwrap();
    assert!(resumed.contains("Loaded save 3."), "{resumed}");
    assert!(
        resumed.contains("Yours: You HP 46/46 · Spearman 1 · Rider 2 · Keep guard 6"),
        "{resumed}"
    );
    assert!(resumed.contains("Loaded save 2."), "{resumed}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_pursuit_that_catches_nobody_goes_unremarked() {
    // Alone against the outlaws, the player is knocked out in the first
    // round, so the enemy's pursuit finds nobody left to cut down.
    let output = run(
        &["play", MARCHES, "--line", "--seed", "7"],
        "travel ashmere\nengage outlaws\nautoresolve\n",
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Defeat."), "{text}");
    assert!(!text.contains("cuts down 0"), "{text}");
}
