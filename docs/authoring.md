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
possible but keep protagonist-specific main questlines, starting state and saves
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

Static protagonist identity and mutable play state are separate concerns.
Conceptually, `ProtagonistSpec` answers "who is the player?" while
`PlayerState` answers "what has happened to them?". Canonical identity,
background and source-character binding belong to authored content; current
location, quest/story phase, faction membership/reputation, relationships and
optional capability state belong to mutable runtime/save state.

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
