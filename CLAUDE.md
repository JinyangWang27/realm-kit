# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test -p realmkit-engine --test techniques <name_substring>   # one test file, filtered
cargo test -p realmkit-cli menu::tests                            # unit tests in one module

cargo run -p realmkit-cli -- play examples/arena --seed 1 --line < examples/arena/walkthrough.txt
cargo run -p realmkit-cli -- validate examples/sect

python3 -m scripts.combat_sim [report|check|tune]   # balance simulator (stdlib only)
uvx ruff check scripts && uvx ruff format --check scripts   # Python lint/format (ruff.toml)
uvx mypy --strict scripts/combat_sim                        # Python types; ruff does not check them
```

Add `--offline` once dependencies are cached. Run the fmt, clippy and test commands before every commit. A pipe such as `| tail` hides failing exit codes.

## Architecture

The workspace has four crates. The CLI must not depend on worldgen or any AI dependency (`cargo tree -p realmkit-cli`).

- **`realmkit-spec`**: static world content, meaning serde types, package loading and validation. Types are split by domain (`stats`, `combat`, `techniques`, `items`, `places`, `story`) and re-exported from the crate root. `validation/` has one file per domain. Every check pushes a `Diagnostic` with a stable `code`, and tests assert on those codes.
- **`realmkit-engine`**: deterministic rules. `Engine::execute(Command) -> Vec<Event>` runs on a clone of `GameState` and commits only on success, so a refused command changes nothing. `lib.rs` is the public API only. `rules/` dispatches commands (`mod.rs`), lists `actions()`, and holds story (dialogue, quests, rewards) and player (effective stats, points, rest) rules. `encounter/` runs fights on the paused initiative timeline: flow in `mod.rs`, time in `timeline.rs`, one action in `action.rs`. Gear, techniques and the RNG each have their own module. `save/` checks that a `SaveSnapshot` is state the rules could actually have produced.
- **`realmkit-cli`**: parses input, renders events and manages save files. `main.rs` parses arguments; `session.rs` runs commands and saves; `play.rs` holds the key and line loops. `menu.rs` builds numbered menus, with submenus, from `engine.actions()`. The same menus serve raw-key play (crossterm) and line mode (pipes, `--line`). Events render in `render.rs`, state views in `panels.rs`. Fixed English interface strings live in `menu.rs`, `render.rs` and `panels.rs`.
- **`realmkit-worldgen`**: a typed authoring API (`WorldDraft`) with no AI provider.

Invariants the code relies on:

- **Determinism.** No clocks or I/O in the engine. Randomness appears only in worlds that author it (crits), drawn from a versioned SplitMix64 stream saved in state. The same world, seed and commands always produce identical events and state.
- **Derived, never saved.** Effective player stats (level table + allocated points + technique passives + worn gear) always come from `rules::player_stats`. The player's HP and MP live in exactly one place, `Stance::Exploring(Vitals)` or `Stance::Fighting(Encounter)`.
- **Formats.** Package and save formats share one version number (`FORMAT_VERSION` in spec, `SAVE_FORMAT_VERSION` in engine). A shape change bumps both and updates every `examples/*` package. Older packages and saves are rejected, never migrated. A save is bound to the package's content hash (`WorldSpec::revision`), so any content edit makes older saves unloadable.
- **Validation guards the engine.** Numeric bounds such as `STAT_BOUND`, `TIME_BOUNDS`, `SLOT_BOUND` and `GEAR_STACK_BOUND` let the engine use `unwrap()` on validated references and keep its arithmetic small. The engine still uses checked arithmetic and returns `EngineError::NumericLimit` instead of wrapping. A new content field needs both a validation check and a save check.
- **Simulator parity.** The damage, timeline and XP formulas mirror `scripts/combat_sim`. Tests in `engine/tests/encounters.rs` pin numbers produced by the simulator, so formula changes must keep both in step.
- **Language.** Player-facing text comes from the world package, in the world's declared language. IDs and typed-command tokens stay ASCII.

`examples/` holds the test worlds: `demo-world` (the quest loop), `quiet-archive` (no combat), `duel` (skills, MP, resting), `arena` (groups, flee, yielding, crits, stat points, gear), `sect` (technique ranks) and `smithy` (forging, improvement and enchanting). Each has a `walkthrough.txt`, except `quiet-archive`. The CLI terminal tests in `crates/realmkit-cli/tests/terminal.rs` run the real binary.

## Docs to update with changes

- `docs/world-format.md`: package fields.
- `docs/implementation.md`: the delivered-slice checklist.
- `ROADMAP.md`: milestones and the presentation track.
- `README.md`: player-facing behaviour.

Longer design reasoning lives in `docs/open-decisions.md`, `docs/equipment.md`, `docs/sandbox-worlds.md` and `docs/glossary.md`.

## Workflow in this repo

- Work on a branch from `main`, one commit per step, and open the PR only after the implementation is complete. Reviews come from a separate Claude Code session whose findings the user pastes in; Codex auto-review is off. Verify each finding, fix the material ones with a test that fails on the old code, and report what was fixed (with the commit) or why it was declined.
- For pure refactors, prove behaviour is unchanged: run every example walkthrough (`--line --seed 7 --saves <dir>`) on both `main` and the branch, and diff the output and the save files byte for byte.
