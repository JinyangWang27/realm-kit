"""One encounter on the paused initiative timeline."""
from __future__ import annotations

from dataclasses import dataclass

from .model import Combatant, Resource, Rules, Skill


@dataclass
class Fighter:
    """A combatant's mutable state inside one encounter."""

    combatant: Combatant
    side: int  # 0 = the player's side
    order: int  # position in the encounter; breaks ties
    hp: int
    mp: int
    next_time: int
    rage: int = 0
    mp_remainder: int = 0  # regeneration carried between ticks so rounding loses nothing
    rage_remainder: int = 0  # damage-rage carried between hits, for the same reason

    @property
    def alive(self) -> bool:
        return self.hp > 0

    def take_hit(self, rules: Rules, damage: int) -> None:
        """Rage proportional to damage taken: `rage_per_max_hp` for a full HP bar, cumulatively."""
        self.rage_remainder += rules.rage_per_max_hp * damage
        gained, self.rage_remainder = divmod(self.rage_remainder, self.combatant.hp)
        self.rage += gained

    def regenerate(self, rules: Rules, elapsed: int) -> None:
        """MP regained over encounter time; speed does not change the rate. Time spent at
        full MP banks nothing, so the carried remainder is dropped at the cap."""
        self.mp_remainder += self.combatant.mp * rules.mp_regen_percent * elapsed
        gained, self.mp_remainder = divmod(self.mp_remainder, 100 * rules.baseline_turn)
        if self.mp + gained >= self.combatant.mp:
            self.mp, self.mp_remainder = self.combatant.mp, 0
        else:
            self.mp += gained

    def can_afford(self, skill: Skill, cost: int) -> bool:
        return (self.mp if skill.resource is Resource.MP else self.rage) >= cost

    def choose_skill(self, rules: Rules) -> Skill:
        """The strongest affordable skill, falling back to a basic attack. Equal power prefers
        the cheaper skill, then the later tier, so a cheaper later tier is actually used."""
        costs = {s.name: rules.skill_cost(s, self.combatant) for s in self.combatant.skills}
        affordable = [s for s in self.combatant.skills if self.can_afford(s, costs[s.name])]
        return max(affordable, key=lambda s: (s.power, -costs[s.name], s.level), default=self.combatant.basic)

    def pay(self, skill: Skill, cost: int) -> None:
        if skill.resource is Resource.MP:
            self.mp -= cost
        else:
            self.rage -= cost


@dataclass(frozen=True)
class FightResult:
    won: bool
    actions: int  # actions the player took
    hp: int  # player HP left
    mp: int  # player MP left


class Encounter:
    """One player against enemies. Ties resolve by (time, side, order); everyone
    attacks the first living opponent with their strongest affordable skill."""

    def __init__(self, rules: Rules, player: Combatant, enemies: list[Combatant],
                 hp: int | None = None, mp: int | None = None) -> None:
        self.rules = rules
        self.fighters = [
            Fighter(c, side=0 if i == 0 else 1, order=i, hp=c.hp, mp=c.mp, next_time=rules.delay(c.speed))
            for i, c in enumerate([player, *enemies])
        ]
        self.player = self.fighters[0]
        if hp is not None:
            self.player.hp = hp
        if mp is not None:
            self.player.mp = mp
        self.player_actions = 0
        self.now = 0

    def run(self) -> FightResult:
        while self.player.alive and any(f.alive and f.side != 0 for f in self.fighters):
            actor = self._next_actor()
            for fighter in self.fighters:
                fighter.regenerate(self.rules, actor.next_time - self.now)
            self.now = actor.next_time
            self._act(actor)
        return FightResult(self.player.alive, self.player_actions, self.player.hp, self.player.mp)

    def _next_actor(self) -> Fighter:
        return min((f for f in self.fighters if f.alive), key=lambda f: (f.next_time, f.side, f.order))

    def _act(self, actor: Fighter) -> None:
        target = next(f for f in self.fighters if f.alive and f.side != actor.side)
        attacker, defender = actor.combatant, target.combatant
        skill = actor.choose_skill(self.rules)
        actor.pay(skill, self.rules.skill_cost(skill, attacker))
        dealt = self.rules.damage(attacker, defender, skill)
        target.hp -= min(dealt, target.hp)
        actor.rage += self.rules.rage_per_action
        target.take_hit(self.rules, dealt)
        if actor is self.player:
            self.player_actions += 1
        actor.next_time += self.rules.delay(attacker.speed, skill.time)
