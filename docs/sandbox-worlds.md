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
   **recurring schedules**, such as "every 1,440 minutes from minute 360", with
   a period of at least one minute. The
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
     lists (target location, goods, reward band). The draw happens only at an
     explicit gameplay transition, such as the giver's recurring refresh or the
     player's arrival, and the drawn offer is saved. Opening a menu or talking
     only reads offers that already exist, so inspecting jobs never consumes
     randomness ([Section 7](open-decisions.md#7-randomness-and-reproducibility)).
   - The drawn values fill the offer's text with the existing single-pass
     template substitution.
   - Offers are activities, not side questlines. They do not break the rule
     that main progress unlocks side questlines in bounded waves.
5. **Runtime instances for spawned copies and parties.** A roaming war party, a
   bandit gang or a caravan needs mutable identity that outlives one encounter,
   and several copies of one template may be alive at once. Unique authored
   characters keep their authored IDs: a wounded lord's wounds, a captured
   companion's captivity and a lord's whereabouts are state keyed by the
   character ID, not a second identity. This follows
   [Section 10](open-decisions.md#10-definitions-instances-and-identity), and
   instances for combatant copies are already planned for M5.
6. **Sandbox routes.** A route still owns exactly one main questline, and a
   playthrough records at most one outcome
   ([Section 3](open-decisions.md#3-outcomes-failure-and-replay)). In a sandbox,
   the main questline is a ladder of ambitions: quests such as "hold a fief"
   and "found an order" are ordinary quest progression. Only the top rung, such
   as "rule a kingdom", is the route's **completed, non-terminal** outcome, so
   play continues after it.

Proficiencies such as trading, leadership and field surgery are not a
foundation. They follow [Section 8](open-decisions.md#8-checks-and-proficiencies):
each capability that needs one owns its ranks and checks. The economy owns
trading, the retinue owns leadership, and mass battle owns surgery. A technique
rank is not such a proficiency. M4d's Smithing technique, which gates recipes,
already blurs that line; [Section 18](open-decisions.md#18-living-sandbox-worlds)
records the question.

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
    the market's modifier and its current stock. A trading proficiency, owned
    by this capability, may narrow the gap between buying and selling prices.
    Content never supplies a formula.
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
- **Roster.** The player's roster holds, per troop definition, a count and an
  XP pool: troops are fungible, not instances. Both are saved.
- **Companions.** Companions are unique characters who join the retinue through
  authored dialogue effects and leave the same way. Each keeps its authored ID
  and its own state: wounds, technique ranks and, in worlds with levels, its
  own XP and level. See [Companion gear](#companion-gear).
- **Recruiting and wages.** Recruiting happens at locations that offer it.
  Wages fall due on a recurring schedule. What happens when they go unpaid
  (desertion, lost standing) is authored.
- **Size limit.** The engine computes the roster limit as an integer: an
  authored base plus authored contributions from standing tracks and the
  retinue's leadership proficiency. Companions count towards it; prisoners have
  their own limit (see [Prisoners and ransom](#prisoners-and-ransom)). A command
  that would exceed the limit is refused. If the limit later drops, for example
  after lost renown, nobody leaves; recruiting is refused until the roster is
  back under it. Agent party templates author their own fixed limits, and a
  validation bound caps every roster.
- **Upgrades.** Battles add XP to the pool of each surviving troop type, and
  recruits join with no XP. Every troop that leaves a type, whether by upgrade,
  casualty or desertion, takes its share of that type's pool: the pool divided
  by the count, rounded down. A troop can upgrade once its share reaches the
  next step's authored XP. It pays that XP from its share and carries the rest
  into its new type's pool. A casualty's or deserter's share is lost, and a
  type whose last troop leaves also drops any rounding remainder. XP is only
  moved or spent, never created outside battle, so a pool never outlives its
  troops and upgrades available after a save are the same as before it.

### Companion gear

Equipment today is worn by the player alone. With a retinue, every retinue
character can wear gear:

- **Wearer.** Each equipment instance has at most one wearer, the player or a
  companion, and the retinue shares one stash for unworn pieces. The saved
  instance records its wearer; moving a piece between characters keeps its
  instance ID, tier and enchantment.
- **Equipping.** The equip command names the wearer. Slots, swaps and the
  before → after preview work exactly as for the player, against that
  character's slots.
- **Derived stats.** Every retinue character's effective stats are derived by
  one function, like `rules::player_stats` today: the character's base
  (combat profile, or level table plus allocated points where the world gives
  companions levels), technique passives and worn gear. They are never saved.
- **In fights.** Companions join personal encounters as allies on the player's
  side, which the encounter state already supports, and count as leaders in a
  mass battle. They are wounded rather than killed unless an authored rule says
  otherwise.
- **Leaving.** A companion who leaves takes nothing with them by default; their
  worn pieces return to the stash unless an authored effect says otherwise.

### Prisoners and ransom

- **State.** A prisoner roster held by the player, by an agent party or by a
  holding's prison: a count per troop definition, plus unique characters whose
  captivity is state keyed by their character ID (held by whom, since when).
  Prisoners have their own authored limit per holder.
- **Taking prisoners.** When a mass battle ends, an authored share of the
  losing side's casualties, drawn from the battle's RNG domain, become the
  winner's prisoners instead of dying, up to the winner's limit. A defeated
  unique leader is captured, not killed, unless an authored rule says
  otherwise. A personal encounter can capture a yielding opponent the same way.
- **Using prisoners.**
  - Sell troop prisoners to a ransom broker at a location, for an engine-computed
    price per troop definition (economy).
  - Recruit prisoners into the retinue when an authored condition allows it,
    which counts against the roster limit.
  - Ransom a captured lord: an authored offer from the lord's faction, drawn at
    a gameplay transition like any offer, pays currency for the release.
  - Release a prisoner, with authored standing effects.
- **Escape.** On a recurring schedule, prisoners may escape with an authored
  chance from the agents' RNG domain; a holding's garrison and buildings lower
  it.
- **The player captured.** Losing a mass battle may capture the player instead
  of ending the route. Captivity is an ordinary setback
  ([Section 3](open-decisions.md#3-outcomes-failure-and-replay)): travel and
  most actions are refused until an authored release (time served, ransom paid,
  or escape) ends it. The retinue is scattered or captured as authored.
- **Conditions and effects.** `HoldsPrisoner`, `IsCaptive`; `CapturePrisoner`,
  `ReleasePrisoner`, `RansomPrisoner`.

### Mass battle

The [capability catalog](capabilities.md#combat-across-sources) keeps mass
battles out of personal combat. This capability resolves army against army.

- **Resolution.** The engine computes each side's strength from its roster, its
  leaders and the ground. It draws from the RNG and distributes casualties over
  several rounds. The player sees the result and the losses, not a
  blow-by-blow fight.
- **Ground.** Locations and roads both author a ground modifier, neutral when
  omitted, so a battle has ground wherever parties can meet: at a location or
  on a road between two.
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
- **Prison.** It holds prisoners within an authored limit that buildings can
  raise, and a captured lord taken there waits for ransom or escape.
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
- **Culture is not a runtime concept.** Culture decides which troops a village
  offers, the troop line a faction fields, and its names and prose, but none of
  that changes during play. Locations and factions reference their troop
  lines and recruit pools directly; an authoring tool may group those
  references as a "culture" before export. Culture becomes runtime state only
  if a world lets it change, such as a conquered town slowly taking its new
  owner's culture; [Section 18](open-decisions.md#18-living-sandbox-worlds)
  keeps that question open.
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
  - "Nearest" and every route a party takes are measured on the road graph by
    total authored travel time. Ties break by authored road order, so routing
    is deterministic.
  - Parties act in a stable order.
  - Their random choices draw from their own RNG domain, so adding a bandit
    gang does not change the player's critical hits.
- **Spawners.** Locations spawn parties from templates, with authored caps and
  intervals. A destroyed party is removed; its instance ID is never reused
  within the same history.
- **Interception.** Travel along one road is a single step, except when a world
  tick falls during the journey while a hostile party is on the same road. The
  player then stops at the road's midpoint, and any battle there uses that
  road's ground. Arriving at a location where a hostile party stands also
  intercepts. Either way, the player chooses to fight, talk or flee.
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
- the player trades, recruits, wins a mass battle and receives a fief, which in
  the fixture is the top ambition and so the route's completed, non-terminal
  outcome, and play continues;
- a war changes a holding's owner without the player's involvement.

## Non-goals

- a scripting or behaviour language, or per-character AI scripts;
- real-time play, or any gameplay tied to the wall clock;
- terrain, continuous coordinates or free-space pathfinding in the rules (the
  road graph is the map, and routing is shortest travel time on it; clients may
  draw it however they like);
- entities created at runtime other than instances of authored definitions;
- a generic bag of numeric variables;
- importing data from other games.
