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
  Every player route has a PlayerSpec-like static player definition, an initial
  state/location, exactly one main questline, and one or more authored outcomes.
  Quests are RealmKit's primary story-progression abstraction. Side questlines are
  optional content layered over the same shared world.
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
- Every route has one player-controlled character definition (`PlayerSpec`),
  initial state/location, exactly one main questline, and one or more outcomes.
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
- In single-player, ordinary recoverable death resumes from the most recent valid
  recovery save snapshot. Manual saves and auto-saves participate in the same
  chronological recovery history; the newest valid snapshot wins.
- Recovery is scoped: a snapshot may rewind only state exclusively owned by that
  playthrough/session/instance. A persistent shared multiplayer world normally
  cannot be rewound because one player dies; multiplayer recovery uses authored
  player/session respawn rules instead. A private instanced session may rewind its
  own isolated state when appropriate.
- Recovery is an explicit restore of a saved deterministic state, not an implicit
  reversal of engine commands. Unsaved changes after that snapshot are discarded.
- Auto-saves occur at meaningful stable boundaries rather than on every command or
  map step. Required/default triggers should include route start and major
  authored progression boundaries such as story-phase transitions; authors or
  capabilities may add stable checkpoints around major encounters, long travel,
  rest, or similar transitions where appropriate.
- Auto-save creation itself must not advance story phase, world time or encounter
  time, and must not change deterministic narrative selection.
- Outcomes are selected from explicit authored conditions over deterministic
  playthrough/world state. The runtime does not invent endings.
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
 latest valid manual/auto-save snapshot
            ↓
       restored GameState
```

Validation should eventually reject impossible required outcomes and ambiguous
states in which mutually exclusive outcomes can become true simultaneously.

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

Every supported generated branch must preserve reachability of required
downstream anchors in a state consistent with their prerequisites. A branch that
permanently invalidates a required anchor is an authoring/validation error.

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

The current Format 1 `Npc` type is an implementation limitation. When the format
needs canonical characters that may be player-controlled in one route and
non-player-controlled in another, evolve toward a shared `Character` entity
rather than duplicating the same person as separate NPC/player definitions.

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
advancing an objective, changing a relationship, moving a character, changing
story phase or reaching an authored outcome. Add variants with the capability
that actually needs them.

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
  `HasItem { quantity }`.
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
- Hidden-versus-disabled presentation is not part of condition semantics.
  Conditions answer whether something is legal/available; presentation decides
  how unavailable authored actions are shown.
- Do not build generic conflicting-write analysis now. Authored order defines
  deterministic behavior; higher-level validation/simulation can diagnose bad
  content when concrete cases justify it.

The current Format 1 `requires: Vec<Condition>` is implicitly conjunctive and is
an implementation limitation. A future format revision can introduce the
composable condition tree when richer story branching first needs it.

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
- Whether the player sees exact odds, qualitative difficulty, or no preview is a
  presentation decision rather than part of check semantics.

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

Normal world time is monotonic. Time travel/loops are separate explicit mechanics
rather than weakening ordinary time semantics.

### Scheduled world-time events

When advancing world time crosses scheduled events, process due events in
chronological order. Events with the same timestamp resolve in stable authored
declaration order. Do not add a generic priority framework unless a concrete
world requires one.

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

When instances are needed, use deterministic monotonically allocated
playthrough-local IDs. Save and restore both existing IDs and the next allocation
counter. Instance IDs are never reused within a playthrough.

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
tombstones. Instance IDs are not reused. Gameplay facts that must survive removal
belong to the relevant durable state, such as quest/objective state, evidence,
flags or another capability-owned record.

Do not build a universal runtime provenance/history graph. An instance's
`definition_id` supplies its basic authored origin; capabilities may retain
additional provenance such as crafting/encounter origin only when gameplay needs
it.

Investigation evidence remains a separate authored identity/state. It may
reference a physical definition or instance, but possession of that object does
not automatically imply that its evidentiary significance has been discovered.

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
6. each explicit stochastic result used by required progression has a valid
   authored continuation, recovery path or outcome; and
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

### Preview/detail policy

RealmKit does not globally require exact or qualitative previews. Combat/check
presentation may show exact numbers, qualitative difficulty or no preview according
to world/client policy. Do not add a universal disclosure-level abstraction until
multiple capabilities require common semantics.

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

## Discussion order

Discuss decisions immediately before their first consumer:

1. M1: presentation and information disclosure.
2. M2: outcomes, definitions/instances, and save compatibility.
3. First non-combat fixture: universal core, conditions/effects, skills/checks.
4. M3/M4: time, combat values, equipment and crafting details.
5. M7: canon divergence, original characters, capability provenance and generation
   completion criteria.
6. Multiplayer only when an authoritative server becomes active work.
