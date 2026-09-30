# RealmKit glossary

This glossary defines RealmKit architecture terms that may have broader or
different meanings in ordinary RPG/game-development usage. Normative docs should
prefer these terms consistently.

## World

The shared fictional environment and authored setting. A world owns shared
locations, characters/NPCs, factions/organizations, reusable entities and lore,
the location graph, story-phase definitions, shared narrative content, and other
world-level state/definitions. It may also provide optional Areas/spatial layouts
for map presentation where useful.

A world contains one or more player routes.

## Player Route

One independently playable way of entering and experiencing a world.

A player route has its own player-control binding (`PlayerSpec`-like authored
data) referencing a shared `Character`, plus starting state/location, exactly one
main questline, one or more outcomes, and an independent mutable
save/playthrough.

Canonical and original-character routes may reuse the same world definitions and
canonical timeline.

## Character

A person/entity that exists in the shared world regardless of who controls it in
a particular route. Canonical and original playable characters should use the
same shared character model as non-player-controlled characters.

A future `CharacterSpec`-like type owns intrinsic authored identity such as
name, description, origin/background and canonical identity. The current Format 1
`Npc` type is a narrower implementation scaffold.

## Player

The character controlled by the human in a particular player route.

"Player" describes control, not narrative importance. The player may be the
source story's protagonist, but does not have to be.

## Protagonist

A narrative role: a principal character of the source story or authored
storyline. RealmKit does not use "protagonist" as a synonym for "player".

In an original-character route, a canonical protagonist may remain an NPC/world
entity while the player controls someone else.

## PlayerSpec

The planned static route-level binding that identifies which shared `Character`
the human controls. It should not duplicate intrinsic character identity or absorb
mutable route/capability state.

This type does not exist in Format 1 yet; the name describes the intended static
counterpart to the existing engine `PlayerState`.

## PlayerState

Mutable runtime state intrinsic to the player-controlled character. The engine
already has a `PlayerState`; future capability-specific player state may be
composed beneath it as needed.

Run-wide quest, dialogue, NPC and world state should not be placed here merely
because it affects the player.

## GameState

Mutable state of the whole playthrough. It contains or owns the player state and
run-wide state such as quest progress, world/story flags, dialogue state and
future NPC/world state.

## Main Questline

The required story-progression spine of one player route. Every player route has
exactly one main questline. It contains quests, which contain typed objectives.

## Side Questline

An optional coherent storyline layered over the shared world. Side questlines may
be gated by main-story/story-phase progress and may feed explicit authored state
back into later main-quest content.

A quest required for main progression is not a side quest.

## Offer

Proposed for M5. A repeatable job template, such as a delivery or a bandit hunt,
whose parameters the saved RNG draws from authored candidate lists at an
explicit gameplay transition, never when a menu opens. An offer is an activity
rather than a side questline, so the bounded waves that gate side questlines do
not apply to it.

## Story Phase

A discrete narrative/canonical progression period shared by the world. Story
phases are advanced by explicit authored transitions, usually main-quest
milestones, rather than by real-world time or ordinary exploration.

## World Time

Optional simulated in-world time represented as a minute count from an authored
epoch. It is monotonic within one forward committed history and advances only
through explicit gameplay actions/effects. Loading or recovering an older snapshot
restores its saved WorldTime exactly, so discarded-future values may be traversed
again on the new branch. WorldTime is independent of story-phase progression and
encounter scheduling.

Presentation may render the same scalar using setting-appropriate clocks, dates or
qualitative periods.

## Recurring Schedule

Proposed for M5. An authored period of at least one minute and a first
occurrence in World Time. The dispatch cursor expands it lazily, one due
occurrence at a time, alongside one-shot scheduled events. Upkeep, restocking and world-agent ticks run on
recurring schedules.

## Encounter Timeline

A local deterministic scheduler used to order actions inside an encounter. Its
units have no implicit conversion to real time or optional World Time.

## Encounter

Proposed for M3. Active local combat state: an ordered list of participants,
each a Character with a combat profile, a side, a controller (player or authored
policy), HP/MP, rage and a next action time on the Encounter Timeline. While
active, it is the sole owner of its participants' HP and MP. Rage exists only
inside an encounter.

## Combat Profile

Proposed for M3. The optional component that lets a Character take part in
encounters: its combat stats plus any loot/XP granted when it is defeated. A
character without one cannot be engaged.

## Canon Anchor

A protected major source fact or event that a source-backed adaptation must not
invalidate. Routes and side content may vary the path, context and local
consequences around an anchor. Every continuing supported branch must preserve
reachability of the downstream anchors required for that branch/outcome in a
canon-compatible state.

An explicitly permitted terminal divergence may end that branch before later
anchors, in which case anchors after the authored endpoint are not required for
that branch. Terminality alone does not waive already-established canon facts or
constraints.

Canon anchors are primarily authoring/worldgen concepts rather than runtime
gameplay entities.

## Canon Constraint

A broader source-fidelity rule that may not correspond to one event, including
established identity, relationships, chronology, character knowledge, core
characterization, world facts and causal consistency.

## Expansion Space

The authored space between protected canon anchors in which RealmKit may add
exploration, side questlines, minor characters, inferred events, alternate
methods, relationship development and bounded local consequences.

## Provenance

Authoring metadata classifying generated source-backed material as `SOURCE`,
`INFERRED`, or `EXPANDED`, with source references or rationale where useful.
The authoring report may also retain capability-selection rationale, meaningful
omissions and explicit author/user overrides. Provenance supports fidelity
review/regeneration and is not required runtime gameplay state.

## Condition

A pure typed query over current deterministic state used to gate authored actions,
branches and content. Conditions compose through `All`, `Any` and `Not` plus
typed leaf predicates. Evaluating a condition never mutates state.

## Effect

A closed typed state change executed as part of an authored transition/engine
command. Multiple effects execute in authored order against staged state, and the
enclosing state transition commits atomically.

## Validation

Hard package/save correctness checks required for safe execution. Structural,
reference and invariant failures are errors; validation does not by itself prove
that every gameplay path is solvable.

## Reachability Analysis

Bounded/domain-specific analysis of whether an authored target can become
available from a relevant starting state. Required targets proven unreachable are
errors; apparently unreachable optional/secret content is normally a warning.

## Simulation

Execution of real RealmKit engine commands from an explicit starting state under
a stated player policy and RNG seed/state. A simulation is evidence for that
particular path, not a proof of every possible playthrough.

## Capability

An optional reusable gameplay system with recurring mutable state, reusable rules
and engine-enforced consequences, such as combat, investigation, relationships,
inventory or crafting.

The existence of an entity in the fiction does not require its corresponding
mechanical capability. For example, factions can exist as world entities without
a faction/reputation system.

## Standing

Proposed for M6. An authored, bounded integer track with named thresholds,
scoped globally, per faction or per character, such as renown or a lord's
relation with the player. Conditions read it and effects change it.

## Market

Proposed for M6. A location that trades an authored set of goods. Each good has
a saved price index, in thousandths of its base price, that a recurring tick
moves against the market's authored production and demand. The engine derives
integer buying and selling prices from it.

## Prosperity

Proposed for M6. A market's bounded integer measure of wealth. It drifts towards
an ideal set by scarcity and buildings, rises with trade, falls with raids and
sieges, and scales income, tariffs, merchant stock and recruit pools.

## Workshop

Proposed for M6. A player-owned business in a town that runs one processed
good's recipe and pays the local profit, possibly a loss, on a recurring
schedule. It is owned property, not a holding.

## Retinue

Proposed for M6. The player's troops, held as a count and an XP pool per troop
definition, plus any companions, who are unique characters wearing gear from a
shared stash. An engine-computed limit caps its size, and wages fall due on a
recurring schedule.

## Prisoner

Proposed for M6. A captured troop, counted per troop definition, or a captured
unique character whose captivity is state keyed by its character ID. Prisoners
are held by the player, an agent party or a holding, within that holder's
prisoner limit, and may be sold, recruited, ransomed, released or escape.

## Trait

Proposed for M6. An authored personality trait from a world's closed list, such
as cautious or cruel. Characters author a few; traits never change during play.
Agent policies, diplomacy weights, dialogue and reactions to the player's deeds
may test them.

## Dialogue Role

Proposed for M5. An authored role such as lord, guild master or companion that
characters list. Its dialogues, topics and line slots are shared by every
member, and inside them `Speaker` refers to the member being talked to.

## Holding

Proposed for M6. A location owned by a faction or character that can change
hands during play, with income, a garrison and authored buildings.

## World Agent

Proposed for M6. A runtime party instance, such as a lord's war party, a bandit
gang or a caravan, that acts on world ticks by applying a policy from a closed,
engine-defined set. Agents move the world without the player.

## Proficiency

Capability-owned competence state used when a mechanic needs it, such as
smithing or stealth. RealmKit does not require a universal proficiency table or
shared numeric scale.

A proficiency is distinct from a learned ability/technique.

## Check

Capability-specific resolution of a legal gameplay attempt. A check may be
deterministic, stochastic, resource-based or otherwise explicitly defined by that
capability.

A failed check is a valid gameplay result and may have authored consequences. It
is distinct from a rejected command, which does not commit state changes or
consume randomness.

## Randomness

An optional engine-level deterministic pseudo-random facility used by world-level
systems and capabilities such as rare events, encounters, critical hits, loot or
checks.

Randomness selects among authored possibilities; it never generates content.
Active RNG state is saved/restored with the playthrough, and presentation/pure
condition evaluation never consumes random draws. Semantically independent random
domains should not be coupled accidentally through unrelated draws.

## Area

An optional spatial/presentation grouping of locations, such as a city, building
floor or wilderness region. An area may provide local map positions for its
locations, but does not own traversal semantics.

Areas are not required for graph-only worlds and are not a replacement for the
World/PlayerRoute hierarchy.

## Spatial Layout

Optional presentation-oriented placement of locations inside an Area, initially
expressible as area-local integer `(x, y)` coordinates when a grid is useful.

Spatial layout never creates movement edges. A client may render an entire compact
area or any viewport size appropriate to its screen. A fixed 3×3 viewport is not a
RealmKit semantic requirement.

## Traversal Graph

The explicit location exits that define where the player can actually move.
Cardinal exits remain authoritative even when diagonal nearby cells are visible
in the spatial layout.

## Definition

Stable authored content identity. A definition describes what an entity/object is
independently of mutable playthrough state.

Inherently unique authored entities may use their DefinitionId directly as runtime
identity. Repeatable definitions may produce runtime instances when distinguishable
copies need independent mutable state.

## Instance

A distinguishable runtime copy of a repeatable or unique authored definition,
identified by a deterministic `InstanceId` that is unique within its authoritative
identity domain. A single-player playthrough is one such domain; multiplayer
chooses a domain large enough that instances which may be transferred or jointly
referenced cannot collide.

An instance retains an immutable `definition_id` while ownership, location,
damage, improvement, enchantment and other mutable state may change. Instance IDs
and allocator state are saved only when their authoritative identity domain is
fully inside the same recovery scope. An ID is not reused within one forward
committed history; restoring an older snapshot may reuse IDs that existed only in
the discarded future branch. Instances are created only where distinct copy
identity is actually needed.

## Unique Definition

An authored definition that permits at most one live runtime instance. Uniqueness
does not eliminate the need for an instance when the physical object carries
mutable state; a legendary weapon is the typical example.

## Outcome

An authored route-level result/end state selected from explicit deterministic
conditions. A valid route initial state matches no outcome. While a playthrough
has no recorded outcome, the engine evaluates outcome conditions after successful
state-changing transitions; outcomes are derived from state rather than triggered
by a separate outcome effect. If multiple outcomes match the same staged state,
the transition fails atomically with an ambiguity error.

A playthrough records at most one immutable route outcome. Once recorded, outcome
evaluation stops for that playthrough even if a non-terminal outcome allows play
to continue. Ordinary setbacks and recoverable gameplay death are not outcomes
unless explicitly authored as such.

## Save Snapshot

A storage-neutral serialized representation of deterministic mutable playthrough
state, bound to a package ID/revision and player route. The engine defines and
validates the snapshot; clients/servers choose the persistence backend.

A save snapshot contains mutable state and stable references to authored
definitions rather than copying the full world package.

## Recovery Scope

The state scope that a checkpoint is permitted to rewind. A single-player
playthrough may rewind its whole state; an isolated multiplayer instance may
rewind itself; a persistent shared world normally cannot be rewound because one
player dies.

## Recovery Save

A saved deterministic playthrough snapshot eligible for death recovery.
Manual saves and auto-saves share one active chronological recovery lineage.
Loading an older snapshot starts a new active lineage from that point; snapshots
from the abandoned future are not automatic-recovery candidates. Ordinary
recoverable death attempts the newest active snapshot; if it fails load
validation, the client may explicitly offer an older snapshot from that lineage,
but recovery never silently skips the corrupt entry.

Auto-saves are created at stable boundaries such as route start and story-phase
transitions rather than on every command or movement step.

## Authoritative Server

The multiplayer process that owns canonical mutable state, executes and
revalidates player commands, resolves RNG, assigns committed command order and
persists the resulting state. Clients submit intents/commands rather than trusted
outcomes.

## Authoritative Scope

A mutable multiplayer state scope whose commands must be ordered consistently,
such as world-, player-, party/session- or encounter-owned state. Independent
scopes may progress concurrently. Concrete scope types are introduced only when a
multiplayer system needs them.

## Campaign

A non-normative term in RealmKit.

RPG literature uses "campaign" for several different concepts, including a world,
an adventure, or a playable storyline. RealmKit uses the more precise terms
`World` and `PlayerRoute`. Do not introduce a first-class `Campaign` type
unless a future requirement cannot be expressed cleanly with those concepts.
