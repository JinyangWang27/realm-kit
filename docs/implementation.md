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
- [x] Optional combat (M3a): the package merges NPCs and monsters into one
  character list with optional dialogue and combat components, names the player
  as a character, and moves levels and combat prose into an optional `combat`
  block. Combat state is `GameState.combat`, absent without combat. Quests can
  complete on a flag.
  The combat-free `examples/quiet-archive` fixture proves the path.
- [x] Stats, damage and skills (M3b): seven stats per level and combat profile,
  a world-named special channel, two-channel damage with no scale constant,
  skills with flat MP costs and unlock levels, opponents answering with their
  strongest affordable skill, and resting at safe locations. Saves hold only
  current vitals, XP and level. `examples/duel`
  exercises a mage build.
- [x] Encounters (M3c-1): `Engage` starts a fight on a paused initiative
  timeline where speed sets turn frequency; MP regenerates over encounter time
  and rage builds from acting and being hit; opponents use their strongest
  affordable skill. The player's vitals live in one place at a time
  (`Stance::Exploring` or `Stance::Fighting`), encounters save and resume
  exactly, and results match `scripts/combat_sim`.
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
  rule uses and never saved.
- [x] Technique ranks (M4b): techniques with author-named ranks, skills and
  passive bonuses per rank, trained by use (with falloff) and by a small share
  of victory XP, taught by dialogue effects and quest rewards, and held by
  breakthrough gates. The core art's rank is shown as the realm.
  `examples/sect` exercises it.
- [x] Equipment (M4c): wearable items in authored slots with bonuses, speed
  penalties, weapon basic-attack overrides and damage modifiers (multiplying,
  immunity wins, clamped to 1/10–10). Every piece is an individual saved
  instance; equip swaps out whatever held its slots, with a before → after
  preview from the engine.
- [x] Forging and improvement (M4d): locations offer stations; recipes,
  hidden until known and gated by requirements such as a Smithing technique
  rank, forge new pieces from counted materials; improvement tiers replace a
  piece's bonuses (and speed penalty) one tier at a time. Checks precede any
  change, results are guaranteed, and crafting trains its technique.
  `examples/smithy` exercises it.
- [x] Enchanting (M4e): enchantments fit pieces by slot and add passive
  stat bonuses on top of the piece's tier, laid once per piece at a station
  for a catalyst, known and gated like recipes, and kept through
  improvement. `examples/smithy` now plays forge → equip → improve → enchant
  → save/load.
- [x] Condition trees and effect lists (M5a): `requires` and `known_when` are
  one optional condition composed with `all`, `any` and `not` over typed
  leaves, now including carried counted items; dialogue choices apply an
  ordered `effects` list, including `grant_items` and `take_items`, that
  commits or fails as a whole. Saves judge a condition by whether it could
  once have held, and items that effects hand over are loose in the
  inventory check.
- [x] World time, roads and schedules (M5b): an optional clock in minutes
  that only travel, waiting and resting move; undirected roads with travel
  minutes and conditions beside compass exits; one-shot and recurring events
  with conditions and effects; characters who move among locations on a
  schedule, drawn from their own `world` random stream; and a time-of-day
  condition. Occurrences resolve in chronological, then schedule, order, and
  saves keep only the minute and each mover's location. `examples/marches`
  exercises it.
- [x] Gregorian calendar (M5b extension): an optional `time.calendar.epoch`
  dates minute 0; the date, next midnight and next first of a month derive
  from the minute alone and are never saved; the clock gains `{year}`,
  `{month}` and `{day_of_month}` while `{day}` still counts elapsed days.
  Validation rejects impossible epochs, a minute range past the year 9999 and
  calendar placeholders without a calendar.
- [x] Economy core (M6a): currency with a world-language format, goods,
  producer kinds and markets whose price index per good follows production
  and demand on a scheduled four-phase price tick drawn from its own
  `market` stream, linked markets that converge, merchants who must be
  present, buying and selling with a spread and per-unit index steps, and
  currency conditions and effects. `scripts/combat_sim/economy.py` mirrors
  the tick and the trade prices, and engine tests pin its numbers.
- [x] Consumables and wares (M4, part of M6a-2): items may restore HP and
  MP when used, capped at the maxima and refused when they would restore
  nothing; in an encounter a use is the player's turn, one basic action
  long. Markets may sell wares at a fixed price, buy-only and unlimited,
  equipment arriving as pieces. No new saved state; saves accept used-up
  consumables and freely bought wares. `examples/arena` exercises both.
- [x] Troops and mass battles (M6c-1, M6d-1): troop lines whose soldiers
  level up in squads sharing XP, renamed where authored, with branch
  upgrades; recruiting from refilling pools; wages, desertion and recovery
  on an upkeep schedule; mass battles against armies, autoresolved or
  commanded round by round, with allies who join while a condition holds.
  The round rule draws from its own `battle` stream and mirrors
  `scripts/combat_sim/battle.py`, whose numbers engine tests pin. Saves hold
  squads and pools, and a battle between rounds. `examples/marches`
  exercises it.
- [x] Economy completion (M6a-2): markets prosper or decline one point a
  day towards an ideal their scarcities lower, and prosperity scales
  demand and merchants' stock; merchants hold stock and a purse, restocked
  on a schedule from their own `stock` stream, so trade is limited both
  ways; villages feed their market town; workshops bought and sold through
  dialogue settle weekly at local prices; and the trading proficiency,
  trained with points from the level table or taught by effects, narrows
  the spread. `scripts/combat_sim/economy.py` mirrors all of it, and a
  world without these parts keeps its earlier numbers.
  `examples/marches` exercises them.
- [x] Presentation seam and start choices (original-game P0): locations may author
  map positions and kinds, all or none; `Engine::map_view` returns the places,
  roads with travel minutes, one-way exits and the player's place, behind a
  `Map` panel; the CLI draws it as text, in a fixed 80 × 24 frame in line
  mode (`map`, `map zoom <n> <place>`) and on its own screen with zoom, pan
  and a Tab cycle in terminal play. Packages load from memory
  (`WorldSpec::from_files`), commands, events and actions serialize, and
  snapshots read and write JSON bytes, so a browser or Tauri host needs no
  filesystem or terminal; `realmkit-engine` builds for
  `wasm32-unknown-unknown`. Start questions are answered at New Game, their
  options' effects applied to the starting state and the answers saved.
  `examples/marches` has positions; `examples/quiet-archive` asks why the
  player came.
- [x] MVP progression and investigation: evidence definitions,
  optionally linked to an item, discovered by `discover_evidence` and read
  by an `evidence` condition; story phases moved forward only by
  `enter_phase`, with a `phase` condition; `main` quests and quest
  `requires`, refused with `QuestLocked` until met; route outcomes recorded
  once after the command that meets them, refusing ambiguity; places hidden
  from the map and the roads until their `known_when` holds; characters of
  kind `feature`, examined rather than talked to; and `Engine::journal`,
  which the CLI's Quests panel prints. Validation rejects undiscoverable
  evidence, unreachable phases, prerequisite cycles and main quests waiting
  on side quests; saves check evidence, phases, prerequisites and the
  outcome. `examples/caravan-trail` plays the MVP slice by combat,
  bribery, intimidation or a survivor's testimony.
- [x] Evidence readings: an evidence definition may give its `source`, the
  `facts` observed and `interpretations` unlocked by evidence, flags or
  phases (`fleeting_interpretation` refuses anything that could revert).
  The current reading is derived (`Engine::evidence_reading`), never saved,
  and a change to known evidence's reading is reported as
  `EvidenceReinterpreted`; the CLI journal prints it. The quiet archive's
  stitched map reads kinder once the vault opens.
- [x] Authoring conveniences for the original game's first slice:
  an `at_least` condition (`at_least_zero`, `at_least_exceeds`), allowed
  wherever `all` and `any` are, including readings; evidence facts gated by
  a lasting `when` (`fleeting_fact`), derived by `Engine::evidence_facts` and
  announced as `EvidenceFactLearned`; dialogue choices with an `id`, asked
  `once` (saved as `GameState.taken_choices`, checked on load) or going
  `back` to a hub until it offers only leaving (`once_without_id`,
  `invalid_back`); unavailable choices listed with `blocked_text`
  (`DialogueOption`, refused with `ChoiceBlocked`,
  `unconditional_blocked_text`); and language overlays in `text/<tag>.json`
  (`missing_translation`, `unused_translation`, `invalid_translation`,
  `WorldSpec::in_language`, `realmkit play --language`, `realmkit text`).
  Saves bind to a rules-only revision, so prose fixes and added languages
  keep them loadable. `caravan-trail` shows the first, second and fourth;
  `quiet-archive` asks a hub of questions and ships a Simplified Chinese
  overlay.
- [x] Documentation and verification: explain content/rules and deferred scope;
  run formatting, workspace tests, Clippy, and an independent engine/CLI build.

## Next implementation priorities

The open items are **not delivered** and do not change the format by
themselves. They mirror the original-game-driven roadmap priority while keeping
every capability optional:

- [x] Overland-map authored placement + engine map query, with
  places known by an authored condition.
- [x] Embeddable graphical-client seam, including a browser/WASM hosting proof
  without terminal or native-filesystem assumptions.
- [x] Start choices.
- [x] First-class authored story-phase/main-side-questline/outcome
  progression, as far as the original game's MVP uses it. Failure,
  deadlines and terminal outcomes wait for content that needs them.
- [ ] Factions, war/peace and typed standing/relation tracks.
- [ ] Remaining retinue needs used by the representative sandbox world:
  companions, roster limit, provisions/morale and travel speed.
- [ ] Holdings/sieges, then world agents and faction strategy; add only the
  politics/order pieces demonstrated by the fixture.
- [ ] Shared-authority handoff and server adapters only after the complete
  offline game path is viable.

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

A package revision is a 64-bit FNV-1a digest of the package's canonical JSON
with its prose emptied and its language left out, so a rule edit
makes older saves incompatible while prose fixes and added languages do not;
there are no migrations yet.
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
