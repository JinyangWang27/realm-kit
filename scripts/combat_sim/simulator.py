"""Experiments built from encounters: outcomes, rest cycles, bosses and grinding."""
from __future__ import annotations

from dataclasses import dataclass

from .content import DEFAULT, Content
from .encounter import Encounter, FightResult
from .model import Combatant, Kind, Profile, Rules


@dataclass(frozen=True)
class XpRules:
    decay: bool = True  # scale XP by level difference; nothing from monsters 5+ levels below

    @staticmethod
    def to_next(level: int) -> int:
        return 100 * level

    def for_kill(self, player_level: int, monster_level: int) -> int:
        """Base 15 per monster level + 5, then ±10% per level of difference (capped at ±4)."""
        base = 15 * monster_level + 5
        if not self.decay:
            return base
        diff = monster_level - player_level
        return 0 if diff <= -5 else base * (10 + max(-4, min(4, diff))) // 10


@dataclass(frozen=True)
class Outcome:
    actions: int
    hp_lost: int  # percent of maximum HP

    def __str__(self) -> str:
        return f"{self.actions:>2}a/{self.hp_lost:>3}%"


@dataclass(frozen=True)
class GrindResult:
    level: int
    kills: int
    rests: int

    def __str__(self) -> str:
        return f"L{self.level}, {self.kills} kills, {self.rests} rests"


def hp_lost_percent(player: Combatant, hp: int) -> int:
    return 100 - 100 * hp // player.hp


class Simulator:
    def __init__(self, rules: Rules = Rules(), content: Content = DEFAULT) -> None:
        self.rules = rules
        self.content = content

    def player(self, build: Profile, level: int, speed: int | None = None) -> Combatant:
        return build.at_level(self.rules, level, speed)

    def monster(self, level: int, kind: Kind = Kind.NORMAL) -> Combatant:
        return self.content.monster_of(kind).at_level(self.rules, level)

    def fight(self, player: Combatant, enemies: list[Combatant],
              hp: int | None = None, mp: int | None = None) -> FightResult:
        return Encounter(self.rules, player, enemies, hp, mp).run()

    def outcome(self, build: Profile, level: int, enemies: list[Combatant]) -> Outcome | None:
        """Actions and HP lost on a win from full health; None on a loss."""
        player = self.player(build, level)
        result = self.fight(player, enemies)
        return Outcome(result.actions, hp_lost_percent(player, result.hp)) if result.won else None

    def fights_per_rest(self, build: Profile, level: int, monster_level: int) -> int:
        """Consecutive same-kind fights won from full HP/MP before a loss (at most 50)."""
        player = self.player(build, level)
        hp, mp = player.hp, player.mp
        for won_so_far in range(50):
            result = self.fight(player, [self.monster(monster_level)], hp, mp)
            if not result.won:
                return won_so_far
            hp, mp = result.hp, result.mp
        return 50

    def boss_level_needed(self, build: Profile, boss_level: int) -> int | None:
        boss = self.monster(boss_level, Kind.BOSS)
        return next((p for p in range(1, 60) if self.fight(self.player(build, p), [boss]).won), None)

    def grind(self, build: Profile, target: int, xp: XpRules = XpRules(),
              farm: int | None = None) -> GrindResult:
        """Grind from level 1 to `target`. Fights the highest monster costing <= 35% HP
        (or always `farm`) and rests when the next fight would be lost."""
        level, earned, kills, rests = 1, 0, 0, 0
        while level < target:
            player = self.player(build, level)
            if farm is not None:
                monster_level = farm
            else:
                safe = [m for m in range(1, 60)
                        if (o := self.outcome(build, level, [self.monster(m)])) and o.hp_lost <= 35]
                if not safe:
                    break
                monster_level = max(safe)
            gain = xp.for_kill(level, monster_level)
            if gain == 0:
                break
            hp, mp = player.hp, player.mp
            while earned < xp.to_next(level):
                result = self.fight(player, [self.monster(monster_level)], hp, mp)
                if not result.won:
                    rests += 1
                    result = self.fight(player, [self.monster(monster_level)])
                hp, mp = result.hp, result.mp
                kills += 1
                earned += gain
            earned -= xp.to_next(level)
            level += 1
        return GrindResult(level, kills, rests)
