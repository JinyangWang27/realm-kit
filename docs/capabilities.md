# Optional world capabilities

RealmKit's universal core is small: authored narrative content, locations,
entities, deterministic state, conditions, choices, commands and events. A world
adds only the gameplay capabilities justified by its source. This document is a
catalog for authors and future engine work, not a checklist or promised backlog.

## When something deserves a capability

A recurring source element may deserve a capability when it has all three:

1. State that changes during play.
2. Rules that apply in more than one authored scene.
3. Choices whose consequences the engine must calculate or enforce.

A single locked door can use a condition and flag. Repeated locks involving
tools, skill, risk and consequences may justify lockpicking. A single courtroom
scene can be a dialogue tree; recurring claims, evidence and credibility may
justify a courtroom capability. Prefer authored events until reusable rules add
real expressive value.

Capabilities are presence-driven. A complete absence is valid and creates no
state, validation requirements, screens or commands. A present capability must
be internally complete and machine-valid. Do not add placeholder data to make a
world appear compatible with systems it does not use.

## Capability catalog

| Capability | State it may own | Typical deterministic actions |
| --- | --- | --- |
| Investigation | Clues, evidence, hypotheses, contradictions | Examine, interview, connect, deduce, accuse |
| Relationships | Trust, fear, affection, loyalty, commitments | Confide, persuade, threaten, reconcile |
| Reputation and factions | Standing, rank, alliances, hostility | Join, negotiate, endorse, betray, request aid |
| Stealth and infiltration | Awareness, suspicion, concealment, access | Hide, distract, trespass, pick a lock |
| Pursuit and escape | Relative distance, route, obstacles, stamina | Chase, flee, intercept, take a shortcut |
| Survival | Hunger, fatigue, temperature, injury, supplies | Eat, rest, shelter, forage, treat injury |
| Travel and time | Calendar, journey progress, schedules, deadlines | Travel, wait, camp, choose a route |
| Economy and trade | Currency, prices, merchant stock, debts | Buy, sell, bargain, borrow, repay |
| Equipment | Individual items, slots, effective modifiers | Equip, compare, unequip |
| Crafting | Recipes, materials, proficiency, stations | Forge, improve, enchant, brew |
| Combat | HP/MP, stats, initiative, effects, opponents | Attack, use skill, defend, flee |
| Magic and rituals | Known rites, costs, preparation, curses | Cast, prepare, dispel, perform a ritual |
| Puzzles and mechanisms | Mechanism state, known facts, attempts | Inspect, manipulate, combine, answer |
| Debate and courtroom | Claims, admitted evidence, credibility | Present, challenge, defend, rule |
| Politics and intrigue | Influence, offices, secrets, alliances | Scheme, expose, support, blackmail |
| Party and companions | Membership, role, morale, approval | Recruit, assign, counsel, dismiss |
| Romance | Affection, boundaries, commitments | Court, respond, commit, separate |
| Leadership and settlement | Population, safety, resources, assignments | Assign, build, ration, defend |
| Business or estate | Staff, stock, income, upkeep | Hire, invest, price, expand |
| Collection and scholarship | Texts, artifacts, translations, knowledge | Study, translate, catalogue, compare |
| Performance | Repertoire, audience mood, renown | Perform, rehearse, improvise, compete |
| Transformation | Form, identity, corruption, disguise | Transform, resist, conceal, revert |
| Time loop | Cycle, reset state, retained knowledge | Reset, exploit knowledge, prevent an event |

Names in this table describe design areas, not compulsory Rust modules. Closely
related capabilities can share domain types where that makes the actual code
smaller. Do not create one crate per row or a generic plugin registry.

## Example compositions

A detective adaptation might use:

```text
exploration + dialogue + investigation + relationships + time + accusation
```

It needs no combat, equipment or crafting. The investigation state records which
evidence the player has legitimately obtained; authored dialogue and accusation
branches inspect that state.

A court-intrigue adaptation might use:

```text
dialogue + factions + reputation + secrets + debate + political consequences
```

A wilderness story might use:

```text
travel + time + survival + weather + injury
```

Crafting belongs only if the source supports characters making or improving
things. Combat belongs only if physical or magical conflict is an important
playable part of the source.

Capabilities compose through explicit shared concepts such as entity IDs,
locations, inventory entries, conditions, flags, commands and events. For
example, an investigation can set a story flag consumed by dialogue; combat can
yield a clue; a deadline can close an accusation branch. Cross-capability effects
must be declared and validated rather than discovered through hidden global state.

## World-generation selection

The author or external agent should select capabilities after extracting canon:

1. Identify the source's repeated conflicts, decisions and consequences.
2. Describe the smallest mechanics that let the player participate in them.
3. Cite source passages or canon facts supporting each selected capability.
4. Prefer existing RealmKit capabilities where their semantics match.
5. Represent isolated moments with authored dialogue, conditions and flags.
6. Omit unsupported genre conventions, even if common in other RPGs.
7. Validate and simulate each enabled capability and its interactions.

The resulting authoring report should explain selections and omissions. “It is an
RPG” is not sufficient justification for combat, crafting, loot, classes or an
economy. Generated player-facing text for every enabled capability follows the
source-language rule.

## Validation and runtime expectations

Each capability defines:

- The authored definitions it requires.
- Its mutable state, if any.
- Structured commands and events.
- Validation rules and structured diagnostics.
- Save/load representation.
- Deterministic simulation boundaries.
- Presentation data needed to label its actions in the world's language.

Validation accepts complete absence. When present, it detects missing references,
unreachable required outcomes and incomplete definitions as far as that
capability's analyzer supports. Clients build their action menus from the world
and current state, so absent capabilities remain invisible.

Core state should contain only data truly shared by all worlds. Capability state
uses explicit optional typed composition. This does not require runtime-loaded
plugins, scripting, an ECS, one crate per capability, or AI during play. Add the
smallest typed implementation when a representative world needs it.

Campaign selection sits above capabilities. One package may contain independently
validated campaigns with different protagonists, starting states and capability
sets. An authored completion outcome can unlock another campaign, including an
original-character perspective after a canonical one. Unlocking exposes static
packaged content; it never invokes AI. Campaign saves remain separate unless the
package explicitly maps selected outcome facts between them.

## Suggested implementation order

The current combat demo proves the command/event and static-content seams. The
next non-combat fixture should exercise investigation because it tests the
optional-capability architecture directly. Relationships/reputation and time are
then broadly useful. Inventory/equipment can serve combat and non-combat worlds.
Skill checks should be introduced only with clear semantics for certainty,
difficulty and any deterministic randomness.

Every new capability should ship with one small playable fixture proving why it
exists. A capability without a representative world is speculative scaffolding.
