# RealmKit scaffold

The project brief is the design: static, editable world packages feed a synchronous,
deterministic engine. Generation is a separate build-time activity. This first slice
proves the complete single-player loop with authored content.

## Implementation sequence

- [x] Spec: serde domain types, versioned JSON directory loader, references and
  template validation. Test valid demo content and malformed packages.
- [x] Engine: explicit in-memory state and command/event API. Test the entire quest
  loop, identical replays, rejected commands, death and reward idempotence.
- [x] Presentation: line-oriented CLI for play, validate and inspect; deterministic
  parsing and stored prose rendering. Exercise the real binary with a scripted game.
- [x] Worldgen: validate/export authored WorldSpec values without overwriting an
  existing destination; typed create/update/link operations and structured
  diagnostics. No AI provider or source compiler yet.
- [x] Menus (M1): the engine lists context-sensitive actions with availability;
  the CLI numbers them, supports arrows/Enter/Esc via crossterm in a terminal
  and numbered lines elsewhere. Viewing panels does not advance the turn.
- [x] Saves (M2): the engine produces and validates storage-neutral
  `SaveSnapshot`s bound to the package ID, content revision and the implicit
  `default` route. The CLI stores them in a `--saves` directory with a lineage
  index, auto-saves at route start and quest completion, and restores the newest
  save on death.
- [x] Optional combat (M3a): package Format 2 merges NPCs and monsters into one
  character list with optional dialogue and combat components, names the player
  as a character, and moves levels and combat prose into an optional `combat`
  block. Combat state is `GameState.combat`, absent without combat. Quests can
  complete on a flag. Format 1 packages and saves are rejected, not migrated.
  The combat-free `examples/quiet-archive` fixture proves the path.
- [x] Documentation and verification: explain content/rules and deferred scope;
  run formatting, workspace tests, Clippy, and an independent engine/CLI build.

## Concrete boundaries

`realmkit-spec` owns serializable content and package validation; engine and
worldgen depend only on spec. CLI depends on engine and spec. Only serde,
serde_json and thiserror are needed by the core crates; the CLI adds crossterm
for raw terminal key input.

The package uses fixed JSON filenames, with world metadata (including the
optional combat block), locations, characters, items, quests and dialogues in
separate files. IDs are
explicit strings, exits are directed, and every reference is validated on load.
Unknown fields and unsupported versions fail early. The format implements only
the MVP domains, with extensions requiring an explicit format/version decision.

Combat is optional. Where a world has it, combat uses fixed integer damage, one
instance per fighting character, and no respawns. Defeats grant fixed loot/XP exactly once; active kill quests
advance on defeat. Quest completion is an explicit dialogue choice at the giver
and sets an authored flag that unlocks a ruins exit. Dialogue choices can require
flags or quest states. A lethal enemy response ends play; with saves on, the
CLI restores the newest save. Level thresholds are cumulative.

A package revision is a 64-bit FNV-1a digest of the package's canonical JSON, so
any content edit makes older saves incompatible; there are no migrations yet.
Loading checks the format version, package, revision and route, then the state
invariants the rules maintain (known IDs, combat state present exactly when the
world has combat, stats matching the level, opponent HP within authored limits,
quest states matching defeats and flags, a valid conversation). The CLI writes each save to a new
file, then updates `lineage.json`; both writes go to a temporary file first and
are then renamed into place.

Tests must catch dangling references, duplicate IDs, invalid stats and templates,
remote interactions, invalid/hidden dialogue choices, early quest completion,
repeated rewards, arithmetic overflow, and EOF/invalid CLI input. Engine state is
read-only to callers; failed commands leave it unchanged. Template substitutions
are single-pass so inserted values are never interpreted as template syntax.

## Authoring extensions

External agents drive a typed `WorldDraft`; future Rust/CLI/MCP adapters share
that core. Validation returns serializable diagnostics with severity, entity ID,
code and message. Canon IR and source provenance belong to authoring, separate
from playable runtime types; future provenance can live in an optional package
sidecar keyed by runtime entity IDs. A simulator can consume WorldSpec and replay
engine Commands, with no need to put generation into the engine.

All player-facing text in a source-backed world must be in the same language
as its source. This includes authored names, descriptions, dialogue, choices,
quests, story passages and templates, plus client-owned labels, help, prompts,
status text and runtime errors shown during play. World metadata declares the
content language; a source-backed draft checks it against the declared source
language. This checks metadata only: the author must review language and literary
fidelity. Machine-facing schema keys, IDs, enum values and stable typed-command
tokens may remain language-neutral ASCII.

The hand-authored English demo has no source input. The current CLI still contains
English-only fixed interface labels (centralized in its menu/presentation code).
Full package-language UI localization remains future presentation work, not an
exception to the source-language invariant.

Combat prose selection remains an implementation detail. M1 ensures
presentation-only inspection commands do not advance the turn, so browsing no
longer perturbs later combat prose. Future formats may use more semantically local
event/encounter counters when needed.
