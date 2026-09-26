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
- On ordinary recoverable death, play resumes from the most recent valid recovery
  save snapshot. Manual saves and auto-saves participate in the same chronological
  recovery history; the newest valid snapshot wins.
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

**Resolved foundation:** source-backed worlds support three player-entry modes:

```text
canonical
original
both
```

- `canonical`: the player controls a canonical protagonist and follows a
  source-grounded main questline with bounded freedom around required canon
  anchors.
- `original`: the player controls a new character in the same shared world and
  canonical timeline. Canonical protagonists remain world entities/NPCs and their
  required story anchors continue to occur.
- `both`: the package contains both routes and New Game offers the choice
  immediately. The routes use separate mutable saves and do not implicitly
  transfer inventory, relationships, injuries, flags or quest state.

The route choice determines who is player-controlled, not which canonical
characters exist. In an original-character *The Return of the Condor Heroes*
route, Yang Guo still exists and proceeds through the authored canonical
timeline; the player's own main questline may intersect with and locally affect
his story without replacing him.

Static player identity and mutable state are separate. Conceptually:

```text
PlayerSpec  = who the player-controlled character is
PlayerState = mutable state intrinsic to that character
GameState   = mutable state of the whole playthrough
```

"Player" must not be treated as synonymous with "protagonist". In a canonical
route they may be the same character; in an original route the canonical
protagonist remains a world NPC while the player controls someone else.

Canonical-character binding, background and other fixed identity belong to
authored `PlayerSpec`-like content. The existing engine `PlayerState` remains
the mutable player component. Quest progression, story/world flags, dialogue and
other run-wide state belong to `GameState` or typed sub-state beneath it rather
than being folded into `PlayerState` merely because they affect the player.

Open questions:

- Which original-character attributes does the user supply: name, identity,
  background, abilities, relationships to canon characters, insertion point?
- Which attributes may worldgen propose for approval?
- May the player customize appearance/name at runtime without invalidating
  pre-authored grammar or dialogue?
- How is player knowledge represented so an original character cannot act on
  facts that worldgen knows but the character has not learned?

Current leaning: generation establishes a concrete role, knowledge boundary and
insertion point. Runtime customization is a separate optional capability. Do not
add a first-class `Campaign` type until a concrete multi-route package needs it.

## 6. Conditions and effects

**Needed before:** expanding dialogue, investigation or multi-route state.

Define the shared typed vocabulary used by story branches and capabilities.
Likely conditions include flags, optional quest/objective state, item possession,
evidence-obtained state, relationship thresholds, time windows and entity state.
An evidence condition is not shorthand for inventory possession: investigation
may obtain evidence from testimony, observation or a physical entity. Likely effects include
setting flags, transferring items, changing relationships, advancing objectives,
moving entities and reaching authored outcomes.

Open questions:

- Do conditions need AND, OR and NOT initially, or can authored branches keep the
  first version conjunctive?
- Which numeric comparisons are needed, and which create needless scripting?
- Can an effect batch fail atomically when one effect is invalid?
- How are conflicting simultaneous effects ordered?
- When does repeated application become an error versus an idempotent no-op?

Current leaning: closed Rust enums, atomic effect batches and explicit ordering.
No arbitrary expression language or embedded scripts.

## 7. Randomness and reproducibility

**Needed before:** any randomized check, loot or encounter.

Open questions:

- Which mechanics benefit from randomness rather than authored uncertainty?
- Is the seed chosen by the package, player or new-game operation?
- Must the exact PRNG algorithm be part of the format version?
- What is the stable draw order when several effects resolve together?
- Can authors require a fully non-random campaign?

Current leaning: randomness remains optional. If enabled, use a specified seeded
PRNG whose state is saved and included in replay. Never use platform randomness
implicitly. Weighted authored outcomes still consume deterministic draws.

## 8. Skills and checks

**Needed before:** reusable persuasion, stealth, scholarship or survival rules.

Open questions:

- Are checks deterministic thresholds, seeded rolls, resource spends, player
  reasoning, or capability-specific combinations?
- Does failure close content, add a consequence, or offer another approach?
- How are difficulty and expected proficiency exposed to authors and players?
- Can critical information be lost to chance?
- Are broad shared skills useful, or should worlds define only source-relevant
  proficiencies?

Current leaning: critical progression cannot depend on a single unlucky roll.
Stats unlock approaches or change costs/consequences; investigation rewards player
reasoning and obtained evidence. Add a shared skill system only after two
capabilities need the same semantics.

## 9. Time models

**Resolved foundation:** RealmKit separates narrative progression from optional
clock/calendar simulation and from encounter scheduling.

Keep four concepts distinct:

```text
real time           human thinking/input time; never advances game rules
story phase         discrete narrative/canonical progression; universal core
world time          optional in-world clock/calendar for schedules and deadlines
encounter timeline  local deterministic ordering inside combat/other encounters
```

### Story phase

Story phase is part of the universal world model. It represents bounded periods
of narrative/canonical progression and is advanced primarily by explicit
main-quest milestones or other authored story transitions.

Free exploration, reading, menu use, side quests, ordinary dialogue and player
thinking do not implicitly advance the canonical story. A player can therefore
explore the side content available in one phase without the source timeline
silently running ahead.

Story-phase transitions may change NPC locations/availability, world
presentation, side-quest availability, routes and other authored world state.

### World time

World time is an optional capability, not a universal requirement. Enable it only
when the source/gameplay needs concepts such as:

- travel durations
- day/night behavior
- NPC schedules
- appointments
- rest/recovery tied to elapsed time
- explicit deadlines
- calendar/date-sensitive events

Commands do not have universal durations. Presentation-only actions such as
`look`, map/menu navigation, inventory/status/journal inspection and similar
browsing consume no world time. When world time is enabled, meaningful
advancement is authored or capability-defined—for example a six-day journey or
"rest until morning"—rather than inferred from arbitrary command counts.

Hard deadlines are not a default RealmKit mechanic. Source adaptations should
normally let the main story wait for explicit progression; add deadlines only
when the source or intended gameplay actually depends on them.

### Encounter timeline

The encounter timeline is local scheduling state used to answer which actor acts
next. Its units have no intrinsic conversion to world time or wall-clock seconds.
Combat balancing parameters such as action cost, speed and speed caps must not
accidentally change narrative/calendar chronology.

If an encounter should consume world time, that relationship is an explicit
authored/capability effect after or around the encounter; do not derive it by
summing combat ticks.

### Real time

Real-world time spent reading, thinking or choosing never advances story phase,
world time or encounter rules.

Conceptually:

```text
main-quest milestone ───────→ StoryPhase transition

optional authored action ───→ WorldTime advance

combat/encounter action ────→ EncounterTimeline scheduling

human thinking time ────────→ no gameplay-time effect
```

Open questions:

- What concrete representation should optional world time use when first needed
  (for example integer minutes, authored periods, or calendar-aware timestamps)?
- How should simultaneous world-time events be ordered deterministically?
- When world time exists, which recovery/schedule mechanics need shared engine
  semantics versus capability-specific rules?
- How should exact deadlines versus qualitative time pressure be presented to the
  player?

Combat timing details, action costs, speed cap and armour penalties remain in the
[roadmap](../ROADMAP.md).

## 10. Definitions, instances and identity

**Needed before:** saves, equipment, repeatable encounters or movable NPCs.

Separate authored definitions from mutable playthrough instances. This is already
agreed for equipment but needs consistent treatment for NPCs, enemies, clues,
containers and encounters.

Resolved here:

- Investigation evidence has its own authored identity/state. It may optionally
  reference a physical entity or item as its subject/source, but it need not be a
  physical object at all; testimony, observations and deductions can also become
  evidence.
- A physical object can participate in both inventory and investigation through a
  typed cross-reference. Do not encode investigation semantics as a generic item
  tag or infer evidence solely from possession.

Open questions:

- Which entities can have multiple instances?
- How are deterministic instance IDs allocated and preserved across saves?
- Can an instance change definition through transformation or disguise?
- How are spawned/removed entities represented without losing provenance?

Current leaning: stable authored definition IDs plus monotonically allocated,
saved instance IDs. Avoid instances for content that is inherently unique.

## 11. Save compatibility and package evolution

**Needed before:** M2 save/load.

Open questions:

- How is a save bound to a package ID and content revision?
- Which changes are compatible with existing saves?
- Are migrations owned by the world package, RealmKit, or both?
- How are atomic writes, backup saves and corruption diagnostics handled?
- Can a package update remove content referenced by a save?

Current leaning: versioned saves record engine format, package ID/revision,
player-route ID (when a package contains more than one route) and all deterministic
state. Refuse unknown incompatibilities rather than silently resetting fields.
Add migrations only for real released changes.

## 12. Validation, reachability and simulation

**Needed incrementally:** with every new capability.

Open questions:

- What does “reachable” mean when several mutually exclusive endings are valid?
- How does validation distinguish impossible content from intentionally secret or
  optional content?
- Which player policies should deterministic simulation exercise?
- How are difficulty and balance targets authored?
- What proof is required before worldgen calls a package complete?

Current leaning: validators report structural errors separately from reachability
warnings. Required outcomes/objectives declare themselves. Simulations state their
starting state, policy and assumptions; failure under one policy is not proof of
impossibility.

## 13. Capability selection and provenance

**Needed before:** automated source compilation.

Open questions:

- What evidence must worldgen cite to justify enabling a capability?
- When is an implication strong enough—for example, a village blacksmith implying
  player forging versus merely supplying an NPC occupation?
- How does the user approve, add or remove proposed capabilities?
- Should the package retain the selection rationale, or only authoring artifacts?
- How are cross-capability interactions declared and validated?

Current leaning: worldgen proposes the smallest capability set with source
references and a short rationale. The user approves it before detailed content
generation. The exported playable package need not contain source text, but an
optional provenance sidecar can retain the rationale and references.

## 14. Presentation and information disclosure

**Needed first:** M1 menu navigation; revisited per capability.

A 3×3 local neighborhood view is agreed for spatial presentation. The player is
shown in the center; surrounding cells may show nearby locations for context.
Traversal remains cardinal-only through explicit north/south/east/west exits.
Diagonal cells are informational, not implicit movement edges. Vertical
`up`/`down` travel remains a separate contextual action.

Map layout and connectivity are separate concepts: presentation may know that a
location lies northeast of the player even when there is no direct traversable
edge to it. The engine remains authoritative over exits and movement legality.
Arrow/Enter navigation, numbered shortcuts and typed commands remain supported.

Open questions:

- Which actions appear disabled with a reason, and which remain hidden to avoid
  spoilers?
- Should combat previews show exact damage and future turns or qualitative hints?
- How are long action lists grouped on small terminals?
- How much of the 3×3 neighborhood should be shown before discovery: exact names,
  silhouettes/unknown cells, or only locations the protagonist could reasonably
  know or see?
- Should spatial layout use authored integer coordinates, area-local placement
  metadata, or another representation that does not constrain the traversal graph?
- Where should fixed client translations live: RealmKit-owned locale resources,
  package-provided interface text, or a hybrid with well-defined fallback rules?
- How should a client behave when it does not have fixed-interface translations
  for the world's declared language? Silent fallback to English is not acceptable
  for normal source-backed play.
- What accessibility behavior is required for color, screen readers and terminals
  without raw input support?

Current leaning: show actions the protagonist could reasonably consider; explain
ordinary unmet requirements but hide secret branches. Retain the line-oriented
fallback for scripts and inaccessible raw-terminal environments. All displayed
UI must follow the world's language; the open decision is where those fixed
translations are owned, not whether localization is required.

## 15. Equipment and crafting details

**Needed before:** M4.

The [equipment proposal](equipment.md) records the agreed source-gating,
definition/instance split, forging/improvement/enchanting operations and recipe
learning. Still decide material progression, improvement tiers, proficiency
thresholds, enchantment learning/replacement, modifier stacking, exact armour
speed penalties and actual speed cap. Durability, random affixes and crafting
feedback loops remain deferred until a source/world requires them.

## 16. Multiplayer authority and pacing

**Needed before:** any server milestone; not needed for single-player work.

The server will be authoritative and execute the same engine commands. Still
decide how a paused action timeline waits for multiple players, handles disconnects,
resolves simultaneous choices and controls information visible to each player.
Do not constrain current single-player rules around hypothetical networking.

## Discussion order

Discuss decisions immediately before their first consumer:

1. M1: presentation and information disclosure.
2. M2: outcomes, definitions/instances, and save compatibility.
3. First non-combat fixture: universal core, conditions/effects, skills/checks.
4. M3/M4: time, combat values, equipment and crafting details.
5. M7: canon divergence, original characters, capability provenance and generation
   completion criteria.
6. Multiplayer only when an authoritative server becomes active work.
