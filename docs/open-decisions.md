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
- Generated player-facing content uses the source language.
- Gameplay capabilities are optional and source-grounded. Complete absence is
  valid and produces no placeholder state or UI.
- Combat and crafting are optional. Equipment does not imply crafting.
- World generation offers canonical, original, or canonical-then-original
  protagonist campaigns. Unlockable campaigns are pre-generated.
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

Decide which data every playable world must have. Current leaning:

```text
world/campaign identity
controlled protagonist
current location
authored narrative
choices
flags
typed conditions and effects
one or more outcomes
```

Open questions:

- Must every campaign use a location graph, even if it remains in one courtroom
  or operates through letters and memories?
- Are quests universal, or merely one optional progression presentation?
- Is inventory core because clues and letters are objects, or optional because
  evidence can have its own state?
- Which concepts belong to a campaign rather than the whole package?

Avoid answering by making every field optional. The core should express the
smallest real playable world clearly.

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

Declare an adaptation policy per campaign:

- Strict: routes vary while major canonical events remain fixed.
- Bounded: alternate outcomes are allowed when consistent with established
  characters, knowledge and causality.
- Free: the source provides setting/cast, with broader authored divergence.

Open questions:

- Should bounded divergence be the default?
- Which canon facts may never change, and how are they represented?
- How should worldgen report invented connective material versus sourced facts?
- When a canonical protagonist can make a noncanonical choice, how far may its
  consequences propagate?

The generation report should record the selected policy and provenance for major
deviations. Runtime only consumes the compiled branches.

## 4. Original-character generation

**Needed before:** the first original-protagonist campaign.

Open questions:

- Which attributes does the user supply: name, identity, background, abilities,
  relationship to canon characters, insertion point?
- Which attributes may worldgen propose for approval?
- May the player customize appearance/name at runtime without invalidating
  pre-authored grammar or dialogue?
- How is knowledge limited so an original character cannot act on facts they have
  not learned?
- For canonical-then-original mode, which ending facts can carry forward?

Current leaning: generation establishes a concrete role and knowledge boundary;
runtime customization is a separate optional capability. Campaigns carry only an
explicit mapping of cross-campaign facts.

## 5. Conditions and effects

**Needed before:** expanding dialogue, investigation or multi-campaign state.

Define the shared typed vocabulary used by story branches and capabilities.
Likely conditions include flags, quest/objective state, item/evidence possession,
relationship thresholds, time windows and entity state. Likely effects include
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

Open questions:

- Which entities can have multiple instances?
- How are deterministic instance IDs allocated and preserved across saves?
- Can an instance change definition through transformation or disguise?
- Are clues unique facts, physical items, or sometimes both?
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

Arrow/Enter navigation plus numbered shortcuts and typed commands is agreed.
Open questions:

- Which actions appear disabled with a reason, and which remain hidden to avoid
  spoilers?
- Should combat previews show exact damage and future turns or qualitative hints?
- How are long action lists grouped on small terminals?
- Which fixed interface labels come from a RealmKit locale versus world content?
- What accessibility behavior is required for color, screen readers and terminals
  without raw input support?

Current leaning: show actions the protagonist could reasonably consider; explain
ordinary unmet requirements but hide secret branches. Retain the line-oriented
fallback for scripts and inaccessible raw-terminal environments.

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
