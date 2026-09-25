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
- [x] Documentation and verification: explain content/rules and deferred scope;
  run formatting, workspace tests, Clippy, and an independent engine/CLI build.

## Concrete boundaries

`realmkit-spec` owns serializable content and package validation; engine and
worldgen depend only on spec. CLI depends on engine and spec. Only serde,
serde_json and thiserror are needed; the CLI uses standard-library I/O.

The package uses fixed JSON filenames, with world metadata, locations, NPCs,
monsters, items, quests, dialogues and narrative in separate files. IDs are
explicit strings, exits are directed, and every reference is validated on load.
Unknown fields and unsupported versions fail early. The format implements only
the MVP domains, with extensions requiring an explicit format/version decision.

Combat uses fixed integer damage, one monster instance per authored monster ID,
and no respawns. Defeats grant fixed loot/XP exactly once; active kill quests
advance on defeat. Quest completion is an explicit dialogue choice at the giver
and sets an authored flag that unlocks a ruins exit. Dialogue choices can require
flags or quest states. A lethal enemy response ends play; start a new session to
retry. Progress is intentionally in memory. Level thresholds are cumulative.

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

All generated player-facing content must be in the same language as its source:
names, descriptions, dialogue, choices, quests, story passages, and templates.
World metadata declares the content language; a source-backed draft checks it
against the declared source language. This checks metadata only: the author must
review language and literary fidelity. The hand-authored English demo has no
source input. The initial CLI's fixed interface labels are English.
