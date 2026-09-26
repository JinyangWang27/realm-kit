# RealmKit glossary

This glossary defines RealmKit architecture terms that may have broader or
different meanings in ordinary RPG/game-development usage. Normative docs should
prefer these terms consistently.

## World

The shared fictional environment and authored setting. A world owns shared
locations, characters/NPCs, factions/organizations, reusable entities and lore,
the location graph and spatial layout, story-phase definitions, shared narrative
content, and other world-level state/definitions.

A world contains one or more player routes.

## Player Route

One independently playable way of entering and experiencing a world.

A player route has its own player-controlled character definition
(`PlayerSpec`-like authored data), starting state/location, exactly one main
questline, one or more outcomes, and an independent mutable save/playthrough.

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

## Story Phase

A discrete narrative/canonical progression period shared by the world. Story
phases are advanced by explicit authored transitions, usually main-quest
milestones, rather than by real-world time or ordinary exploration.

## Canon Anchor

A protected major source fact or event that a source-backed adaptation must not
invalidate. Routes and side content may vary the path, context and local
consequences around an anchor, but every supported branch must preserve
reachability of required downstream anchors in a canon-compatible state.

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
Provenance supports fidelity review/regeneration and is not required runtime
gameplay state.

## Condition

A pure typed query over current deterministic state used to gate authored actions,
branches and content. Conditions compose through `All`, `Any` and `Not` plus
typed leaf predicates. Evaluating a condition never mutates state.

## Effect

A closed typed state change executed as part of an authored transition/engine
command. Multiple effects execute in authored order against staged state, and the
enclosing state transition commits atomically.

## Capability

An optional reusable gameplay system with recurring mutable state, reusable rules
and engine-enforced consequences, such as combat, investigation, relationships,
inventory or crafting.

The existence of an entity in the fiction does not require its corresponding
mechanical capability. For example, factions can exist as world entities without
a faction/reputation system.

## Spatial Layout

Presentation-oriented information describing where nearby locations appear
relative to one another. The 3×3 local map uses spatial layout.

Spatial layout does not create movement edges.

## Traversal Graph

The explicit location exits that define where the player can actually move.
Cardinal exits remain authoritative even when diagonal nearby cells are visible
in the spatial layout.

## Outcome

An authored route-level result/end state selected from explicit deterministic
conditions. Outcomes independently declare whether they count as route completion
and whether they terminate the playthrough.

A playthrough records at most one immutable route outcome. Ordinary setbacks and
recoverable gameplay death are not outcomes unless explicitly authored as such.

## Recovery Save

A valid saved deterministic playthrough snapshot eligible for death recovery.
Manual saves and auto-saves share one chronological recovery history; ordinary
recoverable death restores the newest valid snapshot.

Auto-saves are created at stable boundaries such as route start and story-phase
transitions rather than on every command or movement step.

## Campaign

A non-normative term in RealmKit.

RPG literature uses "campaign" for several different concepts, including a world,
an adventure, or a playable storyline. RealmKit uses the more precise terms
`World` and `PlayerRoute`. Do not introduce a first-class `Campaign` type
unless a future requirement cannot be expressed cleanly with those concepts.
