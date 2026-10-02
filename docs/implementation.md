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
- [x] Stats, damage and skills (M3b): seven stats per level and combat profile,
  a world-named special channel, two-channel damage with no scale constant,
  skills with flat MP costs and unlock levels, opponents answering with their
  strongest affordable skill, and resting at safe locations. Saves hold only
  current vitals, XP and level. The combat block's new shape makes this package
  Format 3; older packages are rejected, not migrated. `examples/duel`
  exercises a mage build.
- [x] Encounters (M3c-1): `Engage` starts a fight on a paused initiative
  timeline where speed sets turn frequency; MP regenerates over encounter time
  and rage builds from acting and being hit; opponents use their strongest
  affordable skill. The player's vitals live in one place at a time
  (`Stance::Exploring` or `Stance::Fighting`), encounters save and resume
  exactly, and results match `scripts/combat_sim`. Package and save format 4.
- [x] Grinding (M3c-2): authored groups bring packs into one encounter,
  repeatable groups can be fought again, `Flee` escapes at the player's next
  turn unless forbidden, yielding groups end fights alive with victory or
  defeat flags, opponent levels gate their skills, and XP falls off with level
  difference as in the simulator. Package and save format 5; saves bound variable XP and
  loot instead of fixing them. `examples/arena` exercises all of it.
- [x] Seeded randomness (M3d): critical hits on skills and basic attacks draw
  from a hand-written, versioned SplitMix64 stream saved with the game, only
  in worlds that author a crit. Seeds are explicit (`Engine::new_with_seed`,
  `--seed`); refused commands draw nothing; replays are exact. Package and
  save format 6.
- [x] Stat points (M4a): levels grant points that the player allocates to the
  stats a world accepts, within caps, with an optional refund at safe places.
  Effective stats (level table + allocation) are derived by one function every
  rule uses and never saved. Package and save format 7.
- [x] Technique ranks (M4b): techniques with author-named ranks, skills and
  passive bonuses per rank, trained by use (with falloff) and by a small share
  of victory XP, taught by dialogue effects and quest rewards, and held by
  breakthrough gates. The core art's rank is shown as the realm.
  `examples/sect` exercises it. Package and save format 8.
- [x] Equipment (M4c): wearable items in authored slots with bonuses, speed
  penalties, weapon basic-attack overrides and damage modifiers (multiplying,
  immunity wins, clamped to 1/10–10). Every piece is an individual saved
  instance; equip swaps out whatever held its slots, with a before → after
  preview from the engine. Package and save format 9.
- [x] Forging and improvement (M4d): locations offer stations; recipes,
  hidden until known and gated by requirements such as a Smithing technique
  rank, forge new pieces from counted materials; improvement tiers replace a
  piece's bonuses (and speed penalty) one tier at a time. Checks precede any
  change, results are guaranteed, and crafting trains its technique.
  `examples/smithy` exercises it. Package and save format 10.
- [x] Enchanting (M4e): enchantments fit pieces by slot and add passive
  stat bonuses on top of the piece's tier, laid once per piece at a station
  for a catalyst, known and gated like recipes, and kept through
  improvement. `examples/smithy` now plays forge → equip → improve → enchant
  → save/load. Package and save format 11.
- [x] Condition trees and effect lists (M5a): `requires` and `known_when` are
  one optional condition composed with `all`, `any` and `not` over typed
  leaves, now including carried counted items; dialogue choices apply an
  ordered `effects` list, including `grant_items` and `take_items`, that
  commits or fails as a whole. Saves judge a condition by whether it could
  once have held, and items that effects hand over are loose in the
  inventory check. Package and save format 12.
- [x] World time, roads and schedules (M5b): an optional clock in minutes
  that only travel, waiting and resting move; undirected roads with travel
  minutes and conditions beside compass exits; one-shot and recurring events
  with conditions and effects; characters who move among locations on a
  schedule, drawn from their own `world` random stream; and a time-of-day
  condition. Occurrences resolve in chronological, then schedule, order, and
  saves keep only the minute and each mover's location. `examples/marches`
  exercises it. Package and save format 12.
- [x] Economy core (M6a): currency with a world-language format, goods,
  producer kinds and markets whose price index per good follows production
  and demand on a scheduled four-phase price tick drawn from its own
  `market` stream, linked markets that converge, merchants who must be
  present, buying and selling with a spread and per-unit index steps, and
  currency conditions and effects. `scripts/combat_sim/economy.py` mirrors
  the tick and the trade prices, and engine tests pin its numbers. Package
  and save format 12.
- [x] Consumables and wares (M4, part of M6a-2): items may restore HP and
  MP when used, capped at the maxima and refused when they would restore
  nothing; in an encounter a use is the player's turn, one basic action
  long. Markets may sell wares at a fixed price, buy-only and unlimited,
  equipment arriving as pieces. No new saved state; saves accept used-up
  consumables and freely bought wares. `examples/arena` exercises both.
  Package and save format 13.
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

Combat is optional. Where a world has it, damage comes from the two-channel
formula in `realmkit-engine::damage`, with checked integer arithmetic and one
final rounding; there is one instance per fighting character and no respawns. Defeats grant fixed loot/XP exactly once; active kill quests
advance on defeat. Quest completion is an explicit dialogue choice at the giver
and sets an authored flag that unlocks a ruins exit. Dialogue choices can require
flags or quest states. A lethal enemy response ends play; with saves on, the
CLI restores the newest save. Level thresholds are cumulative.

A package revision is a 64-bit FNV-1a digest of the package's canonical JSON, so
any content edit makes older saves incompatible; there are no migrations yet.
Loading checks the format version, package, revision and route, then the state
invariants the rules maintain (known IDs, combat state present exactly when the
world has combat, XP matching the level, vitals within their maximums, and an
encounter whose participants, times and remainders the rules could produce,
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
