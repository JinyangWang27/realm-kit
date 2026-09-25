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

A future world-builder skill should teach the author to:

1. Extract canon, chronology, relationships and source references before gameplay.
2. Build a coherent location graph and distinguish characters from combat enemies.
3. Derive quests and encounters from source events with narrative justification.
4. Preserve characterization, vocabulary, rhythm, tone and the source language.
5. Prewrite all runtime prose, dialogue choices and supported alternate branches.
6. Validate references and simulate progression, then repair problems before export.

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

Future agents can iterate on `validate_world()` diagnostics instead of scraping
CLI text. Graph/quest reachability analysis, richer provenance, source extraction,
and MCP tools should be added with their first concrete consumers.
