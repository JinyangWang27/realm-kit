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
