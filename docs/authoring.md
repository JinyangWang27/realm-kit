# Typed world authoring

`realmkit-worldgen` is an authoring library. An external coding agent, editor,
or future compiler supplies intent and builds typed `realmkit-spec` values:

```text
Source + world-building instructions
                  ↓
           External author/agent
                  ↓
        Typed WorldDraft operations
                  ↓
      WorldSpec → diagnostics → repair
                  ↓
       Validated static package
```

The library does not own an autonomous AI agent, credentials, an LLM pipeline,
or a YAML editor. JSON serialization occurs at export. Future Rust, CLI and MCP
interfaces should adapt this same core rather than implement their own rules.

## Current API

```rust
use realmkit_spec::{Direction, Exit, WorldSpec};
use realmkit_worldgen::WorldDraft;

fn author() -> Result<(), Box<dyn std::error::Error>> {
    let world = WorldSpec::load("examples/demo-world")?;
    let mut draft = WorldDraft::new(world);
    let mut garden = draft.get_world().location("village").unwrap().clone();
    garden.id = "garden".into();
    garden.name = "The Walled Garden".into();
    garden.description = "The last apples lie in the wet grass.".into();
    garden.npcs.clear();
    garden.monsters.clear();
    garden.exits.clear();
    draft.create_location(garden)?;
    draft.link_locations("village", Direction::West, Exit {
        destination: "garden".into(),
        requires: vec![],
        blocked_text: "The garden gate is shut.".into(),
    })?;
    for diagnostic in draft.validate_world() {
        eprintln!("{}: {}", diagnostic.code, diagnostic.message);
    }
    draft.export("my-world")?;
    Ok(())
}
```

`create_location` rejects duplicates. `update_location(Location)` replaces an
existing location by ID and rejects missing IDs. `link_locations` sets one
directed exit, checking both endpoints. Drafts may have unresolved references
while being authored; validate before export. Other content can be constructed
directly using `WorldSpec` types before opening a draft. Add further typed
operations when an actual authoring workflow needs them.

`export` validates and creates a **new** directory. It refuses existing paths,
including existing empty directories. The parent directory must exist. An I/O
failure can leave a partial output directory; exports are not published
atomically yet. Retry to a new path after resolving the error.

## Language and source fidelity

For source-backed authoring, use `WorldDraft::from_source(world, source_language)`.
The draft emits `source_language_mismatch` and refuses export if the world's
declared language differs from the source language (case-insensitive tag
comparison). An empty source language is also rejected.

Every generated player-facing string **must be in the same language as the
source input**: names, location/NPC/monster/item/skill descriptions, dialogue,
choices, quest introductions/progress/completion, story branches, encounter
text, combat templates, ambient passages, victory and death text. Internal
IDs, schema keys, enum values and stable typed-command tokens remain
machine-oriented.

The same-language rule applies to the final play experience, not only generated
world content. RealmKit clients must localize their own fixed labels, help,
prompts, status messages and player-visible errors to the world's declared
language. Those strings need not be generated from the source, but they must not
silently fall back to English. The current M0 CLI is English-only and therefore
requires presentation-layer localization before a non-English source-backed
world fully satisfies this invariant.

A language tag does not prove that prose obeys this rule; author review must
check the text itself. Do not silently fall back to English or generic fantasy
prose.

## Player entry mode

World generation asks how the player wants to enter the source world. The
authoring model supports three product-level choices:

```text
canonical
original
both
```

In `canonical` mode, the player controls an extracted canonical protagonist.
For a *The Return of the Condor Heroes* world this could be Yang Guo. Worldgen
preserves that character's identity, knowledge, relationships, voice and major
canonical anchors while allowing exploration, side questlines and bounded
variation in how events are reached.

In `original` mode, the player controls a newly authored character inserted
into the same shared world and canonical timeline. Canonical protagonists do not
disappear or get replaced: Yang Guo, Xiaolongnü, Guo Jing, Huang Rong and other
canonical characters remain world entities/NPCs and continue through their
pre-authored canonical story. The original player has a separate main questline
that intersects with, observes and locally influences that timeline without
invalidating its required canon anchors.

In `both` mode, the static package contains both independently playable routes.
New Game offers the choice immediately; one route does not have to unlock the
other. They reuse the same world definitions and canonical timeline where
possible but keep player-route-specific main questlines, starting state and saves
separate. There is no implicit transfer of inventory, injuries, relationships,
flags or quest state between routes.

The route choice controls who is player-controlled, not who exists in the world:

```text
shared world / canon timeline
├── canonical route: player = Yang Guo
└── original route:  Yang Guo = canonical NPC
                     player = original protagonist
```

Do not introduce a dedicated runtime `Campaign` abstraction merely to express
this before a concrete multi-route package needs one. For now, treat "route" as
the conceptual unit for an independently playable protagonist/storyline. The
current Format 1 supports one fixed route. Multiple simultaneously controlled
protagonists remain a separate future design.

World character identity and player control are separate concerns:

```text
Character   = who exists in the world
PlayerSpec  = which Character the human controls
PlayerRoute = how that player experiences the world
```

Future shared character identity should live on a `CharacterSpec`-like world
entity. `PlayerSpec` should primarily reference that character rather than
duplicate name/background/identity. A canonical route binds control to a canonical
character; an original route binds control to a worldgen-authored original
character while canonical protagonists remain ordinary world characters/NPCs.

Original-character generation is constraint/override driven. Resolve the final
identity before generating dependent route prose, quests, relationships and
dialogue. Starting location/phase belong to the route; mutable relationships,
faction state and capability data belong to route/game/capability state rather
than `PlayerSpec`.

Worldgen must maintain a player-knowledge boundary distinct from omniscient Canon
IR knowledge. Compile only gameplay-relevant knowledge distinctions into runtime
state. Original starting history/relationships must satisfy canon constraints and
must not manufacture major retroactive canonical relationships.

The existing engine `PlayerState` stores mutable state intrinsic to the
controlled character. Run-wide quest, dialogue, NPC and world state belongs to
`GameState` or typed state owned beneath it. Runtime identity customization is
opt-in and typed rather than a universal right to rewrite authored identity.

A future world-builder skill should teach the author to:

1. Extract canon, chronology, relationships and source references before gameplay.
2. Build a coherent shared location graph and a readable spatial layout. Keep
   traversal exits separate from presentation placement: the 3×3 local map may
   show diagonal nearby locations, while actual movement remains through explicit
   cardinal exits. Keep NPCs/locations/factions as world entities rather than
   nesting them inside quests.
3. Compile the canonical story into a main questline and major story phases.
4. Extract side-story seeds from canonical people, places, factions, conflicts,
   occupations and unresolved details, then expand those seeds into side
   questlines before inventing generic filler.
5. Gate side questlines by explicit main-story/story-phase progress. Let their
   outcomes feed typed state back into later main quests where authored.
6. Preserve characterization, vocabulary, rhythm, tone and the source language.
7. Prewrite all runtime prose, dialogue choices and supported alternate branches.
8. Validate references and simulate main/side progression, including unlock and
   feedback paths, then repair problems before export.
9. Apply the user's player-entry route choice and validate each independently
   playable route from its own starting state.

Optional mechanics also require source grounding. Equipment may exist without a
player crafting system. Add forging, improvement, enchanting, alchemy or similar
loops only when the source mentions or reasonably supports them. When absent,
omit their definitions, progression, stations and UI actions completely; do not
fill every adaptation with a default fantasy-RPG feature set.

Combat follows the same rule. It is a capability, not the definition of a
RealmKit world. A detective novel can compile to locations, interviews, clues,
evidence, deductions and authored accusation branches without HP, monsters,
attacks, damage text or combat progression. Conversely, do not force an
investigation system into a source that does not support one. Choose the smallest
set of mechanics that expresses the source's important conflicts and choices.
See the [optional capability catalog](capabilities.md) for selection criteria,
examples and validation/runtime expectations.

Capability absence must be complete and valid. The authoring API should not emit
dummy combat stats, empty combat prose or invisible placeholder monsters to pass
validation. Validators check each present capability as a coherent bundle and
accept its complete absence. Presentation exposes actions from present content
and state, so absent systems leave no empty screens or disabled global commands.

These are authoring instructions, distinct from the Rust definitions of valid
and executable content. There is no skill framework in this scaffold.

## Open-world quest authoring

RealmKit's intended source-adaptation shape is one shared open world with a
canonical main questline plus optional side questlines. Side content should not
form disconnected mini-worlds: it reuses and changes the same NPCs, locations,
factions and world state as the main story.

Main progress establishes a `story_phase` (or equivalent typed progression
state). A phase exposes a bounded set of side questlines. Players can explore and
finish those stories in any supported order, but later side content remains
locked until the main story advances. This provides Skyrim-like freedom within
the source chronology without making the canonical story permanently optional.

Side quests may influence the main questline, but only through explicit
pre-authored state and conditions. Valid effects include changing dialogue,
relationships/reputation, NPC availability or survival, evidence/knowledge,
available routes, assistance/resources, or substituting/skipping authored main
objectives. Any larger canonical divergence must be permitted by the world's
adaptation policy and compiled before play.

Generation should prefer side stories in this order:

1. reuse canonical NPCs, locations and conflicts
2. expand minor canonical characters/events
3. infer plausible events strongly supported by the setting
4. introduce implied/background characters when needed
5. create new minor material only when the source leaves a genuine gameplay gap

Track whether generated material is sourced, inferred or expanded so reviewers
can distinguish adaptation from invention.

## Canon fidelity

Source-backed generation uses bounded fidelity rather than an unrestricted
adaptation mode. Worldgen extracts and maintains authoring-side Canon IR with
protected canon anchors, broader canon constraints and provenance.

Canon anchors are major source facts/events that supported branches must preserve.
A generated branch may explore, add side stories, change local consequences or
take alternate routes between anchors, but it must still be able to reach every
required downstream anchor in a canon-compatible state. Making a required anchor
permanently unreachable is a validation error.

Broader constraints also preserve established identity, relationships, chronology,
character knowledge, core characterization, world facts and causal consistency.
Do not leak omniscient worldgen knowledge into player/NPC knowledge before an
authored discovery path exists.

Use authoring-side provenance:

```text
SOURCE    directly represented in the source
INFERRED  strongly supported by the source but not explicitly narrated
EXPANDED  new gameplay material constrained by canon
```

Retain source references/rationale in an authoring report or optional provenance
sidecar. Runtime does not require source text, Canon IR, or provenance analysis.

## Time authoring

Treat canonical/narrative progression as **story phases**, not as a continuously
running clock. Main-quest milestones or other explicit authored transitions move
the world between phases. Free exploration and side content inside a phase do not
silently advance the source chronology.

An in-world clock/calendar is optional. When a world needs it, author world time
as a monotonic minute count from a world/route-defined epoch. Presentation may map
that scalar to clock times, dates, day counts or setting-specific periods; the
runtime does not need to understand the display calendar.

Advance world time only through explicit authored/capability actions such as
travel, waiting, rest or appointments. Do not assign generic durations to every
command. Presentation-only actions consume zero world time.

When advancement crosses scheduled events, resolve them chronologically and use
stable authored declaration order for equal timestamps. Capabilities define what
elapsed time means to them; the core time model does not hard-code hunger,
recovery, crafting or NPC-schedule semantics.

Deadlines may use exact runtime timestamps while being described narratively to
the player.

Encounter timelines remain separate local schedulers. Their units exist to order
actors deterministically and do not represent seconds, minutes or calendar time.
If an encounter should consume world time, author that consequence explicitly
rather than deriving it from encounter ticks.

Story phase and world time remain independent. A story-phase transition may also
explicitly advance world time for a canonical time skip, but elapsed world time
never silently advances the source story.

Real-world time spent reading or choosing never changes any gameplay state.

## Deliberately open extensions

Canon IR belongs to worldgen: source identities, aliases, chronology,
relationships and textual evidence are distinct from playable NPCs, quests and
monsters. Runtime `realmkit-spec` stays focused on game content. Provenance can
be added as an authoring-side mapping/optional package sidecar keyed by entity
IDs; the runtime loader ignores additional files and needs no source text.

Future simulation can execute ordinary engine commands against WorldSpec and
return structured feedback. A separate simulation adapter can depend on both
the engine and authoring core without creating any engine → worldgen dependency.
The current deterministic replay tests demonstrate the execution seam; there
is no `simulate_quest` API or automatic balance analysis yet.

The playable spec can grow through explicit optional capability sections. Core
state should contain only universally needed identity/location/flags; combat,
investigation and crafting own their data and mutable state. This is ordinary
typed composition, not a dynamic plugin system. Cross-capability effects must be
explicit—for example, combat may set the same story flags used by dialogue.

Future agents can iterate on `validate_world()` diagnostics instead of scraping
CLI text. Graph/quest reachability analysis, richer provenance, source extraction,
and MCP tools should be added with their first concrete consumers.
