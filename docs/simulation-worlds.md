# Non-spatial simulation worlds

Status: design direction. This document describes a future architecture pressure
test; it does not change the current Format 1 contract or the active milestone
scope.

RealmKit started with a spatial text-RPG slice, but its long-term model should be
able to support authored simulation games whose interesting state is social,
institutional, economic or temporal rather than geographic. A Tenure+-style
academic-career simulation is a useful reference scenario: the player advances
through terms or years, manages resources and relationships, takes actions such
as submitting work or applying for funding, and experiences delayed consequences
from earlier choices.

The goal is not to reproduce one game. The goal is to ensure RealmKit's
abstractions do not accidentally make directional map traversal, combat, or
inventory universal.

[Living sandbox worlds](sandbox-worlds.md) are a sibling architecture test. They
keep the location graph but add a world that moves without the player: world
agents, economy, standing and holdings on recurring schedules. Both tests share
the scheduling and randomness rules below.

## Architecture test

A mature RealmKit should be able to express a non-spatial simulation using the
same core guarantees as a conventional adventure:

- AI may author content before play, but runtime remains AI-free and offline.
- The engine owns mutable state and deterministic rule resolution.
- Player choices enter through structured commands.
- Rules emit structured events and explicit state changes.
- Randomness, when present, uses explicit versioned RNG state and chooses only
  among authored possibilities.
- Save/load captures every state component needed to reproduce subsequent play.
- Clients render the same engine state without owning game rules.

For this class of world, the dominant loop is conceptually:

```text
state
  -> available action
  -> preconditions and costs
  -> deterministic/stochastic resolution
  -> effects and emitted events
  -> scheduled or newly eligible consequences
  -> new state
```

This is a conceptual execution pattern, not a proposal for one untyped universal
`Action` object. RealmKit should continue to prefer small typed capability
models, structured commands/effects and explicit cross-capability references over
a stringly typed scripting language, generic ECS or plugin registry.

## Reference scenario: academic-career simulation

An academic-career world could track state such as:

- calendar position: year, term and deadlines;
- funding or other budget-like resources;
- stress, morale or similar route-owned condition;
- reputation or institutional standing;
- research/project progress;
- relationships with students, collaborators and administrators;
- authored career checkpoints and outcomes.

Representative actions might include:

- submit a paper;
- apply for a grant;
- recruit or supervise a student;
- invest time or money in a project;
- attend, decline or prioritize an institutional obligation;
- rest or recover;
- respond to a relationship or career event.

The interesting gameplay comes from systems interacting over time. A student
relationship, project state and stress level established several turns earlier
may together make a later event eligible. The player should be able to understand
that later consequence as the result of accumulated state rather than an
unrelated random story fragment.

For example, conceptually:

```yaml
# Illustrative only; not a serialization proposal.
student:
  motivation: 34
  relationship: 61
  project_progress: 75
  stress: 88

event:
  id: student_burnout_warning
  when:
    student.stress: "> 80"
    student.project_progress: "> 60"
  choices:
    push_harder:
      effects:
        student.project_progress: +20
        student.relationship: -15
        student.stress: +10
    give_break:
      effects:
        student.project_progress: -5
        student.relationship: +10
        student.stress: -25
```

A later authored event may depend on the resulting relationship and stress state.
The engine does not need to know what a "student" or "paper" means unless a
representative capability requires typed semantics for those concepts.

## Navigation must not become a hidden universal

Current Format 1 is spatial: worlds contain locations and player movement is an
important part of the demo. That is an implementation fact, not necessarily a
permanent requirement for every RealmKit game.

A future non-spatial fixture should be allowed to prove that navigation can be
absent rather than represented by fake rooms such as "office", "lab" and
"meeting" solely to satisfy the engine. A client for such a world might present
a dashboard, inbox, calendar, institution page or other domain-specific view
instead of a compass and map.

Do not make location/navigation optional speculatively. Make that change only
when a representative world needs it, then remove the spatial assumption at the
smallest coherent boundary. Until then, current Format 1 requirements remain
unchanged.

## Capability composition

A simulation world should be assembled from the same presence-driven capability
model used elsewhere. An academic-career scenario might compose:

```text
world time/calendar
+ economy/funding
+ relationships
+ reputation
+ collection/scholarship
+ authored events and progression
+ one small domain-specific capability, if repeated rules justify it
```

It may have no combat, equipment, inventory or spatial navigation.

The capability test remains unchanged: recurring mutable state, reusable rules
and meaningful player-facing consequences must justify a capability. One grant
application can be an authored event. Repeated grant rounds with budgets,
eligibility, scoring, deadlines and downstream effects may justify a dedicated
typed mechanic.

Avoid a generic bag of numeric stats simply because simulation games expose many
meters. State should have an authoritative owner, validation rules and clear
semantics. Cross-capability effects should be explicit.

## Events, scheduling and delayed consequences

This genre puts more pressure on scheduling than the first adventure fixture.

RealmKit should distinguish:

- immediate command resolution;
- story-phase progression;
- optional in-world calendar time;
- scheduled events at authored world times;
- condition-triggered event eligibility;
- encounter-local timelines, when a capability such as combat needs one.

Advancing a term or performing a time-consuming action may cross several scheduled
events. They should resolve in chronological order with stable authored tie
breaking, consistent with the roadmap's World Time direction.

Condition-triggered events should not be polled from wall-clock time. Eligibility
is evaluated at explicit deterministic gameplay transitions. If several events
become eligible together, ordering must be authored or otherwise stable and
documented.

## Randomness

Simulation games often need uncertainty: acceptance/rejection, rare opportunities,
variable outcomes or event selection. RealmKit should preserve the existing
determinism principle:

- randomness comes from explicit saved RNG state;
- the same package, starting state, seed and command sequence reproduce the same
  result;
- RNG selects among authored outcomes rather than generating new prose;
- unrelated random domains should be separated when draw coupling would make
  small content changes unexpectedly alter other systems.

An action such as `submit_paper` may therefore resolve to accepted, revision or
rejected outcomes using authored weights and explicit RNG state, while remaining
fully replayable.

## Authoring and AI boundary

This direction strengthens rather than weakens RealmKit's build-time AI boundary.

A world-generation workflow may use AI to propose or author:

- entities and relationships;
- action definitions and parameters;
- event pools and condition/effect wiring;
- schedules and deadlines;
- outcome text;
- progression arcs and endings;
- domain-specific capability content.

The exported world package contains the finished playable possibilities. Runtime
does not ask a model what happens next.

This preserves offline play, reproducibility, validation, regression testing and
the ability to inspect why a state transition occurred.

## Presentation

The engine should not assume that every client is an adventure transcript.

A simulation-oriented client may present:

- current term/year and upcoming deadlines;
- resource and reputation summaries;
- people or organization panels;
- projects and applications;
- an event/inbox feed;
- available actions;
- history explaining prior state changes.

These are projections of engine/package data. The client must not independently
decide eligibility, probabilities or effects.

The same world could still be playable through a simple terminal client, but a
richer web/mobile client may make the domain legible without changing game rules.

## Reference-fixture strategy

Do not broaden the engine before the existing roadmap earns the abstraction.

A useful sequence is:

1. finish the current adventure-oriented milestones and prove optional non-combat
   capabilities with a narrative fixture;
2. add a small non-spatial simulation fixture;
3. use that fixture to identify remaining accidental assumptions about locations,
   movement, inventory or combat;
4. extract only the minimum typed abstractions required by both styles;
5. test deterministic replay and save/load across the simulation's delayed events.

The non-spatial fixture succeeds as an architecture test when it can be authored
and played without dummy movement, dummy combat state or runtime-generated prose,
while preserving RealmKit's structured command/event and deterministic-state
model.

## Non-goals

This direction does not imply:

- a universal simulation DSL;
- executable formula strings in content;
- an ECS;
- runtime-loaded plugins;
- runtime LLM calls;
- mandatory calendars, relationships or economy;
- making every numeric meter part of core state;
- implementing a Tenure+ clone in the current milestone.

The intended outcome is narrower: RealmKit remains a small deterministic engine,
but its core abstractions are general enough that a stateful career/life/institution
simulation can become one world composition rather than a separate engine.
