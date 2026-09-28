# RealmKit

A small, deterministic text-RPG engine for authored or generated worlds.
**AI authors the game at build time. RealmKit runs the finished game offline.**

“RPG” does not require combat. RealmKit's long-term model is a narrative world
plus source-grounded capabilities: a detective story may use interviews, clues,
deductions and accusations while omitting combat completely. The current demo and
format are the first combat-enabled slice, not a requirement for every world.
See the [architecture glossary](docs/glossary.md) and
[optional capability catalog](docs/capabilities.md).

The first playable world, *The Bell in the Pines*, is hand-authored. Talk to
Elder Mara, choose dialogue, accept her quest, defeat the ash wolf, collect loot
and XP, level up, and return to unlock the chapel and its crypt.

## Play

Install a stable Rust toolchain, then run from the repository root:

```sh
cargo run -p realmkit-cli -- play examples/demo-world/
```

Or install the CLI:

```sh
cargo install --path crates/realmkit-cli --locked
realmkit play examples/demo-world/
realmkit validate examples/demo-world/
realmkit inspect examples/demo-world/
```

Building initially downloads Rust dependencies. Playing requires only the
compiled `realmkit` binary and the world directory: no network, account, API key,
model, worldgen crate, or source material.

In a terminal, each scene shows a context-sensitive menu: use ↑/↓ and Enter,
or press the number. Esc steps back out of a conversation,
`n/s/e/w/u/d` (or `h/j/k/l`) move directly, and `:` opens a typed command
such as `:talk elder`. Ctrl-C quits.

Piped input, scripts and `realmkit play <world> --line` use line mode: type a
menu number or a command and press Enter. Type `help` for the command list.
Typed commands still work everywhere:

```text
look
talk elder
choose 1
choose 1
north
attack wolf
attack wolf
attack wolf
south
talk elder
choose 1
inventory
status
quests
east
down
quit
```

Choices and scene actions are numbered from one among the currently visible
options. Normal menu play uses authored display names rather than requiring entity
IDs; typed commands such as `accept <quest-id>` and `complete <quest-id>` still
use stable machine IDs. Progress exists only for the current session; restarting
starts over.

The same journey is available as a scripted smoke test:

```sh
cargo run -p realmkit-cli -- play examples/demo-world/ < examples/demo-world/walkthrough.txt
```

## Crate boundaries

| Crate | Owns | Local dependencies |
| --- | --- | --- |
| `realmkit-spec` | Serializable playable content, package loading, structured validation | None |
| `realmkit-engine` | Commands, events, explicit in-memory state, deterministic rules | `realmkit-spec` |
| `realmkit-worldgen` | Typed authoring operations, validation feedback, package export | `realmkit-spec` |
| `realmkit-cli` | Input parsing, terminal I/O, stored-text rendering | `realmkit-engine`, `realmkit-spec` |

The engine is synchronous. `Engine::new(&world)` validates its input;
`execute(Command)` returns structured events and commits the resulting state.
`state()` exposes an immutable view. Failed commands leave state unchanged.
Given the same world and command sequence, state and events are identical.
There are no clocks, random generators, network clients, or AI SDKs in gameplay.

Combat deals fixed damage followed by a surviving enemy's counterattack.
Damage is capped at remaining HP. Each monster is a unique, non-respawning
instance; defeat rewards happen once. Levels use authored cumulative XP
thresholds and fully restore HP. Quests remember earlier defeats, so accepting
after a kill does not strand the quest. Death stops actions; inspection remains
available, and a new session starts a fresh game.

All story prose comes from the world package. Format 1 currently chooses combat
template variants from deterministic engine progression; M1 makes `look`,
inventory, status and quest-panel inspection spend no turn, so presentation-only
browsing no longer perturbs later combat prose. The exact variant-selection key is
still an implementation detail rather than a content contract. Nothing generates
new prose or branches during play.

## World authoring

Packages use versioned, UTF-8 JSON files in a directory. JSON keeps the first
format small and dependency-light; it is an output detail, not the authoring
API. See [the format guide](docs/world-format.md) and the editable demo files.

`realmkit-worldgen::WorldDraft` supplies typed location creation, updates,
directional links, structured validation, and export. External agents can
construct `realmkit-spec` values and drive this core. It does not own an LLM
or implement a source compiler yet. See [the authoring guide](docs/authoring.md).

**Everything displayed to a player in a source-backed world must use the
source's language.** World-authored names, descriptions, dialogue, choices,
quest text and templates are generated in that language, and RealmKit clients
must localize their own fixed labels, help, prompts and runtime messages to the
world's declared language. Machine-facing schema keys, IDs, enum values and
stable typed-command tokens may remain language-neutral ASCII.

Source-backed drafts check that the declared source and world languages match;
this is a metadata check, not linguistic verification. Authors must also review
the actual text. The hand-authored demo is English. The current CLI still has
English-only fixed interface labels; full package-language UI localization remains
future presentation work and is not an exception to the language invariant.

## Verify

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build -p realmkit-engine -p realmkit-cli --locked
cargo tree -p realmkit-cli
```

After dependencies are cached, add `--offline` to build/test commands. The CLI
dependency tree contains no worldgen or AI dependency. Tests cover the full
terminal journey, deterministic replay, rejected commands, reward idempotence,
overflow rollback, death, validation, authoring/export, source-language
metadata, and Unicode template interpolation.

## Scope

See [the milestone roadmap](ROADMAP.md) for proposed menu interaction, saves,
combat stats and timing, character builds, and source-grounded authoring.
The [non-spatial simulation note](docs/simulation-worlds.md) records the
long-term architecture pressure test for career/life/institution simulations
without making that work part of the current milestone. Normative architecture
terms are defined in the [glossary](docs/glossary.md). Unresolved design questions
and their decision points are tracked in the
[open-decisions register](docs/open-decisions.md).

This is a playable scaffold, not a complete RPG system. Equipment, skills/MP,
multi-target kill counts, encounters, independent dungeon instances, factions,
save files, TUI, multiplayer, and source compilation are deferred. The crypt is
an ordinary graph location. No empty crates or placeholder runtime systems
are created for those features.

Future authoring can add canon IR, provenance sidecars, world-building
instructions, simulation, source-specific gameplay capabilities, and CLI/MCP
adapters without changing the engine's AI-free boundary. Validation currently
checks structure and references; it does not prove reachability, narrative
fidelity, or solvability.
