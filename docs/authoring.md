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
IDs and schema keys remain machine-oriented. A language tag does not prove
that prose obeys this rule; author review must check the text itself. Do not
silently fall back to English or generic fantasy prose.

## Protagonist and campaign choice

World generation asks the user how they want to enter the source story. The
initial authoring model should support three explicit choices:

```text
canonical
original
canonical_then_original
```

In canonical mode, the user selects or approves a character extracted from the
source. Worldgen preserves that character's knowledge, relationships, voice and
plausible choices. In original mode, the user supplies or approves a new role and
its insertion point; worldgen must integrate it without casually replacing a
canonical character or granting knowledge the new character could not possess.

`canonical_then_original` compiles two campaigns into the same static package.
The original-character campaign unlocks after an authored completion outcome in
the canonical campaign. This is package-local meta-progression, not generation at
runtime: all locations, dialogue, branches and prose for both campaigns already
exist before the first session begins.

The user also chooses what the second campaign represents:

- Another perspective during the same chronology.
- A continuation after the canonical campaign.
- A bounded alternate branch from an authored point.

Campaigns have separate mutable saves. Completing the first campaign unlocks the
second but does not implicitly copy inventory, injuries, relationships or flags.
The package declares any facts that carry across—for example which canonical
ending occurred, an NPC's survival, or a public event. This prevents accidental
state leakage and lets validation analyze each starting state.

Conceptually, a campaign has an ID, protagonist definition, start location/state,
enabled capabilities, endings, and an optional unlock condition. The current
format supports one fixed campaign; add this typed structure when the first
multi-campaign world is implemented. Multiple simultaneously controlled
protagonists remain a separate future design.

A future world-builder skill should teach the author to:

1. Extract canon, chronology, relationships and source references before gameplay.
2. Build a coherent location graph and distinguish characters from combat enemies.
3. Derive quests and encounters from source events with narrative justification.
4. Preserve characterization, vocabulary, rhythm, tone and the source language.
5. Prewrite all runtime prose, dialogue choices and supported alternate branches.
6. Validate references and simulate progression, then repair problems before export.
7. Apply the user's protagonist/campaign choice and validate each campaign from
   its own starting state.

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
