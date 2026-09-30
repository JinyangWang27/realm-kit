# Living sandbox worlds

Status: design direction. The rules below are proposals. This document does not
change the current package format or the active milestone. The roadmap schedules
the work as [M5](../ROADMAP.md#m5--longer-authored-adventures-and-source-specific-mechanics)
foundations and [M6](../ROADMAP.md#m6--living-sandbox-worlds--proposed)
capabilities, and [Section 18](open-decisions.md#18-living-sandbox-worlds) of the
open-decisions register tracks the questions this document leaves open.

RealmKit's fixtures so far are authored adventures: the player moves through
places, talks, fights and follows a questline. A *living sandbox* is a different
kind of world. The story is mostly what the player makes of it, and the world
keeps moving whether or not the player is watching. Like the
[non-spatial simulation note](simulation-worlds.md), this document is an
architecture test. It asks what RealmKit must add so that a sandbox is one more
world composition, not a separate engine.

## Reference scenario: a realm of rival kingdoms

The player starts with a horse, a sword and a few coins. They travel an overland
map of towns, castles and villages. They buy goods cheaply in one market and
sell them dearly in another. They hire village recruits and train them into
veterans, take jobs from guild masters and lords, and hunt the bandits that prey
on caravans. They earn renown, swear fealty to a king and receive a village as
a fief. Later they may join a knightly order, build its chapter house, marry
into a noble house or crown themselves.

Meanwhile lords ride out with their own war parties. Kingdoms declare war and make
peace. Castles fall to sieges that the player never sees, and invaders raid the
borders. A later visit finds a new banner over a town the player once knew.

No part of this needs a concrete source game. The scenario only pushes
RealmKit's abstractions and tests.

## Architecture test

A mature RealmKit should play this scenario with the guarantees every world
keeps:

- The runtime is deterministic and offline. The same package, seed and commands
  produce the same events and state, including everything the world did
  without the player.
- Save and load capture every piece of state needed to continue exactly, such as
  roaming parties, prices and pending schedules.
- Randomness comes from explicit, saved RNG streams and selects only among
  authored possibilities.
- Every capability below is optional. A world that uses none of them carries no
  state, commands or screens for them.
- Content supplies bounded numbers and typed rules, never formula strings or
  scripts.

## What already fits

- **Places.** Towns, castles and villages are locations. A location's
  `characters` list places lords, merchants and guild masters.
- **People.** Lords and companions are characters with dialogue. Conversations
  already branch on flags, quest states and technique ranks.
- **Personal fights.** Bandit ambushes and duels are encounters. Authored
  groups, fleeing, yielding and critical hits all exist
  ([Encounters](world-format.md#encounters)).
- **Gear.** Equipment instances, forging, improvement and enchanting exist
  ([Equipment](equipment.md)).
- **Chance and continuity.** A seeded, saved RNG exists, and so do saves and
  recovery.
- **Endings.** Route outcomes are designed with separate `completion` and
  `terminal` properties
  ([Section 3](open-decisions.md#3-outcomes-failure-and-replay)), so a sandbox
  can mark an ambition as achieved and keep playing.

## Foundations

These are general improvements that a sandbox needs first. Most of them are
already planned or decided for authored adventures. The roadmap delivers them as
M5 slices.

1. **Condition tree.** Conditions become `All / Any / Not` over typed leaf
   predicates, and a `requires` list becomes one condition. A sandbox adds many
   numeric predicates: currency held, standing with a faction, whether two
   factions are at war, who holds a location, the world time. Each predicate
   stays domain-typed, for example `CurrencyAtLeast`, `StandingAtLeast`,
   `AtWar`, `HoldsLocation` or `WorldTimeWithin`. There is still no generic
   numeric comparison and no variable bag
   ([Section 6](open-decisions.md#6-conditions-and-effects)).
2. **Effect lists.** A dialogue choice carries `effects`, a list applied in
   authored order to staged state, in place of today's single optional `effect`.
   The whole transition commits or fails together. Each effect variant arrives
   with the capability that owns its state: paying coin, changing standing,
   adding troops, granting a holding.
3. **World time and travel.** Optional world time follows
   [Section 9](open-decisions.md#9-time-models): minutes from an authored epoch,
   advanced only by explicit actions. The sandbox needs three additions:
   - Roads carry an authored travel duration.
   - Exits are no longer limited to the six compass directions, because a
     crossroads town may have more roads than that.
   - Actions that pass time: wait for a duration and rest until an hour.

   One-shot scheduled events are already specified. The new proposal is
   **recurring schedules**, such as "every 1,440 minutes from minute 360". The
   dispatch cursor expands them lazily, one due occurrence at a time, in the
   same chronological order as one-shot events. Wages, taxes, market restocking
   and world-agent ticks all run on recurring schedules. The engine never polls
   a clock.
4. **Quest lifecycle.** The current quests are one-shot and cannot fail. A
   sandbox needs quests that fail, carry a deadline in world time and can be
   abandoned. Failure is an ordinary authored state change, not an outcome,
   unless the route says otherwise. It also needs **offers**: repeatable job
   templates such as "deliver cloth to a town" or "clear the bandits near a
   village".
   - An offer's parameters are drawn by the saved RNG from authored candidate
     lists (target location, goods, reward band).
   - The drawn values fill the offer's text with the existing single-pass
     template substitution.
   - Offers are activities, not side questlines. They do not break the rule
     that main progress unlocks side questlines in bounded waves.
5. **Runtime instances for characters and parties.** A wounded lord, a
   captured companion and a roaming war party each need mutable identity that
   outlives one encounter. This follows
   [Section 10](open-decisions.md#10-definitions-instances-and-identity) and is
   already planned for M5.
6. **Proficiencies outside combat.** Trading, leadership and field surgery are
   proficiencies in the [Section 8](open-decisions.md#8-checks-and-proficiencies)
   sense. Technique ranks already serve as one: M4d gates recipes on a Smithing
   rank. However, techniques and recipes live in the combat block today. Moving
   techniques out of that block lets a world without combat still rank its
   traders and surgeons.
7. **Sandbox routes.** A route still owns exactly one main questline. In a
   sandbox, that questline is a ladder of ambitions ("hold a fief", "found an
   order", "rule a kingdom") with **completed, non-terminal** outcomes, so play
   continues after the ambition is reached.

## Sandbox capabilities

Each capability below states what it owns, what the player can do, and what it
adds to conditions and effects. Like every capability, it is present only when
the world authors it, and it arrives with a fixture that proves it.

### Economy

- **State.** The player's currency, a fungible quantity. Each market's stock of
  trade goods.
- **Rules.**
  - A market is a location that trades an authored set of goods.
  - Price is computed in the engine, in integers, from an authored base price,
    the market's modifier and its current stock. A trade proficiency may
    narrow the gap between buying and selling prices. Content never supplies a
    formula.
  - Stock drifts back towards authored levels on a recurring restock schedule.
- **Commands.** Buy and sell. The engine previews the price before the player
  confirms.
- **Conditions and effects.** `CurrencyAtLeast`; `PayCurrency` and
  `GrantCurrency`.
- **Bounds.** Validation caps prices, stock and currency so that engine
  arithmetic stays small, following the existing `*_BOUND` pattern.

### Factions and standing

- **Relationship to the core.** Factions already exist as world entities
  ([Section 1](open-decisions.md#1-minimum-universal-world-core)). This
  capability adds mutable state to them.
- **Diplomacy.** A war/peace relation between each pair of factions. It starts
  as authored and changes only through effects: `DeclareWar` and `MakePeace`.
- **Standing.** Authored standing tracks, each a bounded integer with named
  thresholds. A world might author renown (global), honour (global), standing
  with each faction, and relation with each lord. There is no universal list of
  tracks.
- **Conditions and effects.** `StandingAtLeast`, `AtWar`; `ChangeStanding`.

### Retinue

- **Troops.** Troop definitions carry a combat profile, wages and an optional
  upgrade path (recruit → footman → sergeant).
- **Roster.** The player's roster is a count per troop definition: troops are
  fungible, not instances.
- **Companions.** Companions are unique characters who join the retinue. They
  keep their own gear and technique ranks.
- **Rules.** Recruiting happens at locations that offer it. An authored rule,
  typically a standing threshold or a leadership rank, sets the roster size
  limit. Wages fall due on a recurring schedule. What happens when they go
  unpaid (desertion, lost standing) is authored.
- **Upgrades.** Training moves troops up their upgrade path using XP from
  battles.

### Mass battle

The [capability catalog](capabilities.md#combat-across-sources) keeps mass
battles out of personal combat. This capability resolves army against army.

- **Resolution.** The engine computes each side's strength from its roster, its
  leaders' ranks and the ground, which is an authored location modifier. It
  draws from the RNG and distributes casualties over several rounds. The player
  sees the result and the losses, not a blow-by-blow fight.
- **Champions.** Optionally, the author can let the player fight a personal
  encounter against the enemy commander at a chosen moment. The result of that
  duel modifies the battle.
- **Simulator parity.** The formula must be mirrored in `scripts/combat_sim`
  before its numbers are pinned in tests, exactly as the personal-combat
  formulas are.

### Holdings

- **Ownership.** A holding is a location with an owner, a faction or a
  character, that can change during play.
- **Income.** A holding produces income and supplies on a recurring schedule.
- **Garrison.** It keeps a garrison roster.
- **Buildings.** Authored improvements (a mill, walls, a chapter house) cost
  currency and world time and change the holding's numbers.
- **Sieges.** A siege is a mass battle against the garrison after an authored
  preparation time. The walls improve the defenders.
- **Conditions and effects.** `HoldsLocation`; `GrantHolding`,
  `TransferHolding`.

### Politics and orders

Politics composes the capabilities above rather than adding a new one.

- **Membership ranks.** A faction may author ranks, each with requirements
  (standing, a holding, currency). The same shape covers vassalage and
  membership of a knightly order.
- **Orders.** An order is a faction with ranks and its own troop definitions.
  Its chapter house is a building on a holding.
- **Courtship and marriage.** Relationships plus authored dialogue.
- **Founding a kingdom.** The player's own kingdom is an authored, dormant
  faction that an effect activates. Nothing is created at runtime beyond
  instances, so validation still sees every faction that can ever exist.

### World agents

World agents are what make the world move without the player.

- **Parties.**
  - A party is a runtime instance of an authored party template: a roster, an
    owning faction, a position and a goal.
  - A position is either a location or a point on a road between two
    locations.
  - Lords' war parties, bandit gangs, caravans and invading hosts are all
    parties.
- **Ticks.** On each world tick, which is a recurring schedule, every party
  applies one policy from a closed, engine-defined set: patrol, raid, escort,
  besiege and return home.
  - Authored priorities choose the policy. For example, a lord at war besieges
    the nearest enemy holding with a weak garrison, and otherwise patrols home
    lands.
  - Parties act in a stable order.
  - Their random choices draw from their own RNG domain, so adding a bandit
    gang does not change the player's critical hits.
- **Spawners.** Locations spawn parties from templates, with authored caps and
  intervals. A destroyed party is removed; its instance ID is never reused
  within the same history.
- **Interception.** When the player and a hostile party share a road or arrive
  at the same place, the player's travel stops there. The player chooses to
  fight, talk or flee.
- **Off-screen battles.** Parties that meet away from the player resolve through
  mass battle.
- **Diplomacy.** Kingdoms change diplomacy through authored, weighted world
  events that are checked on ticks. Lords are not scripted individually.
- **Bounds.** A new `AGENT_BOUND` caps the number of live parties, so ticks
  and saves stay small.

### Tournaments and other set pieces

Tournaments, arena fights and wagers need no capability. They are encounters
plus currency effects, gated by standing or world time.

## Reference fixture

Every capability ships with a fixture that proves it. The sandbox fixture is a
small, original world, provisionally `examples/marches`:

- three towns, one castle and two villages on a road network;
- two kingdoms and one knightly order;
- one bandit spawner and one caravan route.

It grows one slice at a time, the way `arena` and `smithy` did. Its walkthrough
covers 30 or more in-world days. During them:
- the player trades, recruits, wins a mass battle, receives a fief and reaches a
  non-terminal ambition outcome;
- a war changes a holding's owner without the player's involvement.

## Non-goals

- a scripting or behaviour language, or per-character AI scripts;
- real-time play, or any gameplay tied to the wall clock;
- terrain, continuous coordinates or pathfinding in the rules (the road graph is
  the map; clients may draw it however they like);
- entities created at runtime other than instances of authored definitions;
- a generic bag of numeric variables;
- importing data from other games.
