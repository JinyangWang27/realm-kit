# RealmKit open decisions

This is the discussion register for architectural and game-design choices that
are intentionally unresolved. It prevents an implementation detail from silently
becoming policy. Decide items when their milestone needs them; do not block M1 on
questions that only affect later generation or multiplayer work.

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
- Every playable world has a location graph, a main questline, and one or more
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
- Failed commands do not mutate state. Presentation uses structured commands and
  events shared by typed commands, menus and future clients.
- Combat uses gradual defence reduction with explicit immunity/vulnerability.
  Immunity overrides minimum damage; vulnerability and defence bypass differ.
- Speed controls action frequency on a proposed paused timeline. Effective speed
  has a cap, and heavy armour can trade speed for protection.
- Recipe knowledge and proficiency are separate: authored sources teach recipes;
  proficiency gates their use.

These decisions can still be revised deliberately, but they are not open by
default during implementation.

## 1. Minimum universal world core

**Needed before:** the first non-combat format revision.

Resolved minimum core:

```text
world identity
controlled protagonist
location graph
authored narrative / dialogue / choices
main questline
quest objectives
story phases
flags and typed state
typed conditions and effects
one or more outcomes
```

Additional decisions:

- The location graph is universal. A constrained story may use only one or a few
  locations, but movement/place remains part of RealmKit's world model.
- Every world has a main questline. Side questlines are optional.
- A questline contains quests; quests contain one or more typed objectives.
- NPCs, locations, factions and reusable entities belong to the shared world,
  not to an individual quest. Quests reference and change that shared world.
- Story phases provide bounded open-world periods around major main-story
  transitions. Main progress changes phase and can move NPCs, alter availability,
  unlock/close questlines, and change world presentation.
- Inventory is optional.
- Evidence is owned by an investigation capability. Evidence may refer to a
  physical entity such as a letter, weapon or photograph, including an item that
  can also be carried in inventory, but evidence state is separate from item
  possession. Prefer a typed relation/reference over an unstructured string tag.

Open question:

- Which concepts, if any, should later be extracted from a single-world model into
  a separate `Campaign` type when one package supports multiple independently
  playable protagonists/storylines?

Avoid answering by making every field optional. The core should express the
smallest real playable world clearly.

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

## 2. Outcomes, failure and replay

**Needed before:** saves and the first non-combat fixture.

Define first-class outcomes such as ongoing, completed, failed and named alternate
endings. Death is one possible outcome, not the universal failure state.

Open questions:

- Can a campaign continue after a nominal ending, or does an outcome freeze it?
- Which outcomes count as completion for unlocking another campaign?
- Does failure restart the campaign, restore a checkpoint, or remain as a valid
  ending chosen by the world?
- Can one save discover several endings, or does each branch use a separate save?
- What completion metadata persists outside campaign saves?

Current leaning: authored outcomes declare whether they are terminal, successful,
and eligible for unlocks. Restart/checkpoint behavior belongs to the campaign.

## 3. Canon fidelity and divergence

**Needed before:** source-grounded generation.

RealmKit source adaptations do not permit unrestricted rewrite of the source.
Major canonical anchors remain protected in both canonical and original routes.
Generated play may change routes, local outcomes, assistance, relationships,
side-story consequences and other bounded details, but it must not transform the
source into a fundamentally different story.

Open questions:

- Which canon facts are protected anchors for a given source, and how are they
  represented?
- How should worldgen report invented connective material versus sourced facts?
- How much bounded variation is allowed around each anchor without invalidating
  characterization or causality?

The generation report should record protected anchors and provenance for major
expansions. Runtime only consumes the compiled branches.

## 4. Player entry and original-character generation

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

Static identity and mutable state are separate. Conceptually:

```text
ProtagonistSpec = who the player is
PlayerState     = what has happened to the player
```

Canonical-character binding, background and initial identity belong to authored
content. Location, story phase, quests, faction membership/reputation,
relationships and enabled capability state belong to mutable runtime/save state.

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

## 5. Conditions and effects

**Needed before:** expanding dialogue, investigation or multi-campaign state.

Define the shared typed vocabulary used by story branches and capabilities.
Likely conditions include flags, optional quest/objective state, item possession,
evidence-obtained state, relationship thresholds, time windows and entity state.
An evidence condition is not shorthand for inventory possession: investigation
may obtain evidence from testimony, observation or a physical entity. Likely effects include
setting flags, transferring items, changing relationships, advancing objectives,
moving entities and ending campaigns.

Open questions:

- Do conditions need AND, OR and NOT initially, or can authored branches keep the
  first version conjunctive?
- Which numeric comparisons are needed, and which create needless scripting?
- Can an effect batch fail atomically when one effect is invalid?
- How are conflicting simultaneous effects ordered?
- When does repeated application become an error versus an idempotent no-op?

Current leaning: closed Rust enums, atomic effect batches and explicit ordering.
No arbitrary expression language or embedded scripts.

## 6. Randomness and reproducibility

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

## 7. Skills and checks

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

## 8. Time models

**Needed before:** combat timeline implementation, schedules or deadlines.

Keep three concepts distinct:

```text
real time        player thinking time; never advances game rules
story time       dates, travel, schedules and deadlines
action timeline  fine-grained ordering inside encounters
```

Open questions:

- Which commands advance story time, and by authored or calculated amounts?
- How does an encounter's action timeline map back to story time, if at all?
- What happens when several story events share a timestamp?
- How do rest, recovery and NPC schedules interact?
- Are deadlines visible exactly or described narratively?

Combat timing details, action costs, speed cap and armour penalties remain in the
[roadmap](../ROADMAP.md). Do not equate combat ticks with wall-clock seconds.

## 9. Definitions, instances and identity

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

## 10. Save compatibility and package evolution

**Needed before:** M2 save/load.

Open questions:

- How is a save bound to a package ID and content revision?
- Which changes are compatible with existing saves?
- Are migrations owned by the world package, RealmKit, or both?
- How are atomic writes, backup saves and corruption diagnostics handled?
- Can a package update remove content referenced by a save?

Current leaning: versioned saves record engine format, package ID/revision,
campaign ID and all deterministic state. Refuse unknown incompatibilities rather
than silently resetting fields. Add migrations only for real released changes.

## 11. Validation, reachability and simulation

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

## 12. Capability selection and provenance

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

## 13. Presentation and information disclosure

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

## 14. Equipment and crafting details

**Needed before:** M4.

The [equipment proposal](equipment.md) records the agreed source-gating,
definition/instance split, forging/improvement/enchanting operations and recipe
learning. Still decide material progression, improvement tiers, proficiency
thresholds, enchantment learning/replacement, modifier stacking, exact armour
speed penalties and actual speed cap. Durability, random affixes and crafting
feedback loops remain deferred until a source/world requires them.

## 15. Multiplayer authority and pacing

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
