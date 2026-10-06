# RealmKit

A small, deterministic text-RPG engine for authored or generated worlds.
**AI authors the game at build time. RealmKit runs the finished game offline.**

“RPG” does not require combat. RealmKit's long-term model is a narrative world
plus source-grounded capabilities: a detective story may use interviews, clues,
deductions and accusations while omitting combat completely. Combat is an
optional block: the demo uses it, while
`examples/quiet-archive` is a small world without it.
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
realmkit play examples/quiet-archive/ --language zh-Hans
realmkit text examples/quiet-archive/
```

A world may come in several languages: `inspect` lists them, and
`--language <tag>` plays in one. A save carries over from one language to
another. `text` prints every piece of a world's text by key, the starting
point for a translation. The CLI's own labels and help are in English.

Building initially downloads Rust dependencies. Playing requires only the
compiled `realmkit` binary and the world directory: no network, account, API key,
model, worldgen crate, or source material.

In a terminal, each scene shows a context-sensitive menu: use ↑/↓ and Enter,
or press the number. Stat training and equipment wait behind one entry each
(`Train stats ›`, `Equipment ›`) and stay open while you use them. Esc steps
back out of a submenu or a conversation,
`n/s/e/w/u/d` (or `h/j/k/l`) move directly, and `:` opens a typed command
such as `:talk elder`. Ctrl-C quits. Terminal play adds bold and colour;
set `NO_COLOR` for plain text. A fight takes over the screen with health
bars and its last few lines, and leaves only its outcome behind.

A world may ask a few questions at New Game, such as why you came; your
answers shape the start and appear on the character panel. In a world whose
places have map positions, `Map` opens the overland map on its own screen:
`+`/`-` zoom, the arrows pan, `0` fits every place, `c` centres on you, Tab
steps through the places one road away with their travel time, and Esc
returns to play. Line mode prints the whole map in an 80 × 24 frame, and
`map zoom <0-6> <place>` a closer view of one place.

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
options. A choice you cannot take yet may still be listed, marked `[locked]`
like a locked exit; choosing it says why (`You do not have two silver to
offer.`). A question once asked leaves the list, and "back to the questions"
becomes the way out once nothing is left to ask. Normal menu play uses authored display names rather than requiring entity
IDs; typed commands such as `accept <quest-id>` and `complete <quest-id>` still
use stable machine IDs.

Without a saves directory, progress lasts only for the session. With one, play
resumes where you left off:

```sh
realmkit play examples/demo-world/ --saves ~/.local/share/realmkit/bell
```

The game auto-saves at the start and whenever a quest is completed. Type `save`
to save now, `load` to list saves and `load <number>` to restore one. Loading an
older save abandons the saves made after it. If you die, your newest save is
restored. A save that fails to load is reported, not skipped; the game then
offers older saves for you to choose. Only one game at a time can use a saves
directory; a second is refused.

The same journey is available as a scripted smoke test:

```sh
cargo run -p realmkit-cli -- play examples/demo-world/ < examples/demo-world/walkthrough.txt
```

## Crate boundaries

| Crate | Owns | Local dependencies |
| --- | --- | --- |
| `realmkit-spec` | Serializable playable content, package loading, structured validation | None |
| `realmkit-engine` | Commands, events, explicit state, save snapshots, deterministic rules | `realmkit-spec` |
| `realmkit-worldgen` | Typed authoring operations, validation feedback, package export | `realmkit-spec` |
| `realmkit-cli` | Input parsing, terminal I/O, stored-text rendering, save files | `realmkit-engine`, `realmkit-spec` |

The engine is synchronous. `Engine::new(&world)` validates its input;
`execute(Command)` returns structured events and commits the resulting state.
`state()` exposes an immutable view. Failed commands leave state unchanged.
`snapshot()` captures a storage-neutral `SaveSnapshot` without changing state;
`Engine::restore(&world, snapshot)` resumes it or rejects it whole when it
belongs to another package, revision or route, or holds impossible state.
`Engine::start(&world, seed, &answers)` begins a world that asks start
questions. Nothing assumes a terminal or a filesystem: a browser or desktop
host loads a package from memory with `WorldSpec::from_files`, passes
commands, events, the map view and the journal (`Engine::journal`) as JSON,
and stores
`SaveSnapshot::to_json` bytes wherever it likes; the engine builds for
`wasm32-unknown-unknown`.
Given the same world and command sequence, state and events are identical.
There are no wall clocks, network clients or AI SDKs in gameplay. Randomness exists
only in worlds that author it (critical hits in `examples/arena`, a wandering
storyteller and daily price changes in `examples/marches`), and comes from
a seeded, versioned generator saved with the game: `--seed <n>` replays a run
exactly, and without it the CLI picks a seed from the clock and prints it.

Combat is optional: a world without a `combat` block has no fighting, HP, XP or
levels, and its quests complete through flags set in dialogue
(`examples/quiet-archive`). Where a world has combat, characters have seven
stats and hits deal physical or special damage (the world names special, for
example magic or 内力) through one formula in which defence reduces damage
gradually. Fights are encounters on a paused initiative timeline: speed decides
who acts first and how often, and the game waits for each of your commands.
Skills spend MP, which regenerates as the fight goes on, or rage, which builds
from acting and being hit; opponents answer with their own skills, and the
player can rest at safe places (`examples/duel`). Packs fight together, some
fights can be repeated for grinding, you can flee most of them, and sparring
partners yield instead of dying (`examples/arena`). Worlds can also grant stat
points on levelling up, which you spend to shape your own build, and
techniques mastered rank by rank, each rank named by the world
(`examples/sect`). Equipment is worn in slots, each piece its own item, with
bonuses, heavy weapons and armour that trade speed, and wards against damage.
At a station such as an anvil, known recipes forge new pieces from materials
and improvement tiers make a piece better one step at a time, trained by a
smithing technique. At an altar, a learned enchantment is laid on a piece
once for good, and stays through later improvement (`examples/smithy`).
Worlds can also keep a clock (`examples/marches`). Roads between places take
authored time (`Travel to Ashmere — 2 h`, or `travel ashmere`), `wait 2h` lets
time pass, some people keep hours, others wander from town to town on a
schedule, and events such as a thaw that opens a causeway happen at set times
whether or not you are there. Time moves only when you travel, wait or rest.
A world with a calendar shows the date too, such as `742-03-02 08:00`.
Worlds with an economy have currency and markets: buy smoked eels where the
fen is full of them and sell them where they are scarce (`Market ›`, or
`buy eels 6`, `sell eels 6`, `market`). Every unit you trade nudges the local
price, and once a day each market's prices drift with what it makes and
needs, so a good route stops paying if you flood it. Merchants hold only
what their market makes and a purse that refills each morning, so one stall
cannot take a whole cargo (`[sold out]`), and a town that goes without what
it needs loses prosperity, and with it demand and stock. Trading is a
proficiency: spend the points levels give you (`Proficiencies ›`, or
`train trading`) to narrow the spread. You can also buy a workshop from the
right merchant, such as Maddoc's weavery in Vellmarket. It turns wool into
cloth and pays or costs you its margin every week.
A world with a calendar may keep a bank. At a branch (`Bank — 40 silver ›`,
or `deposit 50`, `withdraw 20`) you leave money in one account that every
branch shares, and it is not carried. At the end of each calendar month the
bank pays interest on the month's average daily balance (`The month ends:
March 742.`, `Bank interest: …`). Your soldiers' wages come out of the bank
first, then your purse.
Some markets also sell wares at a fixed price, gear included, and some items
can be used: the arena's quartermaster sells a healing draught that you
drink from `Use item ›` (or `use healing_draught`), even mid-fight, where
drinking takes your turn.
Worlds with troops let you raise soldiers where they are recruited
(`Recruit ›`, or `recruit levy 6`), pay their wages, daily or, where the
world says so, at each month end for whoever is serving then, and lead them against
armies (`engage outlaws`). Each round you charge, hold the line, flank with
riders or retreat, or autoresolve the rest; nearby allies may join you.
Soldiers who survive share the victory's XP and rise in level together,
renamed as the world decides, and at the end of a line you choose their
branch (`Upgrade ›`). In the marches, levies raised at Ashmere beat the fen
outlaws beside the keep's men.
Worlds may have factions. People show whom they serve (`Steward Brannoc
(Hollin Keep)`), factions are at war or at peace with each other, and the
world measures you on its own standing tracks, such as renown or favour
with each faction. What you do moves them (`Favour with Hollin Keep +20 (now
20, Trusted)`), wars break out and end (`War: the fen bands and the
Vellmarket wool guild.`), and conversations and people can depend on both:
the marches' steward lets a trusted traveller carry terms to the fen, and
once there is peace, the outlaws are gone. The status panel lists your
standing and the wars being fought.
Stories can be told in phases, with a main questline and side quests that
wait for an earlier quest or a later phase, and investigations turn on
evidence. Examining things (`Examine the cold camp`, or `examine cold_camp`)
and talking to people reveals evidence (`New evidence: Wheel ruts`), which
stays known for good and is not the same as carrying an item; places such
as a hidden fork appear on the map, and their roads open, only once you
have learned of them. The Quests panel is the journal: the chapter, your
quests, main ones marked, the evidence you hold with how you read it now,
and the ending reached. When something you learn changes how you read
evidence you already hold, play says so (`Understanding changed — …`).
Evidence can fill in as you go: new facts about something you already know
arrive as `New fact — …` and join it in the journal.
`examples/caravan-trail` is an investigation slice: a caravan reaches
Thornwick, another caravan goes missing, and its trail can be followed by
fighting, paying or facing down bandits, or by finding a survivor, until
three readings of an old stone ring open The Buried Road and you choose a lead.
Damage is capped at remaining HP. Each fighting character is a unique,
non-respawning instance; defeat rewards happen once. Levels use authored
cumulative XP thresholds and fully restore HP and MP. Quests remember earlier defeats, so accepting
after a kill does not strand the quest. Death stops actions; inspection remains
available. With saves on, the CLI restores the newest save.

All story prose comes from the world package. The engine currently chooses combat
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
overflow rollback, death, save/resume equivalence, corrupt-save handling, validation, authoring/export, source-language
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

This is a playable scaffold, not a complete RPG system. Multi-target kill
counts, independent dungeon instances, companions, holdings, world agents,
faction strategy and membership, multiplayer, and source compilation are
deferred. The crypt is
an ordinary graph location. No empty crates or placeholder runtime systems
are created for those features.

Future authoring can add canon IR, provenance sidecars, world-building
instructions, simulation, source-specific gameplay capabilities, and CLI/MCP
adapters without changing the engine's AI-free boundary. Validation currently
checks structure and references; it does not prove reachability, narrative
fidelity, or solvability.
