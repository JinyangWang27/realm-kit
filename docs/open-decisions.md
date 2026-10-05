# RealmKit open decisions

This is the discussion register for architectural and game-design choices that
are intentionally unresolved. It prevents an implementation detail from silently
becoming policy. Decide items when their milestone needs them; do not block M1 on
questions that only affect later generation or multiplayer work.

Normative architecture terminology is defined in the [glossary](glossary.md).
Detailed proposals already live in the [roadmap](../ROADMAP.md),
[equipment proposal](equipment.md), [capability catalog](capabilities.md), and
[authoring guide](authoring.md). This register links to those decisions rather
than replacing them.

## Agreed foundations

- Gameplay runtime is deterministic, static-package-driven and AI-free.
- All player-facing text in a source-backed world uses the source language,
  including client-owned labels, help, prompts and runtime messages. Machine-facing
  identifiers and stable typed-command tokens may remain language-neutral.
- Gameplay capabilities are optional and source-grounded. Complete absence is
  valid and produces no placeholder state or UI.
- Every playable world has a shared location graph and one or more player routes.
  Every player route has a PlayerSpec-like player-control binding to a shared
  Character, an initial state/location, exactly one main questline, and one or more
  authored outcomes. Quests are RealmKit's primary story-progression abstraction.
  Side questlines are optional content layered over the same shared world.
- Main-story progress unlocks bounded sets of side questlines through explicit
  story-phase/quest conditions. This preserves open exploration within a phase
  without allowing the player to consume an unlimited independent game while
  permanently ignoring the main story.
- Side quests may affect the main questline through explicit, pre-authored state
  such as flags, relationships, evidence, items, NPC state, routes, dialogue and
  objective alternatives. They never rewrite the main story dynamically at runtime.
- Inventory is optional. Evidence belongs to investigation state, not inventory;
  a physical item may be linked to an evidence definition, but possession alone
  does not automatically make an item evidence.
- Combat and crafting are optional. Equipment does not imply crafting.
- World generation offers canonical, original, or both player-entry modes.
  Canonical and original routes are independently playable; `both` exposes the
  choice at New Game rather than requiring one route to unlock the other.
- World characters are shared entities; `PlayerSpec` binds control of one
  character to a route. Player and protagonist are not synonymous.
- Source adaptations use protected canon anchors/constraints with bounded
  expansion between them. Required downstream anchors must remain reachable; no
  unrestricted/free adaptation mode exists.
- Failed commands do not mutate state. Presentation uses structured commands and
  events shared by typed commands, menus and future clients.
- Combat uses gradual defence reduction with explicit immunity/vulnerability.
  Immunity overrides minimum damage; vulnerability and defence bypass differ.
- Story phase is universal narrative progression; world clock/calendar time is
  optional; encounter timing is local scheduling state; real-world thinking time
  advances none of them.
- Speed controls action frequency on a proposed paused encounter timeline.
  Effective speed has a cap, and heavy armour can trade speed for protection.
- Recipe knowledge and proficiency are separate: authored sources teach recipes;
  proficiency gates their use.

These decisions can still be revised deliberately, but they are not open by
default during implementation.

## 1. Minimum universal world core

**Resolved.**

RealmKit separates the shared fictional world from an independently playable
player route:

```text
World
├── identity / language
├── shared location graph
├── shared characters / NPCs
├── factions / organizations
├── shared entities / lore
├── authored narrative / dialogue / choices
├── story-phase definitions
├── typed conditions / effects / world state
│
└── PlayerRoutes [1..N]
    └── PlayerRoute
        ├── PlayerSpec
        ├── initial state / starting location
        ├── exactly one main questline
        │   └── quests
        │       └── typed objectives
        └── one or more authored outcomes
```

Decisions:

- `World` is the shared fictional environment. Canonical characters, locations,
  factions and reusable entities belong to the world rather than being duplicated
  inside a route or quest.
- Every world has at least one `PlayerRoute`. A route is one independently
  playable way of entering and experiencing that world.
- Every route has one player-control binding (`PlayerSpec`) referencing a
  shared `Character`, plus initial state/location, exactly one main questline, and
  one or more outcomes.
- A questline contains quests; quests contain one or more typed objectives.
- Story phases belong to the shared world/canonical chronology. Route progression
  can trigger authored phase transitions.
- Side questlines are optional and may be shared or route-gated as authored.
- The location graph is universal. A constrained story may use only one or a few
  locations, but movement/place remains part of RealmKit's world model.
- Factions/organizations may exist as ordinary world entities without enabling a
  faction/reputation gameplay capability.
- Inventory is optional.
- Evidence is owned by an investigation capability. Evidence may refer to a
  physical entity such as a letter, weapon or photograph, including an item that
  can also be carried in inventory, but evidence state is separate from item
  possession. Prefer a typed relation/reference over an unstructured string tag.
- `Campaign` is not a normative RealmKit architecture term. Use `World` for the
  shared setting and `PlayerRoute` for an independently playable entry/storyline.
  Introduce a first-class campaign abstraction only if a future requirement proves
  that these concepts are insufficient.

Avoid answering future format questions by making every field optional. The core
should express the smallest real playable world and route clearly.

## 2. Open-world quest progression

**Resolved foundation:** RealmKit should support a Skyrim-like open structure over
a deterministic, source-grounded world.

The main questline is the spine of story progression. Side questlines are
optional stories that reuse the same locations, NPCs, factions and world state.
They are not isolated mini-worlds.

Side content is unlocked in bounded waves by main-story progress, usually through
story phases and explicit quest/flag conditions. A player may freely explore and
complete available side content before advancing the main quest, but later side
questlines can remain unavailable until the main story moves forward. This keeps
the world open without making the main story irrelevant.

Conceptually:

```text
main quest progress
       ↓
   story phase
       ↓
┌──────┼──────────┐
main   side A     side B
quest  questline  questline
       ↓          ↓
       └────┬─────┘
            ↓
     explicit world state
            ↓
   later main-quest branches
```

Side quest outcomes may feed back into the main questline through typed,
pre-authored state. A main quest must not silently require completion of a side
quest; if a particular quest is required for canonical progression, it is part of
the main questline (or an explicit alternative main objective), not merely side
content. Examples include:

- dialogue/options changing because of a relationship or reputation result
- an NPC helping, refusing, moving, disappearing or surviving
- evidence or knowledge enabling a different main objective
- an item/resource opening a route or avoiding an encounter
- a side quest unlocking, replacing or skipping a main-quest objective
- an authored alternate main-story outcome when the adaptation policy permits it

This feedback must remain explicit and statically packaged. The runtime evaluates
conditions; it does not invent consequences.

For source adaptations, the default should preserve major canonical anchors while
allowing side content to alter the route, context, assistance, difficulty and
bounded outcomes around those anchors. Broader divergence belongs to an explicit
adaptation policy.

## 3. Outcomes, failure and replay

**Resolved.**

Every player route defines one or more authored outcomes. Outcome semantics use
two independent properties:

```text
completion  whether reaching this outcome counts as completing the route
terminal    whether this playthrough stops at the outcome
```

This permits, for example:

```text
canonical ending      completion=true   terminal=false
tragic ending         completion=true   terminal=true
terminal failure      completion=false  terminal=true
```

`completion=false, terminal=false` is not a route outcome; it is an ordinary
setback or state transition.

Decisions:

- A route may continue after a completed non-terminal outcome. This supports a
  Skyrim-like post-story phase in which the main quest is finished but exploration
  and remaining side content continue.
- A playthrough records at most one immutable route outcome. Continuing after a
  non-terminal outcome does not allow the same history to later become a different
  ending.
- To experience a mutually exclusive outcome, the player loads/branches from an
  earlier save or starts another playthrough. One deterministic history does not
  collect several endings.
- Failed quests, defeat, imprisonment, injury and similar setbacks are ordinary
  authored state changes unless the route explicitly maps them to an outcome.
- Death is not universally a terminal outcome. An authored route may define a
  terminal death/failure ending, but ordinary gameplay death is recoverable by
  default when a valid save exists.
- In single-player, ordinary recoverable death attempts the newest recovery
  snapshot in the active chronological manual/auto-save lineage. Loading an older
  snapshot makes that snapshot the head of a new active lineage: snapshots from
  the abandoned future are no longer eligible for automatic recovery, although a
  client may retain them for explicit manual branch selection. If the newest
  active snapshot fails load validation, recovery surfaces the error; a client may
  explicitly offer an older snapshot from the same active lineage, but RealmKit
  does not silently skip the corrupt entry.
- Recovery is scoped: a snapshot may rewind only state exclusively owned by that
  playthrough/session/instance. A persistent shared multiplayer world normally
  cannot be rewound because one player dies; multiplayer recovery uses authored
  player/session respawn rules instead. A private instanced session may rewind its
  own isolated state when appropriate.
- Recovery is an explicit restore of a saved deterministic state, not an implicit
  reversal of engine commands. Unsaved changes after that snapshot are discarded.
  If ordinary recoverable death occurs while an engine command is still executing
  against staged state, that enclosing transition is abandoned: none of its
  staged state changes or staged gameplay events commit before the recovery
  snapshot is restored.
- Auto-saves occur at meaningful stable boundaries rather than on every command or
  map step. Required/default triggers should include route start and major
  authored progression boundaries such as story-phase transitions; authors or
  capabilities may add stable checkpoints around major encounters, long travel,
  rest, or similar transitions where appropriate.
- Auto-save creation itself must not advance story phase, world time or encounter
  time, and must not change deterministic narrative selection.
- A valid route initial state must satisfy no route outcome condition. Validation
  and engine construction reject an initial state that already matches one or
  more outcomes, so initialization does not need to synthesize outcome events.
- Outcomes are selected from explicit authored conditions over deterministic
  playthrough/world state only while the playthrough has no recorded outcome. The
  engine evaluates those conditions after each successful state-changing
  transition, against the resulting staged state before commit. If exactly one
  outcome matches, recording the immutable outcome and emitting its event are
  part of the same atomic transition. Once an outcome has been recorded, later
  transitions do not evaluate or record additional route outcomes, including
  after a non-terminal completion outcome. An explicit choice that should cause
  an ending does so by changing typed state that makes the authored outcome
  condition true; there is no separate `ReachOutcome` effect. If multiple
  outcomes match while no outcome has yet been recorded, the transition fails
  with an explicit outcome-ambiguity error and none of its state changes are
  committed. The runtime never invents endings or resolves ambiguity by arbitrary
  declaration order.
- No universal cross-save profile/meta-progression is required. A client may later
  track discovered endings or achievements outside the route save, but the core
  engine does not depend on such metadata.

Conceptually:

```text
quests / relationships / flags / NPC state / side consequences
                            ↓
                     outcome conditions
                            ↓
                     OutcomeReached(id)

ordinary recoverable death
            ↓
   newest recovery snapshot
            ↓
       load validation
        ↙        ↘
    valid       invalid
      ↓            ↓
 restored      surface error;
 GameState     client may explicitly
               offer older snapshot
```

Validation should diagnose impossible required outcomes and overlapping mutually
exclusive outcome conditions where the available analyzers can prove them.
Runtime ambiguity checking remains the deterministic backstop for reachable
overlaps that static/bounded analysis does not prove.

**Delivered subset (Format 17).** `outcomes` with a `when` condition, checked
after every non-panel command against the staged state; one match is recorded
with `OutcomeReached` in the same transition. Rather than a runtime
ambiguity error, which a scheduled event or an unlucky order of play could
hit on every attempt and so lock the story, validation requires every pair
of outcomes to exclude each other provably (`ambiguous_outcomes`): one
requires a condition the other requires under `not`, or they require
different statuses of one quest. Validation also rejects an outcome that could hold at
the start (`outcome_at_start`) by requiring, on every branch, something no
start provides. Only
completed, non-terminal outcomes exist so far: the original game's MVP ends by
choosing a lead while the others stay open. `terminal` and failure outcomes
wait for content that needs them.

## 4. Canon fidelity and divergence

**Resolved.**

RealmKit source adaptations use one bounded-fidelity model rather than global
`strict / bounded / free` modes:

> RealmKit may expand canon, route around canon, and vary local consequences, but
> generated play must not contradict protected canon or transform the source into
> a fundamentally different story.

Worldgen represents source fidelity through three authoring concepts:

```text
Canon IR
├── Canon Anchors
├── Canon Constraints
└── Expansion Space
```

### Canon anchors

A canon anchor is a protected major source fact or event that generated routes
must preserve. Examples include critical identities, relationships, chronology,
major encounters and source-defining outcomes.

Canon anchors belong to Canon IR / worldgen authoring metadata, not necessarily
to the runtime playable format. Their concrete schema should be introduced only
when source-grounded generation needs it.

Every continuing supported branch must preserve reachability of the canon
anchors required for that branch/outcome in a state consistent with their
prerequisites. A branch that permanently invalidates one of its required anchors
is an authoring/validation error.

An explicitly authored terminal outcome may end the branch before later anchors
only when the source-adaptation policy explicitly permits that terminal divergence;
anchors after that endpoint are then not required for that branch. This exemption
does not permit contradicting protected facts/constraints already established, and
worldgen must not infer it merely because an outcome is terminal. Canonical
completion paths continue to preserve all anchors required by the adaptation.

### Canon constraints

Some fidelity requirements are broader than individual events. Worldgen must also
preserve relevant source constraints such as:

- established character identity
- established relationships
- chronology and ordering
- what a character can legitimately know at that point
- core characterization and motivations
- established world facts
- causal consistency

Worldgen's omniscient source knowledge must never silently become player or NPC
knowledge before the authored story provides a legitimate path to it.

### Expansion space

RealmKit's open-world freedom lives primarily between canon anchors. Valid
expansion includes exploration, side questlines, minor/new characters, inferred
events, alternate methods, relationship development and local consequences.

A branch may diverge substantially after one anchor provided it can still reach
the next required anchor in a canon-compatible state:

```text
Canon Anchor A
      │
      ├── exploration
      ├── side questline
      ├── alternate route
      └── local consequences
              │
              ▼
        Canon Anchor B
```

Side content may therefore change assistance, difficulty, dialogue, route,
relationships, NPC/local state and objective alternatives without deleting or
contradicting protected source facts.

### Provenance

Worldgen records generated material using authoring-side provenance:

```text
SOURCE    directly represented in the source
INFERRED  not explicit, but strongly supported by source facts
EXPANDED  newly authored gameplay material constrained by canon
```

Provenance retains source references or rationale in an authoring report/sidecar.
The runtime package does not need the source text or source-analysis machinery.

Decisions:

- Canon IR and fidelity/provenance analysis belong to worldgen/authoring, not the
  AI-free runtime engine.
- Protected anchors are explicit authoring constraints.
- Broader canon constraints cover identity, relationships, chronology, knowledge,
  characterization, established facts and causality.
- Generated branches may vary locally but must remain compatible with required
  downstream anchors.
- Contradicting or making a required anchor permanently unreachable is a
  validation error, not merely a style warning.
- `SOURCE / INFERRED / EXPANDED` provenance is retained outside core runtime
  state for review and regeneration.
- There is no unrestricted/free source-adaptation mode. More or less fidelity is
  expressed by the chosen protected anchors/constraints and the expansion space
  between them, not by a global fidelity slider.

## 5. Player entry and original-character generation

**Resolved.**

RealmKit separates world character identity from player control:

```text
Character   = who exists in the world
PlayerSpec  = which Character the human controls
PlayerRoute = how that player experiences the world
```

All playable characters are ordinary shared world characters. Player control is a
route-level binding rather than a separate character species.

Conceptually:

```text
World
├── characters
│   ├── canonical characters
│   └── authored original characters
│
└── PlayerRoutes
    ├── canonical route
    │   └── PlayerSpec.character = canonical_character_id
    └── original route
        └── PlayerSpec.character = original_character_id
```

Decisions:

- Future shared character identity belongs to a `CharacterSpec`-like world
  entity. Names, descriptions, background/origin, canonical identity and other
  intrinsic authored facts belong there.
- `PlayerSpec` identifies which character is controlled by the human. It should
  stay small rather than absorbing route state or capability data.
- In a canonical route, `PlayerSpec` references the appropriate canonical
  character.
- In an original route, worldgen creates an original world character and
  `PlayerSpec` references it. Canonical protagonists continue to exist as normal
  world characters/NPCs.
- Original-character creation is constraint/override driven rather than a
  universal fixed form. The user may provide as little or as much identity,
  background, ability or insertion context as desired; worldgen may propose
  unspecified details.
- The final original-character identity is resolved before generating dependent
  route prose, relationships, quests and dialogue.
- Starting location and story phase belong to the player route. Initial
  relationships, faction membership and similar mutable facts belong to route
  initial state or the relevant capability state. Combat stats, inventory,
  evidence and learned skills remain capability-owned.
- Runtime identity customization is not universal. A world may explicitly support
  safe typed cosmetic/customization fields, but canonical identity/background and
  other authored facts do not become freely mutable by default.
- Worldgen maintains an authoring-side knowledge boundary for the player
  character. Omniscient Canon IR knowledge is not player knowledge.
- Runtime tracks only knowledge distinctions that gameplay actually needs, using
  typed state such as evidence, discovered locations/facts, flags or a future
  dedicated knowledge component when justified.
- Original-character insertion and starting relationships/history must satisfy
  canon constraints. Worldgen may create plausible local connections, but must
  not invent major retroactive relationships/events that rewrite established
  canon.
- `PlayerState` remains mutable state intrinsic to the controlled character;
  run-wide story/world state belongs to `GameState` or typed state beneath it.

Shared `Character` definitions are already delivered. Format 16's remaining
limitation is the single fixed `world.player` binding for the implicit `default`
route. When a concrete multi-route package arrives, add a small route-level
`PlayerSpec` binding to the same shared Character rather than duplicating a
canonical person as separate player and NPC definitions.

## 6. Conditions and effects

**Resolved.**

RealmKit uses a small typed condition/effect model rather than an expression
language or scripting system.

Conditions are pure queries over current deterministic state:

```text
Condition
├── All(Vec<Condition>)
├── Any(Vec<Condition>)
├── Not(Box<Condition>)
└── typed leaf predicates
```

Leaf predicates are added only when a real core/capability need exists, for
example flags, quest/objective state, story phase, item possession, evidence,
relationships, faction state, entity state or optional world-time windows.

Effects are closed typed state changes:

```text
effects: Vec<Effect>
```

Examples include setting a flag, transferring an item, obtaining evidence,
advancing an objective, changing a relationship, moving a character or changing
story phase. Add variants with the capability that actually needs them. Route
outcomes are not effects: the engine derives them by evaluating authored outcome
conditions after state transitions.

Decisions:

- Condition evaluation is pure and side-effect free. Clients/engine code may
  evaluate a condition repeatedly without consuming resources, advancing time,
  drawing randomness or mutating state.
- Boolean composition supports `All / Any / Not` so authors do not need synthetic
  flags merely to express ordinary branching logic.
- Leaf predicates remain typed. Do not add arbitrary property paths, reflection,
  formula strings, embedded scripts or a generic expression evaluator.
- Do not introduce a generic numeric-comparison abstraction until repeated
  capability implementations prove it useful. A predicate should initially expose
  domain-appropriate semantics such as `RelationshipAtLeast` or
  `HasItem { quantity }`. Living sandbox worlds bring the first repeated numeric
  predicates (currency, standing, holdings, world time); they still stay
  domain-typed, as [Section 18](#18-living-sandbox-worlds) proposes.
- Effects remain closed typed variants. No generic `set(path, value)`,
  `eval(...)` or executable scripts.
- Effects in one authored transition execute in authored order against staged
  state.
- The enclosing engine command/state transition remains atomic: if any required
  effect fails, none of the staged effects are committed.
- Each effect defines deterministic behavior when applied to its current state.
  Do not add a generic repeat-policy abstraction prematurely. For example, setting
  an already-set flag may be a no-op, granting an item may accumulate, and quest
  completion must not duplicate rewards.
- Visibility is distinct from ordinary availability conditions. The engine
  evaluates authored visibility gating and does not expose hidden actions to the
  client. For actions that are already visible but unavailable, ordinary
  conditions answer whether they are legal/available and presentation decides
  whether/how to render them disabled, including any authored explanation.
- Do not build generic conflicting-write analysis now. Authored order defines
  deterministic behavior; higher-level validation/simulation can diagnose bad
  content when concrete cases justify it.

Format 12 (M5a) delivers the condition tree and effect lists: every
`requires` and `known_when` is one optional condition composed with `all`,
`any` and `not`, and a dialogue choice carries an ordered `effects` list that
commits or fails as a whole. Format 11's implicitly conjunctive lists are
rejected, not migrated.

## 7. Randomness and reproducibility

**Resolved.**

RealmKit supports genuinely stochastic worlds while preserving deterministic
replay. Randomness is an optional engine-level facility available to both
world-level systems and gameplay capabilities; it is not itself a gameplay
capability.

Typical uses include:

- low-probability world/ambient events
- random encounters or discoveries
- combat critical hits
- randomized loot
- skill/check outcomes
- weighted selection among authored event variants

Runtime randomness only selects among finite pre-authored possibilities. It never
generates new prose, entities, quests or rules.

Conceptually:

```text
World / capability
        ↓
 explicit random opportunity
        ↓
 deterministic seeded RNG
        ↓
 one authored result
```

Decisions:

- A world may declare/use stochastic mechanics even when no other optional
  capability requires them.
- A new playthrough initializes explicit RNG state from a seed supplied by the
  caller/new-game layer. The gameplay engine must not silently obtain entropy from
  wall-clock time, OS randomness or other hidden external state while executing
  commands.
- Current RNG state is part of deterministic `GameState`/save state whenever
  randomness is in use. Loading or death-recovering a save restores the exact RNG
  state as well as the rest of the playthrough.
- Deterministic replay means that the same validated package, same initial state
  including RNG state/seed, and same gameplay decisions produce the same events
  and resulting state.
- Conditions and presentation-only actions never consume random draws.
- Rejected/rolled-back atomic commands do not consume committed RNG state.
- Random opportunities occur only at explicit gameplay transitions such as
  entering/travelling through a location, resting, resolving an attack, opening a
  loot result or executing a skill/check. Merely opening menus or inspecting
  state cannot trigger a rare event.
- Probabilities/weights are authored content. The exact serialization
  representation is deferred until the first stochastic content format needs it.
- Combat may use randomness for mechanics such as critical hits even when basic
  hit/damage rules remain deterministic.
- Semantically unrelated stochastic systems should not accidentally perturb one
  another merely because an unrelated random draw happened first. When
  implementation needs multiple stochastic domains, use deterministic independent
  streams/sub-seeds or another equally explicit mechanism rather than one fragile
  incidental global draw sequence.
- The exact PRNG algorithm and stream/sub-seed mechanism are deferred until the
  first implementation, but once shipped they must be explicit/versioned for
  replay and save compatibility rather than relying on a library's unspecified
  default behavior.
- Every reachable stochastic result must lead to a valid authored continuation,
  recovery path or explicit outcome. Randomness may alter progression, but one
  unlucky draw must not leave the playthrough in an invalid dead end with no
  authored resolution.

A fully deterministic world that uses no stochastic mechanics remains valid and
needs no active RNG state.

## 8. Checks and proficiencies

**Resolved.**

RealmKit has no universal RPG skill/proficiency table. Capabilities define the
proficiencies and check semantics they actually need.

A learned ability/technique and a proficiency are separate concepts. For example,
a combat skill may be an authored action the character knows, while smithing or
stealth proficiency may influence whether/how a capability resolves an attempt.

A condition and a check also have different responsibilities:

```text
Condition
"may this be attempted?"
        │
        ├── false → reject command
        │
        └── true
             ↓
 capability-specific check/resolution
             ↓
       ┌─────┴─────┐
    success      failure
       │             │
 authored effects  authored effects
```

Decisions:

- There is no universal `skills: Map<SkillId, Value>` model and no mandatory set
  of persuasion/stealth/survival/etc. stats shared by every world.
- A capability owns only the proficiencies it needs. Investigation, for example,
  may rely on obtained evidence and player reasoning without any numeric
  investigation proficiency.
- Checks are gameplay resolution, while conditions are pure legality/availability
  queries.
- A legal check may be deterministic, stochastic through RealmKit's saved RNG,
  resource-based, or another capability-specific rule. Do not introduce a generic
  check/formula DSL.
- A rejected command is different from a failed check. Rejection mutates nothing,
  advances no modeled time and consumes no committed randomness.
- Once a legal attempt begins, failure is a valid gameplay result. It may consume
  resources/time/random draws and apply authored consequences.
- Retry rules and failure consequences belong to the capability/content rather
  than a global RealmKit rule.
- Proficiency scales and difficulty representation remain capability-specific.
  Do not force every system into a common numeric range or difficulty enum until
  repeated implementations prove such an abstraction useful.
- Every reachable check result used by required progression must lead to a valid
  authored continuation, recovery path or explicit outcome. A single failed roll
  must not silently make the route impossible.
- Whether the player may know exact odds, qualitative difficulty, or no preview
  is a world/engine disclosure policy, not a client presentation choice. The
  engine exposes only the permitted information; clients decide how to render
  that representation. Do not add a universal disclosure-level abstraction until
  repeated capability implementations prove one useful.

## 9. Time models

**Resolved.**

RealmKit keeps four time concepts distinct:

```text
real time           human thinking/input time; never advances game rules
story phase         discrete narrative/canonical progression; universal core
world time          optional in-world clock/calendar simulation
encounter timeline  local deterministic action scheduling
```

### Story phase

Story phase is universal and event-driven. It represents bounded periods of
narrative/canonical progression and advances only through explicit authored story
transitions, normally main-quest milestones.

Exploration, side content, ordinary dialogue, menu use and real-world thinking do
not implicitly advance canonical chronology.

**Delivered (Format 17).** A world lists its `phases` in order; the first is
current at the start and only the `enter_phase` effect moves the story on,
never back, passing any phases between. A `phase` condition holds from that
phase onwards, which gates side quests in waves. Saves keep the phases
reached, the current one last.

### World time

World time is optional. When enabled, use a monotonic integer count of minutes
from an authored epoch:

```text
WorldTime = u64 minutes since authored epoch
```

The epoch may be route-relative ("minute 0 = route start") or mapped by the
world's presentation layer to a setting-specific calendar/date. The engine stores
the scalar; presentation may render it as clock time, day number, traditional
period names or other authored text.

World time advances only through explicit authored/capability actions such as
travel, waiting, rest, appointments or other mechanics that genuinely consume
time. There are no universal command durations. Presentation-only actions consume
zero world time.

Within one forward committed history, normal world time is monotonic. Restoring
an older snapshot restores its saved WorldTime exactly, so time values that existed
only in the discarded future may be traversed again on the new branch. Authored
time travel/loops are separate explicit mechanics rather than weakening ordinary
forward-time semantics.

### Scheduled world-time events

A scheduled event timestamp must be strictly greater than the current world time
when that schedule becomes active. Initial schedules at or before the route's
initial world time are invalid, and a runtime effect may not schedule an event for
the current or a past minute. This avoids a separate initialization/current-time
dispatch path and prevents same-timestamp self-scheduling loops.

Advancing world time from `old_time` to `new_time` uses a deterministic
dispatch cursor. Start with current world time at `old_time`; repeatedly take the
earliest scheduled event with timestamp `<= new_time`, set current world time to
that event's timestamp, and resolve its effects against the staged state. Each
scheduled-event resolution is an outcome-evaluation/death-check point. If it
records a terminal outcome, stop dispatch immediately and commit the enclosing
transition with WorldTime left at that event timestamp; later due events and the
remaining requested advance do not occur. If it causes ordinary recoverable death,
stop dispatch, abandon the entire still-staged enclosing time-advance transition,
discard its staged effects/events, and restore the active recovery snapshot under
the Section 3 recovery rules. A non-terminal outcome may allow dispatch/play to
continue, but no further route outcomes are evaluated for that playthrough.

An event may schedule a later event using the cursor time as "current"; if the new
timestamp is still `<= new_time`, it is processed during the same advancement.
Because scheduling for the current or a past minute is invalid, an event cannot
recursively schedule another event at its own timestamp.

If no terminal outcome stops the advance, after no due events remain set current
world time to `new_time`. Events with the same timestamp that were already
scheduled resolve in stable authored declaration order. Do not add a generic
priority framework unless a concrete world requires one.

The engine owns current world time, explicit advancement and deterministic
ordering of due authored events. Capabilities own the consequences of elapsed time,
such as hunger, schedules, recovery, crafting completion or deadline behavior.

Deadlines are mechanically exact when world time is used, while presentation may
describe them exactly or narratively ("before sunset", "within three days", etc.).

### Encounter timeline

Encounter timeline remains a separate local scheduler. Its units have no implicit
mapping to seconds/minutes or world time. If an encounter should consume world
time, author that consequence explicitly, for example by applying a world-time
advance after resolution.

### Cross-layer rules

- Real-world thinking/input time advances none of StoryPhase, WorldTime or
  EncounterTimeline.
- WorldTime never implicitly advances StoryPhase.
- A StoryPhase transition may explicitly advance WorldTime when representing an
  authored canonical time skip.
- EncounterTimeline never implicitly advances WorldTime.
- Save/load preserves all enabled time state exactly.

## 10. Definitions, instances and identity

**Resolved.**

RealmKit distinguishes authored definitions from mutable runtime identity without
forcing every entity into an instance model.

Use three representations:

```text
1. authored singleton
   stable DefinitionId + mutable state keyed by that ID

2. fungible quantity
   DefinitionId + count

3. runtime instance
   InstanceId + DefinitionId + independent mutable state
```

The deciding rule is:

> Create a runtime instance when a concrete object needs persistent mutable
> identity distinct from its authored definition—whether the definition permits
> one live object or many distinguishable copies.

Examples:

- Canonical characters, locations, quests and evidence normally use their stable
  authored IDs directly; do not manufacture redundant runtime instance IDs for
  inherently unique authored identities such as Yang Guo.
- Fungible resources such as coins, arrows or identical herbs use a definition
  plus quantity when individual copies have no meaningful state.
- Equipment, spawned enemies, containers or other mutable physical objects use
  runtime instances when the concrete object needs persistent identity/state.
  This includes a unique legendary item when ownership, improvement, enchantment
  or other mutable state belongs to that physical object.

### Instance identity

When instances are needed, use deterministic monotonically allocated IDs
within an explicit authoritative identity domain. In single-player, that domain is
the playthrough. In multiplayer, it is the smallest authoritative state domain
within which instances may be transferred or jointly referenced; IDs from separate
domains must never become ambiguous when state is combined. Do not add UUIDs or
scope-qualified IDs unless a real server design requires them.

Save and restore both existing IDs and the next allocation counter only when
the authoritative identity domain is fully contained within the same recovery
scope. Within one forward committed history, an ID is never assigned to a second
concrete instance. Restoring an older snapshot may restore that allocator and
reuse IDs that existed only in the discarded future.

If instances from an identity domain can survive outside a rewindable scope, that
scope must not independently rewind the shared allocator. The allocator must be
owned by non-rewound authority, or transfer into another identity domain must
establish an unambiguous destination identity. Choose the concrete mechanism with
the first multiplayer implementation; do not add UUIDs, tombstones or scoped-ID
frameworks preemptively.

Every instance retains an immutable authored `definition_id`:

```text
Instance
├── instance_id
├── definition_id
└── mutable instance state
```

Runtime mutation changes state, not authored definition identity. Improvement,
damage, enchantment, ownership, location or disguise keep the same underlying
definition/identity. A genuine replacement/transformation that creates a different
thing consumes/removes the old entity and creates a new one.

### Multiplicity and unique items

Definition multiplicity is authored and orthogonal to whether an instance exists.

- A repeatable definition may have multiple live instances.
- A unique definition may have at most one live instance.
- A unique object may still require a runtime instance when it carries mutable
  physical state.

For example, a unique legendary weapon can have one instance whose ownership,
equipment state, damage, improvement or enchantment changes over time. Moving it
between characters does not change its `InstanceId`.

### Removal and durable consequences

Removed/destroyed/consumed instances do not require universal permanent
tombstones. Within one forward committed history, an InstanceId is not assigned
again after removal. Restoring an older snapshot may reuse IDs that existed only
in the discarded future, as described above. Gameplay facts that must survive
removal belong to the relevant durable state, such as quest/objective state,
evidence, flags or another capability-owned record.

Do not build a universal runtime provenance/history graph. An instance's
`definition_id` supplies its basic authored origin; capabilities may retain
additional provenance such as crafting/encounter origin only when gameplay needs
it.

Investigation evidence remains a separate authored identity/state. It may
reference a physical definition or instance, but possession of that object does
not automatically imply that its evidentiary significance has been discovered.
As delivered (Format 17), an evidence definition may name an `item`; the
`discover_evidence` effect is the only way evidence becomes known. Since Format
18 it may also name its `source`, list its `facts` and author `interpretations`:
the facts never change, and the current reading (derived, never saved) is the
last whose `when` holds, a condition that once true stays true.

## 11. Save compatibility and package evolution

**Resolved.**

A save is a storage-neutral snapshot of deterministic mutable playthrough state,
not a filesystem concept. The engine defines/validates the snapshot representation;
the client or authoritative server decides where it is persisted.

Conceptually:

```text
realmkit-engine
      │
      │ SaveSnapshot
      ▼
persistence adapter
      ├── CLI / desktop → local file or SQLite
      ├── mobile        → app-local storage
      └── server        → database / durable service storage
```

A save records at least:

```text
save_format_version
package_id
package_revision
player_route_id
deterministic mutable state
```

Do not bind saves to a particular RealmKit binary version. Compatibility is
defined by save format plus package identity/revision.

### Initial compatibility policy

For the first save/load implementation, require an exact package revision match:

```text
save.package_id       == loaded package.id
save.package_revision == loaded package.revision
save.player_route_id  exists
```

A revision mismatch is rejected rather than guessed compatible. Package revisions
may add, remove or change authored content freely; old saves remain valid against
the exact revision they were created with unless an explicit future migration is
provided.

The exact representation of `package_revision` (content digest, export revision,
author version, etc.) is an implementation choice. Semantically, gameplay content
that RealmKit treats as a different revision must have a different revision ID.

### Save contents

Saves contain mutable state and stable references to authored definitions; they do
not duplicate the complete world package.

Examples of saved state include player/capability state, quest/objective state,
flags, runtime instances and their IDs, RNG state, World Time, encounter schedules
and deterministic counters.

### Validation and corruption

Loading is all-or-nothing:

```text
decode
  ↓
save-format validation
  ↓
package/revision/route match
  ↓
reference + state invariant validation
  ↓
usable GameState
```

Do not silently reset missing/invalid fields or references. Unknown IDs, invalid
instance counters, incompatible RNG/time state and other corruption produce clear
load errors unless an explicit migration defines the transformation.

Manual saves, auto-saves and recovery checkpoints use the same snapshot format;
their role is persistence metadata/policy.

Local persistence should write a complete replacement atomically (for example,
write temporary state then replace the current snapshot) and retain the previous
successful snapshot as recovery. If the latest snapshot is invalid, clients may
offer the previous one explicitly; do not silently substitute it.

### Migrations

Do not build a migration framework before a real released compatibility need
exists.

If/when needed, keep two concerns separate:

- RealmKit save-schema migration: old `SaveSnapshot` format → new format.
- Package/content migration: state bound to package revision A → revision B.

Generic save-schema migrations may belong to RealmKit. Content-aware package
migrations belong with package/release authoring tooling rather than generic engine
guesswork.

### Multiplayer persistence

In authoritative multiplayer, the server owns the canonical state and durable
persistence. Clients submit commands; they do not authoritatively submit outcomes,
RNG results or mutated state.

Server persistence may be continuous transactional state rather than a literal
save file. Snapshot representations remain useful for restart, backup, testing,
migration and isolated checkpoints.

Shared multiplayer state may eventually be partitioned into world-, player- and
session/instance-scoped state, but those concrete structures are deferred until
multiplayer implementation.

A recovery snapshot may rewind only a state scope it exclusively owns:

```text
single-player playthrough → may rewind whole playthrough
private party instance    → may rewind that isolated instance
persistent shared world   → normally cannot rewind for one player's death
```

This preserves the single-player Skyrim-like checkpoint model without making it a
constraint on future online worlds.

## 12. Validation, reachability and simulation

**Resolved.**

RealmKit separates three quality mechanisms rather than pretending one mechanism
can prove a whole branching RPG correct:

```text
Validation
    ↓
Reachability analysis
    ↓
Simulation
```

### Validation

Validation checks hard structural and semantic invariants required for safe engine
execution: schema/version rules, duplicate or missing IDs, invalid references,
malformed conditions/effects, invalid probabilities, impossible unique-instance
initialization, broken dialogue links, missing route structure and
capability-specific invariants.

A validation error prevents export/load. Validation is not a claim that every
authored path is enjoyable or solvable.

### Reachability analysis

Reachability asks whether authored targets can theoretically become available from
the relevant initial state, using bounded/domain-specific analyzers rather than a
universal state-space model checker.

Required progression is held to a stronger standard than optional content:

```text
required target + proven unreachable → error
optional/secret + apparently unreachable → warning
```

Required targets include the route's main progression and required canon anchors;
capabilities may define additional required targets.

Reachability is evaluated per target/history. Mutually exclusive outcomes do not
need to be reachable in one playthrough; each required outcome/branch only needs
a valid history when the package declares it required.

### Simulation

Simulation executes the real RealmKit engine rather than a second simplified rule
system:

```text
WorldSpec
+ initial GameState
+ player policy
+ RNG seed/state when stochastic
        ↓
engine Commands
        ↓
trace / outcome / failure
```

Every simulation records its route, starting state, player policy, seed/state and
assumptions.

A successful simulation demonstrates that particular execution path. A failed
simulation demonstrates only that the stated policy/seed/path failed; it is not
proof that the route is impossible. Likewise, one successful run does not prove
all branches.

For stochastic authored branches, validation checks that each explicit reachable
result has a valid continuation/recovery/outcome where analyzable. RealmKit does
not attempt to enumerate the complete PRNG state space.

### Balance and completion

Balance is separate from validity. Combat length, economy pacing, survival
pressure and similar targets belong to world/capability-specific tests and
authoring guidance rather than universal core validity types.

Worldgen may call a package complete only when:

1. hard structural/capability validation has no errors;
2. every PlayerRoute has a valid initial state;
3. every required main-progression target is reachable as far as the available
   analyzers can determine;
4. required canon anchors remain reachable;
5. every packaged PlayerRoute has at least one successful real-engine simulation
   to a completion outcome;
6. every reachable explicit stochastic result has a valid authored
   continuation, recovery path or outcome; and
7. capability-specific completion checks pass.

The generation report states exactly what was validated/simulated and what remains
unproven. "Complete" means passed RealmKit's available checks and representative
simulations, not mathematically proven bug-free.

## 13. Capability selection and provenance

**Resolved.**

Capability selection is an authoring/worldgen decision made after canon
extraction and before detailed gameplay generation.

A concept merely existing in the fiction does not justify a gameplay capability.
Enable a capability when the intended game repeatedly needs all of:

1. mutable state;
2. reusable rules; and
3. meaningful player choices/consequences enforced by the engine.

For example, a blacksmith existing in a village does not imply a crafting system.
Repeated player-facing forging, materials, proficiency and consequences may.

Worldgen should produce a reviewable capability proposal containing the smallest
supported capability set, short rationale and relevant source references. It may
also explain meaningful omissions where an RPG convention could otherwise be
mistaken for a source requirement.

Do not use numeric confidence scores for capability selection. Existing authoring
provenance is sufficient:

```text
SOURCE
INFERRED
EXPANDED
```

The author/user may explicitly include or exclude proposed capabilities. Human
approval is supported but is not a runtime or automated-generation requirement.
Overrides are recorded in the authoring report rather than being misrepresented as
source evidence.

The generation order is:

```text
Source
  ↓
Canon IR
  ↓
Capability proposal
  ↓
author/user constraints or overrides
  ↓
Final capability set
  ↓
Gameplay / quest design
  ↓
Detailed content generation
```

The playable runtime package contains the actual definitions/state required by the
final capability set, not generation reasoning or source excerpts. An optional
authoring report/provenance sidecar may retain:

- selected and meaningfully omitted capabilities;
- `SOURCE / INFERRED / EXPANDED` classification;
- source references and rationale; and
- explicit author/user overrides.

Cross-capability interaction remains explicit and typed through conditions,
effects and references. Do not infer behavior from generic tags or hidden naming
conventions. Validation rejects references that require an absent/incompatible
capability.

Capability implementations are primarily package/world-level definitions and
rules. A capability may exist in the package because one player route uses it
while another route never exposes it. Do not duplicate an entire capability
system per route merely because route content differs.

No generic capability dependency graph is required until real implementations
demonstrate a need for one.

## 14. Presentation and information disclosure

**Resolved foundation.**

M1 provides context-sensitive menus. Longer-term presentation follows one strong
boundary:

> The engine determines what the player is allowed to know and do; clients decide
> how to render that information.

### Visibility and availability

Do not conflate a secret action with an unavailable visible action.

Conceptually:

```text
visible?
   ├── no  → client does not receive/offer it
   └── yes
        ↓
available?
   ├── yes → actionable
   └── no  → may be shown disabled with authored explanation
```

Future authored gating should therefore distinguish visibility from ordinary
requirements (for example `visible_if` versus `requires`). Hidden branches do
not become spoilers merely because their requirements fail.

Visible-but-unavailable actions may carry authored player-facing explanatory text.
Clients should not reverse-engineer detailed reasons from hidden/internal state.

### Spatial presentation

A fixed 3×3 map is **not** a RealmKit architectural requirement.

The universal gameplay truth is the explicit location/traversal graph. Worlds may
optionally group locations into map areas and give locations area-local spatial
positions:

```text
World
├── traversal graph                 universal
└── Areas                           optional grouping/layout
    └── Locations
        └── optional (x, y)
```

An `Area` is primarily spatial/presentation organization, such as a city, a
building floor or a mountain region. Spatial coordinates do not create exits and
adjacent cells do not imply movement. Explicit exits remain authoritative.

For a first grid layout, one area coordinate maps to at most one location.
Enterable structures may link from a city-map location to another area representing
their interior. Vertical travel remains explicit through exits and does not require
a universal z-axis.

Clients choose the rendering strategy:

- compact area → show the full map;
- large area → show a viewport/minimap of any appropriate size;
- graph-only area → present exits/actions without a grid.

Viewport dimensions (3×3, 5×5, 7×7, etc.) are client choices, not world semantics.

Opening a map is never required for ordinary movement. The player can always move
through currently legal explicit exits. A future map may offer path planning or
"travel to known location", but such behavior is a separate gameplay operation and
must not silently teleport across encounters, time advancement, locked routes or
other authored consequences.

Map rendering must respect player knowledge. Package/world omniscience does not
automatically reveal every location. A world that needs discovery/fog-of-war may
track the smallest typed discovery state required; worlds where geography is common
knowledge need no universal knowledge database.

For overland worlds, the [overland map](sandbox-worlds.md#overland-map) proposal
applies this: authored display positions, an engine map-view query, and a
client-owned viewport with zoom and panning.

**Original-game priority · delivered (Format 16).** Locations author optional
positions (`x` east, `y` south, within `MAP_BOUND`) and a closed kind (town,
castle, village, waypoint), all or none and never two on one spot.
Coordinates carry no movement semantics. The engine's map query returns the
places the player knows of: since Format 17 a place may author a `known_when`
condition, usually on evidence or a flag, and until it holds the place, its
roads and its exits are off the map and no road or exit there is offered or
taken.
The player's own place is always known. This is the smallest knowledge the
MVP slice needs; it derives from existing state and saves nothing. The query
leaves out characters who move,
since a map would show where they are rather than where they were last seen;
road open/closed state is likewise left to the action list. Areas and a
generic scene graph remain undefined.

### Client integration surface

**Decided for original-game P0 (Format 16).** Packages load from memory through
`WorldSpec::from_files`; `Command`, `Event`, `Action`, the map view and
(Format 17) the journal view (phase, known quests, evidence, outcome) are
serializable; `SaveSnapshot` reads and writes JSON bytes, checking the save
format first; and `realmkit-engine` builds for `wasm32-unknown-unknown`
without an adapter crate. The rest of this section stays the rule for what
comes next.

The original game's graphical client is a concrete consumer of RealmKit, but its framework
must not leak into the engine. The next presentation slice should define/prove the
smallest host boundary needed to:

- load a validated package from in-memory bytes/data rather than requiring a
  filesystem path;
- obtain player-permitted location/map/character/quest/actions views;
- submit the same typed commands used by the CLI;
- import/export storage-neutral save snapshots;
- keep presentation queries pure and deterministic.

The core should have a tested browser/WASM-compatible hosting path, whether that
is direct compilation of the relevant crates or a deliberately thin adapter.
React, Tauri, DOM state and platform storage remain outside RealmKit. Do not
stabilize a broad public SDK before the real client shows which queries it needs.

### Preview/detail policy

RealmKit does not globally require exact or qualitative previews. The
world/engine determines what preview information the player is permitted to know:
exact values, an authored qualitative representation, or none. Clients receive
only that permitted representation and decide how to render it. Do not add a
universal disclosure-level abstraction until multiple capabilities require common
semantics.

Long action-list grouping remains a client concern. The engine provides visible
actions in stable deterministic order; richer clients may group them for display.

### Localization

Use a hybrid ownership model:

- world/package content owns authored/world-specific player-facing prose;
- RealmKit clients own standardized generic UI locale keys;
- packages may later provide explicit setting-specific overrides.

Missing fixed-interface translations for the package language must not silently
fall back to English in normal source-backed play. Exact locale-file serialization
is deferred until localization implementation.

### Accessibility

Presentation must not make gameplay semantics depend on one visual/input mode:

- color is never the sole carrier of meaning;
- every action has a textual label;
- spatial maps have an equivalent textual/list representation;
- raw-key interaction is optional convenience;
- line-oriented play remains supported for accessibility and automation.

## 15. Equipment and crafting details

**Resolved foundation.**

Equipment and crafting are separate optional capabilities. Equipment describes
persistent individual objects; crafting describes explicit authored transformations
of resources and equipment. Neither implies a universal material, quality or
enchantment system.

Decisions:

- Equipment definitions contain concrete authored properties. Runtime does not
  derive item stats from a universal material hierarchy such as iron < steel <
  rare metal. Authoring helpers may generate families of definitions, but export
  concrete values.
- Distinguishable equipment uses runtime instances under the definition/instance
  rules from Section 10. Unique equipment may have at most one live instance.
- Equipment slots are explicit authored slot IDs. An item may occupy multiple
  slots (for example a two-handed weapon occupying main/off hand); validation
  rejects conflicting equipped instances.
- Effective ordinary stats are recalculated from base character/capability state
  plus typed equipped modifiers. Repeated equip/unequip never permanently
  accumulates bonuses.
- Improvement mutates the same instance through explicit authored forward states
  or tiers. Entering a new improvement state replaces the previous improvement
  contribution; it does not stack the whole prior bonus again.
- Improvement labels/counts are world-specific. RealmKit does not require a
  universal Ordinary/Fine/Superior/etc. ladder.
- Recipe knowledge and crafting proficiency are independent gates. Knowing a
  recipe does not imply sufficient proficiency, and proficiency does not
  automatically reveal recipes.
- Proficiency scales and thresholds remain capability/world-specific.
- Initial crafting is deterministic once all requirements are met. A later world
  may use the general check/RNG model only when stochastic crafting is a genuine
  gameplay requirement.
- Forge consumes authored inputs and creates a new instance.
- Improve consumes authored inputs and mutates the same instance's improvement
  state.
- Enchant consumes authored inputs and mutates the same instance by attaching one
  learned authored enchantment.
- Initial enchanting supports at most one enchantment per item. Disenchanting,
  replacement/removal, random affixes, charges/recharge, multi-effect enchanting
  and destructive learning are deferred until a representative world needs them.
- Unique/quest artifacts may explicitly be ineligible for forging, improvement,
  enchanting, destruction or other crafting operations.
- Ordinary equipment modifiers are typed scalar contributions owned by the
  relevant capability. For example armour may contribute defence and a speed
  penalty, while the combat capability owns speed interpretation and the speed
  cap.
- Exceptional mechanics such as immunity/vulnerability retain their explicitly
  authored semantics. Generic stacking rules for multiple exceptional sources are
  deferred until an implementation actually needs simultaneous sources.
- Crafting operations are atomic. Rejection consumes no materials/resources and
  grants no progress/reward; success consumes inputs once and creates/updates the
  intended instance once.
- Durability/repair, arbitrary affix rolling and crafting feedback loops remain
  absent until source/gameplay requirements justify them.

Actual material costs, improvement strengths, proficiency thresholds, armour
penalties, speed caps and similar numeric values are world/capability balance
parameters rather than unresolved architecture.

## 16. Multiplayer authority and pacing

**Resolved foundation; concrete networking/session policies are deferred until a
server milestone.**

Multiplayer keeps RealmKit's deterministic command/event model. It adds an
authoritative coordinator rather than turning the engine into a real-time
simulation.

```text
Client
   │ Command
   ▼
Authoritative server
   ├── validate against current state
   ├── resolve RNG
   ├── execute atomically
   ├── assign authoritative commit order
   ├── persist
   └── emit permitted events/views
```

The first concrete consumer is the original game's shared mode. Its product
assumptions are: solo play remains local/offline; choosing the shared mode crosses
an explicit authority boundary; local and shared histories fork and are not later
merged.

For the first handoff prototype, prefer a reproducible replay/import artifact over
trusting a mutable client save. It can bind the package revision and initial
seed/state to the ordered committed commands required to reproduce the accepted
Ascension state. The server replays/revalidates before taking authority. This
establishes mechanical reachability but is not proof of human play, so stronger
anti-cheat remains deferred unless competitive stakes justify it.

Still open for that milestone: the exact replay artifact and compaction strategy,
account/character binding, persistence/database shape, party/session ownership,
and what parts of an offline history (if any) a public server is willing to
accept. None of these are current package-format requirements.

Decisions:

- The server owns canonical mutable state, rule execution, RNG resolution and
  durable persistence. Clients submit commands rather than authoritative state or
  outcomes.
- Commands that touch the same mutable authoritative scope are serialized into a
  committed order and revalidated against the state that exists when committed.
  Independent scopes may progress concurrently; RealmKit does not require global
  lockstep.
- Deterministic replay is defined by initial state + authoritative committed
  command order + RNG state. Network arrival timing itself is not gameplay state.
- A pending human decision pauses only the encounter/session scope that requires
  it, not the whole server. Other independent players/encounters may continue.
- Equal encounter-timeline timestamps use the same stable deterministic
  tie-breaking semantics as single-player. RealmKit does not require universal
  simultaneous-secret action selection.
- Disconnect/AFK timeout behavior is server/session coordination policy. A timeout
  may submit a predefined ordinary fallback command (for example wait/defend), but
  wall-clock timeout duration does not advance StoryPhase, WorldTime or
  EncounterTimeline by itself and does not invoke runtime AI.
- Race conditions over shared/unique resources resolve naturally through
  authoritative ordering plus command revalidation: the first committed valid
  command changes state; later commands may become invalid.
- Genuine group voting, ready checks or simultaneous collective choices are
  explicit future party mechanics rather than default command semantics.
- Server information disclosure is per player. The server sends only state,
  actions and events that player is permitted to know; hidden server state need
  not be transmitted.
- Multiplayer-capable systems explicitly define ownership of mutable state when
  implemented (for example world-, player-, party/session- or encounter-scoped).
  Do not add a generic scope field to every current single-player datum in
  anticipation of networking.
- Reconnecting clients synchronize from current authoritative server state. Client
  snapshots never rewind persistent shared world state.
- Recovery remains scoped as defined in Sections 3 and 11: a private isolated
  instance may restore its own checkpoint; persistent shared-world state normally
  cannot rewind because one player dies.

Exact network protocol, authentication, database design, timeout values, fallback
commands, party ownership rules and horizontal scaling are implementation/server
policy to decide with the first real multiplayer consumer.

## 17. Combat encounters

**Proposed; confirm before M3c.**

The roadmap holds the details under
[Combatants and encounters](../ROADMAP.md#combatants-and-encounters--proposed)
and [Stat ranges and caps](../ROADMAP.md#stat-ranges-and-caps--proposed). In
summary:

- Combat is a component of the shared Character model, not a monster type.
  `Engage(character)` starts an encounter with any engageable character that has a
  combat profile. An authored group brings in its undefeated members present at
  the location.
- Encounter state holds any number of participants on any number of sides. M3
  content exercises one player against one or more opponents; allies (XvY) reuse
  the same state when a world needs companions or party play.
- Each participant is controlled by the player, which pauses the timeline, or by
  an authored policy enum. M3's only policy uses the strongest affordable skill,
  or the basic attack, on the first living opponent.
- Ties resolve by `(next_time, side order, participant order)`. MP is checked at
  validation and spent on resolution. A speed change applies from the actor's next
  action. Dead participants lose pending actions.
- While an encounter is active it solely owns every participant's HP and MP.
  Opponent vitals never persist outside an encounter.
- Grinding uses repeatable groups, which grant rewards on every victory and
  record no defeats, and `Flee`, which takes effect at the player's next turn.
  Copies within one encounter are told apart by participant position; Section 10
  instance IDs are only for copies with persistent state.
- Skills spend MP or rage. MP regenerates over encounter time and by resting,
  and MP costs are flat as authored; rage starts at zero in each encounter and builds
  from actions and damage taken. The
  [balance simulation](../ROADMAP.md#balance-simulation--proposed) tunes both
  against grinding-oriented targets.
- Most parts of combat are optional and exist only when content uses them:
  character levels and XP, MP, rage, skills, technique ranks, realms and seeded
  RNG. Power comes from stats, never from character level.
- Damage has two channels, physical and special; each world names special
  (magic, 内力, mana) with a required `special_name` in its combat block. The matching stats dominate and the other channel adds an
  authored share (25% proposed), so deep 内力 also blocks some physical damage.
  The proposed formula has no K: defence equal to the combined attack halves
  damage.
- An authored group may yield at a share of maximum HP instead of fighting to the
  death, for sparring, jousts and canonical duels; the player losing such a
  fight is not death recovery.
- In M4, learned techniques have ranks that rise through use, teaching and
  authored breakthrough gates; a world's realm (境界) is the rank of its core
  internal art. This replaces a separate realm-tier system.
- Combat stats have a 9,999 engine safety bound. Speed has a required authored cap
  (candidate 200 at baseline 100). Other stat caps wait for equipment stacking in
  M4. Action cost 100,000 gives every integer speed up to 255 a distinct delay.
- Still open: policies richer than basic attack, relationships between three or
  more sides, respawn delays in World Time (M5), downed/revive rules, and immunity,
  vulnerability and stacking (M4).

## 18. Living sandbox worlds

**Open; decide each item before the M5 or M6 slice that first needs it.**

The [sandbox design direction](sandbox-worlds.md) proposes the capabilities,
and the roadmap schedules them as
[M5 slices](../ROADMAP.md#delivery-slices--proposed-1) and
[M6](../ROADMAP.md#m6--living-sandbox-worlds--proposed). The original game's world is now
the concrete full-game pressure test: after the delivered economy and first
troop/battle slices, factions/standing and the remaining party needs come before
holdings, world agents and faction strategy. Optional detail stays deferred until
authored content needs it. Each question below has a proposed answer that still
needs to be confirmed:

- **Recurring schedules (M5b) · decided.** A schedule authors a first minute
  strictly after the route's initial world time and an optional period of at
  least one minute, checked by validation. The dispatch cursor expands it
  lazily, one due occurrence at a time. Nothing about a schedule is saved:
  its next occurrence follows from the saved minute. Ties at one minute go in
  schedule order (authored events, then movers; later capabilities append
  their own schedules after these). Effects cannot start or stop a schedule;
  an occurrence's authored condition decides whether it does anything.
- **Exits beyond compass directions (M5b) · decided.** Roads are an undirected
  list in `world.json`, each with an ASCII ID, two ends and optional travel
  minutes, condition and blocked text; at most one road joins a pair, so
  travel names its destination. A world, and even one location, may mix
  roads with compass exits.
- **Offers (M5c).** Proposed: the saved RNG draws an offer's parameters from
  authored candidate lists only at an explicit gameplay transition, such as the
  giver's recurring refresh or the player's arrival, and the drawn offer is
  saved. Menus and dialogue only read existing offers, so inspecting jobs
  consumes no randomness ([Section 7](#7-randomness-and-reproducibility)).
  Still open: how many offers a
  giver holds at once, and when a refused offer comes back.
- **Dialogue by role (M5e).** Proposed: roles are authored on characters and
  never change; a role's dialogues open for a member only when the member has
  no matching dialogue of its own, and its topics append after the member's own
  choices; a topic's `next` resolves in the role's own nodes. `Speaker` is a
  typed character reference usable wherever a
  character is, resolved at evaluation; there is no variable binding beyond
  it. Line slots are validated per member against what the role's dialogue can
  reach. Still open: whether roles may also be derived from state (the holder
  of a location, a faction's ruler) rather than listed, and whether a
  character may suppress one inherited topic.
- **Text variants (M5e).** Proposed: any player-facing text may be a list of
  `{ when, text }` variants ending in an unconditional one; the first that
  holds is shown. Evaluation is pure, so displaying text never draws or
  mutates. Still open: whether combat prose variants move to the same rule.
- **Several languages (M5e).** Proposed: a package keeps exactly one language.
  A source that carries several produces one package per language from the
  same authoring data, with identical IDs. Still open: a save is bound to the
  package revision, which includes text, so it cannot move between language
  packages; decide whether the revision should hash rules and text separately.
- **Start choices (M5f).** Proposed: a route may author start questions shown
  at New Game; each option applies an ordered effect list to the initial state,
  and the save keeps only the resulting state plus the chosen option IDs. This
  is authored initial state, not runtime identity editing, so it stays within
  the original-character rules above. **Decided and delivered (Format 16):**
  every question is asked, in order, and no option skips or adds one; options
  may set flags, grant items, currency and techniques (XP included) and raise
  proficiencies; their events are not shown, only the state they produce.
- **Characters who move (M5b) · decided.** A character may author a set of
  locations and a recurring schedule; each occurrence moves it to one of them,
  uniformly drawn from a `world` RNG domain, possibly where it already is or
  where the player stands, and its location is saved keyed by its ID. A mover
  starts at its one placement and has no combat profile. Occurrences will be
  skipped while it rides in a party or is captive, once those exist.
- **Proficiencies (M6).** Proposed: following [Section 8](#8-checks-and-proficiencies),
  each capability owns the proficiencies it needs (trading in the economy,
  leadership in the retinue, surgery in mass battle), with its own ranks and
  checks. A technique rank is not such a proficiency. Still open: M4d gates
  recipes on a Smithing *technique*, which blurs that separation. Either keep
  it as a deliberate exception or move crafting to a crafting-owned proficiency
  before sandbox proficiencies copy the pattern. Proposed growth: proficiency
  points per level from the level table, saved as an allocation per character
  and optionally capped by an authored stat; each proficiency is personal or
  party, and a party proficiency uses the best rank in the retinue; items can
  teach a rank through study over world time or grant a bonus while carried.
  As delivered in M6a-2 (Format 15): proficiencies are a closed engine
  set, so far `trading`, each defined by the block of the capability that
  uses it (`economy.trading`, with a name, a top rank and its narrowing);
  level entries grant shared `proficiency_points`; a save keeps each
  rank's trained and taught parts, and unspent points are derived; a
  `raise_proficiency` effect teaches ranks and a `proficiency` condition
  reads them. Still open: party proficiencies wait for companions, and
  study, carried bonuses and stat caps for a world that needs them.
- **Price model (M6a) · decided.** Prices follow production, not stock. Each
  market keeps a price index per good in thousandths of the base price, within
  authored bounds. A recurring price tick moves it against net supply
  (production minus consumption from authored producer counts, where
  producers' consumption of a good shrinks by 1,000 ÷ index while it is
  dear), reverts it
  towards base, pulls processed goods up towards dearer inputs and converges
  linked markets, in four phases with draws in authored market and goods order
  and convergence applied from one snapshot; caravans will converge their
  destination on arrival. The player's own purchases and sales move the index
  at once by an authored step per unit. Buying costs base × index × (100 +
  spread) and selling fetches base × index ÷ (100 + spread), with one final
  rounding, as the source does; a trading proficiency will narrow the spread.
  Prosperity drifts daily towards an ideal set by scarcity and buildings, and
  scales income, tariffs, stock and recruit pools
  ([Economy](sandbox-worlds.md#economy)). As delivered in M6a-1: the supply
  draws stay, from their own `market` stream; phase 1 clamps to the bounds so
  later phases work on valid indices; and warm-up rounds are an authoring
  choice, run by `scripts/combat_sim economy --prices`, never by the engine.
  M6a-2 (Format 15) added prosperity, which moves one point a day towards
  its ideal and scales a market kind's demand linearly between authored
  percentages at 0 and 100; merchants' stock and purse, redrawn on a
  restock schedule from their own `stock` stream and limiting trade both
  ways; villages that feed their market town on the tick; weekly workshop
  settlement at local prices with no spread; and the trading proficiency.
  Each part is its own optional block, which settles the
  "own block or optional fields" question below for the economy.
- **Standing tracks (M6b).** Proposed: each track is authored with bounds,
  named thresholds and a scope: global, per faction or per character. There is
  no fixed list of tracks such as renown or honour.
- **Mass-battle formula (M6d).** Decided as delivered in Format 14, without
  ground, leadership, individuals other than the player, or a champion duel:
  troops fight as stacks of counts,
  and the player, companions and leaders as stacks of one with their own HP,
  over at most an authored number of rounds
  ([Mass battle](sandbox-worlds.md#mass-battle)). Each round both sides deal
  count × the personal damage formula at power 100, limited to an authored
  melee frontage, split over the enemy's exposed stacks by count, computed per
  attacking and target stack, and scaled by an authored class matchup, the
  ground (an authored type with a percentage per class that locations and
  roads name, neutral when omitted), leadership, current morale and one
  bounded roll per side from a dedicated `battle` domain. Each target stack's
  damage divided by its HP gives its losses, with the remainder carried per
  stack; individuals lose HP instead and are knocked out, never killed, at
  zero. Morale falls with losses, carrying its remainder too, and a rout ends
  the battle with a pursuit round. Simultaneous routs are a draw; at the round
  cap the side with less strength withdraws, the attacker on a tie, and the
  other side wins without a pursuit. Strength, one number (count × HP ×
  attack ÷ 100, scaled by morale), serves the summary, renown, the round cap
  and AI decisions. Without morale, the battle ends only by exhaustion or the
  round cap. Everything is integer and both sides resolve at once. It follows
  the simulator-parity rule: `scripts/combat_sim` mirrors it before tests pin
  its numbers. Still open: how troop quality beyond combat stats counts;
  whether troop classes and the matchup table are authored per world
  (proposed) or a fixed engine set; defaults for frontage, the morale factor,
  the rout threshold and the hold percentage; and whether a champion duel can
  end a battle outright.
- **Battle modes (M6d).** Decided as delivered in Format 14 (a round
  summary shows strength, losses and morale per side): when the player's party joins a battle, the
  player chooses autoresolve or command. A commanded battle is a third stance
  beside exploring and fighting that holds every individual's HP and MP, and a
  champion duel's encounter while it runs, so each keeps exactly one home.
  Each round the player gives one order from a closed set (charge, the
  default; hold, which slows melee on both sides; flank with mounted troops;
  retreat), each a fixed change to the round rule with authored percentages,
  sees a round summary, and may autoresolve the rest at any round. Both modes
  run the same round rule, and a commanded battle that only charges matches
  autoresolve exactly. Agent battles always autoresolve. Still open: how much
  each round's summary shows (proposed: strength, losses and morale per side,
  not per stack).
- **Overland map (presentation).** Proposed: locations may author integer
  display positions and a kind from a closed list, all or none per world; the
  engine answers one map-view query that respects player knowledge; the
  terminal client owns the viewport, zoom, panning and label placement, and
  line mode prints a fixed 80 × 24 view
  ([Overland map](sandbox-worlds.md#overland-map)). Authors densify long roads
  with waypoint locations rather than any free movement. Still open: the
  closed list of kinds, the coordinate bound, whether the map is a panel in
  play or a screen of its own, and whether positions belong to areas
  ([Spatial presentation](#spatial-presentation)) or to the world as one area.
- **Agent policies (M6f).** Proposed: a closed enum (patrol, raid, escort,
  besiege, follow, trade, return home), chosen by authored priority rules over typed
  conditions. Parties act in stable instance order. "Nearest" and every route
  a party takes are shortest total travel time on the road graph, with ties
  broken by authored road order; there is no free-space pathfinding. Agents
  draw from their own RNG domain, separate from combat and offers.
- **Faction strategy (M6g).** Proposed: a faction tick before the agent tick,
  with its own `faction` RNG domain; a closed stance enum (defend, gather,
  campaign, raid, rest) chosen by authored priority rules; a marshal picked by
  an authored standing track with ties by relation, where a per-character
  relation track exists, and then authored order; campaign targets by travel
  time and garrison margin; fief grants to the player on request, otherwise to
  the member with the fewest holdings; defection below an authored relation
  threshold; truces after peace. Still open: whether several allied factions can
  campaign together, and whether the player's own kingdom uses the same stance
  rules for its vassals.
- **Interception (M6f).** Proposed: travel along one road is atomic, but a tick
  that falls during the journey can stop the player at the road's midpoint when
  a hostile party shares that road; a battle there uses the road's ground.
  Arriving at a location where a hostile party stands also intercepts. Still
  open: whether a party can pursue the
  player across more than one road.
- **Runtime-created entities.** Proposed: none beyond instances of authored
  definitions, and instances only for spawned copies and parties. Unique
  authored characters keep their authored IDs, and state such as wounds or
  captivity is keyed by them ([Section 10](#10-definitions-instances-and-identity)).
  A player-founded kingdom is an authored, dormant faction that an
  effect activates, so validation sees every faction that can exist.
- **Ambition ladders.** Proposed: a sandbox route's rungs below the top are
  ordinary main-questline quests; only the top rung is the route's completed,
  non-terminal outcome, because a playthrough records at most one outcome
  ([Section 3](#3-outcomes-failure-and-replay)).
- **Retinue XP (M6c).** Decided as delivered in Format 14: the roster saves
  an XP pool per line and level, and soldiers level up within their line by
  that pool, renamed where the world authors a new name, instead of paying
  for each upgrade step. Battles add to it and recruits add none. Every
  soldier leaving a squad (upgrade, casualty, desertion) takes its share, the
  pool divided by the head count rounded down; a squad whose share covers the
  next level rises whole, paying that XP and carrying the rest. Upgrades are
  branch choices from a line's last level, carrying each soldier's share. A
  squad with nobody left drops its pool, so XP is only moved or spent, never
  farmed. Soldiers are never instances: a full world may hold a million.
- **Roster limit (M6c).** Proposed: an integer the engine computes from an
  authored base plus authored contributions from standing tracks and the
  leadership proficiency. Companions count; prisoners have a separate limit. A
  lower limit refuses recruiting but never removes troops.
- **Companion progression and gear (M6c).** Proposed: equipment instances save
  one optional wearer, the retinue shares one stash, and every retinue
  character's effective stats come from one derivation function and are never
  saved. Still open: whether companions level from the world's level table and
  allocate points like the player, or keep fixed profile stats and grow only
  through techniques and gear.
- **Prisoners (M6d).** Proposed: captures are an authored share of the losing
  side's casualties, drawn from the battle's RNG domain and capped by the
  winner's prisoner limit; captivity of a unique character is state keyed by
  its ID. Player captivity is an ordinary setback, not an outcome. Escapes run on the world's
  prison schedule with their own `captivity` RNG domain. Decided: lords and
  companions are never killed in battle; each one on the losing side is
  captured with an authored chance from the battle's RNG domain and otherwise
  escapes, outside the winner's prisoner limit. Still open: what
  happens to the player's retinue and stash when the player is captured.
- **Battle rewards and injuries (M6d).** Proposed: a victory splits the
  defeated side's authored XP between the player, companions who fought and
  surviving troop pools by authored shares; changes an authored standing track
  by relative strength; and fills a loot pool from each defeated troop's loot
  table, drawn from the battle's RNG domain with more draws for better looting.
  Worn or rusted loot is its own authored definition, since improvement tiers
  only rise from the base. Character XP shares apply only in worlds with
  levels. Lasting injuries are authored stat penalties saved as IDs per
  character and subtracted during derivation, saturating at each stat's lower
  bound. Still open: whether loot left in the pool can
  go to companions' stash automatically.
- **Personalities (M6b).** Proposed: a world declares a closed list of named
  traits and each character authors a few; traits never change during play.
  Agent policy priorities and diplomacy weights may test a leader's traits.
  Reactions are authored rules of the form trait × deed → standing change.
  Deeds are either a closed set the engine detects (raiding a village,
  demanding its supplies or driving off its livestock, which are holdings
  actions; releasing or ransoming a prisoner; unpaid wages) or world-declared
  IDs that an authored `ReportDeed` effect reports, such as a tournament win or a broken promise.
  Companion friction takes effect in the transition that lowers the relation,
  with no tick. Still open: whether any world needs traits that change.
- **Wounded, provisions and morale (M6c, M6d).** Proposed: the roster saves a
  healthy and a wounded count per troop type; battle losses split into killed
  and wounded by an authored share that surgery raises; an authored share of the
  wounded recovers on each upkeep tick. Provisions are a value on trade goods,
  eaten per head, prisoners included. Morale is one bounded integer per retinue;
  low morale lowers battle strength and causes desertion. Wages, provisions,
  recovery, morale drift and desertion run in that fixed order on the retinue's
  own upkeep schedule, with desertion drawn from a `retinue` RNG domain, so
  these rules work without world agents and never shift agent draws. Agent
  parties keep the same two counts, and their wounded recover by an authored
  share on each world-agent tick, without randomness. Still open: whether food
  variety matters, and whether carried goods have a weight limit.
- **Travel speed (M6c).** Proposed: a road's authored duration scaled by the
  party's speed: the slowest of a world-authored base party speed and each
  healthy troop type's authored speed, with penalties for size, wounded and
  prisoners and a bonus from pathfinding, in integers with one rounding. A
  party with no healthy troops moves at the base speed. Roads with an authored
  duration take at least one minute; roads without one take no time. Agent
  parties use the same rule. Where equipment exists, a worn mount's authored
  travel speed replaces the base for its wearer, and the slowest counts.
- **Recruit pools (M6c).** Proposed: each recruiting location saves a count per
  troop definition that recruiting consumes and an authored schedule refills,
  scaled by prosperity and gated by standing where those exist.
- **Knowledge and news (M6f).** Proposed: a closed set of notable world events
  is reported after the command that crossed it, either at once or on arrival
  at a town as the world authors, and kept in a bounded saved journal. The
  player remembers each market's prices and each moving character's location
  as last seen, with the minute; authored effects reveal them. Clients show
  only this knowledge ([Section 14](#14-presentation-and-information-disclosure)).
  Still open: the journal's bound, and whether agents' own knowledge is ever
  limited, which the proposal does not do.
- **Villages (M6e).** Proposed: rent may accrue and wait for collection in
  person, with unrest drawn from a `villages` RNG domain; where the economy
  exists, demanding supplies and driving off livestock are engine-detected
  deeds; livestock is a trade good carried as a herd with a travel penalty;
  bandit trouble is a village state set on the holdings' own village schedule,
  from the same `villages` domain, and cleared through an offer.
- **Marriage (M6h).** Proposed: courtship is authored dialogue over relation
  tracks, and `Marry` records at most one spouse per character, keyed by ID. No
  children, heirs or dynasties. Still open: whether a spouse can act for the
  player, for example running a holding while the player travels.
- **Culture.** Proposed: no runtime concept. Troop lines, recruit pools and
  naming are referenced directly by locations and factions; authoring tools may
  group them as a culture before export. Still open: whether any world needs
  culture to change during play, which would make it holding state.
- **Capability boundaries.** Proposed: every sandbox capability and every
  part inside one is optional, per the
  [Modularity](sandbox-worlds.md#modularity) rules: presence decides, hard
  dependencies are few and validated with stable codes, cross-capability
  interactions occur only when both sides are present, and each absent part
  has a defined behaviour instead of placeholder data. Still open: whether a
  part's presence is marked by its own block or by optional fields inside the
  capability's block.
- **Bounds and save size.** Proposed: a new `AGENT_BOUND` caps live parties,
  and validation keeps party rosters within the existing numeric bounds, so a
  save stays small and ticks stay cheap. The concrete value waits for the
  reference fixture.

## Discussion order

Discuss decisions immediately before their first concrete consumer. With the original game
as the current full-game pressure test:

1. Presentation: overland-map data/disclosure and the minimal embeddable/WASM host
   boundary.
2. M5: start choices plus explicit story-phase/main-side
   questline/outcome progression; dialogue roles/text variants when the opening
   content needs them.
3. M6b: factions, war/peace and standing tracks.
4. Remaining M6c: companions, roster limit, provisions/morale and travel speed.
5. M6e-M6h: holdings, world agents, faction strategy and only the politics/order
   pieces used by the representative sandbox world.
6. M7/M8: authoring diagnostics, simulation, source adaptation and provenance as
   their consumers become active.
7. M9: replay/import, persistence, identity and session rules only when shared
   play becomes active server work.

Open decisions for unrelated future capabilities should stay open rather than
being settled speculatively.
