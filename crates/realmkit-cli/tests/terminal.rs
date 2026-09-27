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
    let output = run(&["play", WORLD], "go east\ntalk elder\nchoose 1\nchoose 1\nnorth\nattack wolf\nattack wolf\nattack wolf\nsouth\ntalk elder\nchoose 1\ninventory\nstatus\nquests\neast\ndown\nquit\n");
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
    let output = run(&["play", WORLD], "3\n1\n1\n1\n2\n1\n1\n1\n1\n1\n1\n3\n2\n");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "1. Talk to Elder Mara",
        "2. Travel north — The Pine Track",
        "3. Travel east — The Roofless Chapel [locked]",
        "The elder keeps the chapel gate locked",
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
    assert!(text.contains("HP 24/24"));
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
    let first = "talk elder\nchoose 1\nchoose 1\nnorth\nattack wolf\n";
    let rest = "attack wolf\nattack wolf\nsouth\ntalk elder\nchoose 1\nstatus\n";
    let uninterrupted = responses(&run(&["play", WORLD], &format!("{first}{rest}")));
    let dir = saves_dir("resume");
    let saved = run(
        &["play", WORLD, "--line", "--saves", &dir],
        &format!("{first}save\nquit\n"),
    );
    assert!(responses(&saved)[6].starts_with("Saved."));
    let resumed = run(&["play", WORLD, "--saves", &dir], &format!("{rest}load\n"));
    assert!(resumed.status.success());
    let resumed = responses(&resumed);
    assert!(resumed[0].contains("Loaded save 2."), "{}", resumed[0]);
    assert_eq!(resumed[1..=6], uninterrupted[6..=11]);
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
        "HP 24/24",
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
fn saving_is_off_without_a_saves_directory() {
    let output = run(&["play", WORLD], "save\n");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Saving is off"));
    assert!(!run(&["play", WORLD, "--saves"], "").status.success());
    assert!(!run(&["validate", WORLD, "--line"], "").status.success());
}
