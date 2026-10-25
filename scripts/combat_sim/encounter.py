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
    uses_mp: bool = True
    uses_rage: bool = True

    @property
    def alive(self) -> bool:
        return self.hp > 0

    @property
    def mp_max(self) -> int:
        return self.combatant.mp if self.uses_mp else 0

    def gain_rage(self, rules: Rules, points: int) -> None:
        """Rage up to the world's cap; at a cap, nothing is banked toward the next point."""
        self.rage += points
        if rules.rage_cap is not None and self.rage >= rules.rage_cap:
            self.rage, self.rage_remainder = rules.rage_cap, 0

    def act_rage(self, rules: Rules) -> None:
        if self.uses_rage:
            self.gain_rage(rules, rules.rage_per_action)

    def take_hit(self, rules: Rules, damage: int) -> None:
        """Rage proportional to damage taken: `rage_per_max_hp` for a full HP bar, cumulatively."""
        if not self.uses_rage:
            return
        self.rage_remainder += rules.rage_per_max_hp * damage
        gained, self.rage_remainder = divmod(self.rage_remainder, self.combatant.hp)
        self.gain_rage(rules, gained)

    def regenerate(self, rules: Rules, elapsed: int) -> None:
        """MP regained over encounter time; speed does not change the rate. Time spent at
        full MP banks nothing, so the carried remainder is dropped at the cap."""
        self.mp_remainder += self.mp_max * rules.mp_regen_percent * elapsed
        gained, self.mp_remainder = divmod(self.mp_remainder, 100 * rules.baseline_turn)
        if self.mp + gained >= self.mp_max:
            self.mp, self.mp_remainder = self.mp_max, 0
        else:
            self.mp += gained

    def can_afford(self, skill: Skill) -> bool:
        """Every cost covered; a resource the fighter does not use pays for nothing."""
        mp, rage = skill.cost_in(Resource.MP), skill.cost_in(Resource.RAGE)
        return (mp == 0 or (self.uses_mp and self.mp >= mp)) and (rage == 0 or (self.uses_rage and self.rage >= rage))

    def choose_skill(self) -> Skill:
        """The strongest affordable skill, falling back to a basic attack. Equal power prefers
        the cheaper skill, then the later tier, so a cheaper later tier is actually used."""
        affordable = [s for s in self.combatant.skills if self.can_afford(s)]
        best = max(affordable, key=lambda s: (s.power, -s.total_cost, s.level), default=None)
        return self.combatant.basic if best is None else best

    def pay(self, skill: Skill) -> None:
        self.mp -= skill.cost_in(Resource.MP)
        self.rage -= skill.cost_in(Resource.RAGE)


@dataclass(frozen=True)
class FightResult:
    won: bool
    actions: int  # actions the player took
    hp: int  # player HP left
    mp: int  # player MP left


class Encounter:
    """One player against enemies. Ties resolve by (time, side, order); everyone
    attacks the first living opponent with their strongest affordable skill."""

    def __init__(
        self, rules: Rules, player: Combatant, enemies: list[Combatant], hp: int | None = None, mp: int | None = None
    ) -> None:
        self.rules = rules
        self.fighters = [
            Fighter(
                c,
                side=0 if i == 0 else 1,
                order=i,
                hp=c.hp,
                mp=c.mp if rules.uses(c).mp else 0,
                next_time=rules.delay(c.speed),
                uses_mp=rules.uses(c).mp,
                uses_rage=rules.uses(c).rage,
            )
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
        skill = actor.choose_skill()
        actor.pay(skill)
        dealt = self.rules.damage(attacker, defender, skill)
        target.hp -= min(dealt, target.hp)
        actor.act_rage(self.rules)
        target.take_hit(self.rules, dealt)
        if actor is self.player:
            self.player_actions += 1
        actor.next_time += self.rules.delay(attacker.speed, skill.time)
