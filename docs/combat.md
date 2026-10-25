# Combat architecture

RealmKit is a deterministic RPG engine, not a turn-based combat engine. A
fighting character's progression and rules hold however a fight is run; the
paused initiative timeline is one way to run one, and a fixed-step real-time
duel is another. This page says which parts belong where, how MP and Rage
work, how real-time duels run and where their authority ends, and what is not
supported yet. Package fields are in
[world-format.md](world-format.md#combat-block-numeric-rules-and-templates).

## Three layers

| Layer | What it owns | Where |
| --- | --- | --- |
| **Character core** (universal) | Levels and XP, stat points, effective stats, gear and techniques, HP, MP and Rage, skill costs, damage and crits, Rage from acting and from damage, resource regeneration and decay, validated vitals | `realmkit-engine/src/character/`, `gear.rs`, `techniques.rs`, `formulas.rs`; spec `stats.rs`, most of `combat.rs` |
| **Turn-based encounters** (optional) | The initiative timeline, action delays, whose turn it is, opponents' policy, attack, skill, flee and item turns, how encounters start and end, and timeline MP regeneration | `realmkit-engine/src/encounter/`; spec `Timeline` |
| **Real-time duels** (optional) | A one-on-one fight's fixed-step clock, attack phases, guard and retreat timing, which intents are legal at each step, the opponent's policy, host-reported contact, and duel-time MP regeneration | `realmkit-engine/src/realtime/`; spec `Realtime` |
| **Mass battles** (strategic, separate) | Troop stacks, frontage, orders, morale, pursuit and autoresolve | `realmkit-engine/src/battle.rs`; spec `troops.rs` |

The core knows nothing of initiative, turns, ticks, frames or a host engine.
Either fight layer decides *when* someone acts and asks the core *what* the
action does: whether the skill can be paid for, what it costs, the damage, and
the Rage it earns. World time asks the core what elapsed minutes do. A mass
battle uses the player's own HP from the core, and troops' fixed stats; it is
not personal combat and is not a real-time system.

Field meanings are the core's, with each layer reading them in its own terms:

- `speed` is how quickly a character acts. The timeline turns it into the delay
  between turns; a duel divides each attack phase's length by it.
- A skill's `time` is the action's length relative to a basic attack. The
  timeline delays the user's next turn by it; a duel scales each of the
  attack's phases by it, as a worn weapon's `basic_time` does a basic attack's.
- `combat.resources.mp_regen_percent` belongs to the timeline alone: per
  baseline turn of encounter time. `combat.realtime.mp_regen_percent` is per
  second of duel time. World-time regeneration is `mp_recovery`.

## One owner per character's vitals

`Vitals { hp, mp, rage, rage_remainder }` are a character's current HP and
resources. Maxima are derived from effective stats and never saved.

- The player's vitals live in `CombatState::vitals`, whatever the stance.
  Exploring, an encounter, a duel and a battle all read and change that one copy.
- An opponent's vitals live in its `Participant` (or duel `Fighter`) for the
  length of the fight: opponents are whole again next time, as before. The
  player's participant or fighter holds `None` and the save check rejects
  anything else.
- Troops in a battle are fungible counts in stacks, never characters with
  vitals.

`Stance` (`exploring`, `fighting`, `dueling`, `battle`) only records how the player is
fighting now. A future named companion would get its own `Vitals` beside the
player's in the same way, not a copy inside a stance.

## MP and Rage

Both resources exist for every fighter, and each character uses MP, Rage,
both or neither. A resource a character does not use stays at 0, gains and
regains nothing, pays for nothing, and is shown as absent, which is not the
same as a resource in use at 0 (`Engine::player_pools`, `Engine::pools_of`).
`player_resources` and a combat profile's `resources` choose explicitly;
without them a character uses MP when its maximum MP is above 0, and Rage when
the world builds Rage at all, which is exactly what worlds did before the
choice existed.

**MP is a reserve.** It starts full, skills spend it, and resting and levelling
up refill it. It is kept between fights. Time refills it on two clocks that
never overlap: `mp_regen_percent` on the encounter timeline, and
`mp_recovery` (a percentage of maximum MP at each multiple of `every` world
minutes). Attacking never earns MP.

**Rage is momentum.** It starts at 0 and builds from fighting:
`rage_per_action` for each attack or skill the character resolves, credited
after the action, and `rage_per_max_hp` for taking damage equal to maximum HP,
proportionally, with the fraction carried. Time never builds it. `rage_cap`
bounds it; at the cap nothing is banked toward the next point. By default Rage
belongs to one fight and clears when the fight ends, as it always has. With
`rage_carries` it stays with the character and `rage_decay` drains points at
fixed world minutes.

A skill costs `cost` from its `resource` (the original form) or `costs` in
both. It is usable only if every cost is covered; MP is checked and spent
first, and each resource spent is reported. A refused skill changes nothing
and draws nothing from the random streams.

A world may present Rage as sword momentum or fighting spirit in its own
words; it is still the same resource, not a third one.

### Clocks

World time, encounter time and duel time are separate clocks and never
regenerate the same MP twice:

- An encounter, duel or battle never advances world time: travel, waiting and
  resting are refused during them.
- Encounter time exists only inside an encounter and duel time only inside a
  duel; the player is never in both. Each one's regeneration progress
  (`Participant::mp_remainder`, `Fighter::mp_remainder`) ends with it.
- World-time recovery and decay happen at fixed multiples of `every`, counted
  from minute 0. Crossing boundaries is additive, so a span split anywhere
  (one long wait, several short ones, a journey interrupted by scheduled
  events) crosses the same boundaries, and nothing needs saving between them.
  Boundaries at a minute apply before anything else scheduled then.

## Real-time duels

A world whose combat block has `realtime` runs one-on-one duels on a
fixed-step clock inside the engine, beside the turn-based layer. It is the
smallest real-time slice that is authoritative and deterministic: one player,
one opponent who fights alone, basic attacks and skills in three phases, a
guard, and a retreat. Package fields are in
[world-format.md](world-format.md#real-time-duels).

### Lifecycle

1. **Start.** `Command::Duel(id)` starts a duel with a fighter here, not
   defeated, whom engaging would bring in alone (a group of one is fine; a
   group of two or more is `NotOneOnOne`). The stance becomes
   `Stance::Dueling(Duel)` and `EncounterStarted` is reported. The player
   brings their own vitals; the opponent's are full and belong to the duel.
   Rage that belongs to one fight starts from 0, as in an encounter.
2. **Play.** During a duel only `Command::Intent(_)` and panels run. Each
   intent applies at the duel's current step; `Intent::Advance(n)` runs the
   clock on.
3. **End.** The opponent falling or yielding is a `Victory`; the player
   yielding is `Yielded`; a completed retreat is `Fled`. The player's death
   leaves the duel open, as it does an encounter, and every later command
   but a panel is refused.
4. **Settle once.** Turn-based encounters and duels end in the same
   `settle` (crate `settle.rs`): the stance returns to `Exploring` first,
   then loot, XP, recorded defeats, defeat objectives, technique shares and
   group flags are granted, all inside the one command that ended the fight.
   With the stance gone, no later command can settle it again.

### The clock

`Duel::step` counts steps of duel time from 0. Only `Advance` moves it,
1 to `ADVANCE_BOUND` (10,000) steps per command; 0 or more is refused, and a
clock that would pass `u64::MAX` is `NumericLimit`. Each step runs in one
fixed order:

1. the clock moves on by one;
2. both fighters regenerate MP: `realtime.mp_regen_percent` of maximum MP per
   `steps_per_second` steps, through `resources::regenerate` with the progress
   kept in `Fighter::mp_remainder`;
3. the player's committed action moves on if it ends at this step, then the
   opponent's; a retreat that ends here ends the duel at once;
4. a ready opponent begins its next attack (its authored policy, below).

An advance stops at the step the duel ends. Because every step does the same
thing whatever command it came in, splitting an advance anywhere (one step
per host frame, or many) gives the same state and events. `steps_per_second`
tells hosts how fast to step to show the duel at its authored pace; the
engine never reads a clock.

**Pause and resume.** The duel clock stands still between commands. A host
pauses by sending no `Advance`: menus, a paused game, a minimised window or a
slow frame never move it. World time is frozen for the whole duel (travel,
waiting and resting are refused), so a duel of any length passes no world
minutes, regenerates nothing on world time and runs no world schedules.

### Actions and phases

A fighter is `Ready`, in an `Attack`, raising or holding a `Guard`, or in a
`Retreat`. Each timed state ends when the clock reaches its `until` step.

- **Attack** (`Intent::Attack`, or `Intent::Skill(id)` for a known, unlocked
  skill): allowed only from `Ready` or `Guard` (it drops the guard). A skill's
  costs are checked and paid at once, MP first, through the core's
  `afford`/`spend`; a refused skill changes nothing. The attack then runs
  `Windup → Active → Recovery → Ready`. Phase lengths are the world's
  `windup`, `active` and `recovery` at speed 100 and a basic action's time,
  scaled to `max(1, ceil(length × time / speed))` steps by the attacker's
  effective speed and the skill's `time` (a worn weapon's `basic_time` for a
  basic attack).
- **Commitment.** During any attack phase the player can start nothing else:
  no other attack, guard or retreat (`Committed`). There is no cancel.
- **Guard** (`Intent::Guard`, only from `Ready`): stops hits from
  `guard_raise` steps later until it is lowered (`Intent::LowerGuard`) or an
  attack drops it. A hit landing on a raised guard deals `guard_percent` of its
  damage, rounded down; one landing while it is still rising is a full hit.
  The guard covers every direction: facing and guard arcs are spatial and not
  modelled yet.
- **Retreat** (`Intent::Retreat`, from `Ready` or `Guard`, refused by a
  `no_flee` group): the player is away after `retreat` steps if still fighting,
  and can still be hit meanwhile.
- **Opponent policy.** The opponent never guards or retreats. Whenever it is
  ready it begins an attack with the strongest skill it can pay for, else a
  basic attack, chosen as in turn-based encounters.

A hit does not interrupt the target's own action in this slice: poise,
stagger and interruption are later work.

### Contact and its authority

Whether a blow physically touched is a spatial question, and the engine has
no geometry. The host reports it with `Intent::Contact(attacker)`: "this
fighter's attack touched the other one now". The engine accepts a report only
when that fighter's attack is in its `Active` phase at the current step and
has not landed yet; anything else is `NoContact` and changes nothing. On an
accepted report the engine decides everything that follows:

- damage through `character::strike` from both sides' effective stats, the
  skill's power and channel, worn gear's modifiers on the player, and the
  target's guard;
- **no random crit and no accuracy roll:** a landed real-time hit deals the
  same damage from the same state, and duels draw nothing from any random
  stream;
- HP loss (stopping at 1 HP in a yielding group), Rage for being hit, Rage
  for acting credited to the attacker only because the attack landed, so
  swinging at the air earns nothing, technique XP for a landed skill, and the
  outcome.

What this bounds: a host can make an attack land at most once, only inside
its active window, and never choose its damage, a kill, loot, XP or victory.
What it does not: the engine cannot tell whether the reported contact was
geometrically real, nor whether a host withheld a contact that happened. A
dishonest or buggy host can therefore make every active window land or none.
This is an explicit trust boundary, not a verified hit. Replays are
deterministic for the same ordered intents including contact reports; they
are not proof that the host's physics produced those reports. Closing the gap
needs engine-side geometry or a validated spatial record (positions, reach
and hit volumes stepped on the same fixed clock), which is open work
(below).

### What a duel reuses

| Rule | From |
| --- | --- |
| HP, MP, Rage | `CombatState::vitals` for the player; full vitals from the profile for the opponent |
| Effective stats, pools | `character::player_stats`, `resources::player_pools`, `profile_pools` |
| Skill affordability and payment | `resources::afford`, `resources::spend` |
| Rage from acting and from damage, cap | `resources::on_action`, `on_damage`, `at_cap` |
| Damage, worn modifiers, HP loss | `strike::damage_of` (with no crit), `strike::take` |
| MP regeneration | `resources::regenerate` on duel time |
| Yielding, `no_flee`, groups | the group rules turn-based encounters use |
| Victory rewards, flags, objectives | `settle::settle`, shared with encounters |

Nothing in a duel is a second copy of the player. It adds no third resource:
no stamina, posture or poise.

### Saves mid-duel

A duel can be saved between any two commands, since every committed command
leaves it at a step boundary and its whole state (step, actions with their
`until` steps, the opponent's vitals, regeneration progress) is in the
snapshot. Loading checks that the rules could have left it: the player and a
lone opponent who is here and undefeated; vitals within maxima; progress
below one point; every committed action ending after now and no later than
its phase allows; a known, unlocked skill; no landed wind-up; a ready
opponent only at step 0; guards and retreats only for the player, and a
retreat only where flight is allowed; the duel still open, or the player
dead. A resumed duel continues exactly as an uninterrupted one would.

### Determinism

For the same package, seed and ordered commands (contact reports included),
duels produce identical events and state, whatever the host's frame rate or
batching. Refused intents change nothing and draw nothing. No wall clock,
frame delta or host physics callback is read.

### Godot and other hosts: what remains

- Spatial authority: positions, facing, reach and hit volumes on the fixed
  step, or a validated spatial record, so contact can be checked rather than
  trusted.
- A host bridge: stepping at `steps_per_second`, sending intents between
  steps, rendering from `Engine::duel()` and the events, interpolating
  between steps for presentation only.
- Opponent intents beyond attacking: guarding, spacing, telegraphed
  patterns.
- Dodges, parries, redirection, interruption and poise, conditional crits,
  light and heavy handling (Gheasin Combat-02 to Combat-04).
- Groups, allies and companions in real time; consumables during a duel.

## Not supported yet

- Verified real-time hit detection: duel contact is host-reported and only
  bounded by the engine (see [Contact and its authority](#contact-and-its-authority)).
- Real-time fights with more than one opponent, allies or companions;
  consumables, gear changes or dodges during a duel; opponents that guard,
  space or retreat; interruption, poise, parries and situational crits.
- Committing elapsed host time in batches outside travel, waiting and
  resting (see [host-driven passage of time](spatial-hosts-and-military-battles.md#host-driven-passage-of-time-future-requirement)).
- Named companions with persistent vitals, and allies in encounters.
- HP regeneration over world time.
- Resource events for recovery and decay: clients read the new vitals.
- Opponents carrying Rage or wounds between encounters; their vitals belong
  to one fight.
- A third fundamental resource.

## Compatibility

Existing packages need no change and keep their rules revision: every new
field is optional and absent by default, and `realtime` is not serialized
when absent. Encounters, damage, RNG draws, progression and battles are
unchanged; every example walkthrough produces byte-identical output and save
files. Turn-based encounters now settle through the same `settle` function as
duels, with the same steps in the same order.

Saves keep format 19, as the [format policy](world-format.md#versions)
allows for a new kind that leaves older content valid: `Stance::Dueling` is a
new stance variant, so every older save parses and loads exactly as before,
and only a world with `realtime` can hold one. A world that gains or loses
`realtime` changes its rules revision, so saves made before are refused, never
reinterpreted. The player's vitals stay in `combat.vitals`; a save in the
earlier layout that kept them in the stance is refused by name, with the
advice to start a new playthrough.
