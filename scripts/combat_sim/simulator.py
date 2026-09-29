"""Experiments built from encounters: outcomes, rest cycles, bosses and grinding."""
from __future__ import annotations

from dataclasses import dataclass

from .content import DEFAULT, Content
from .encounter import Encounter, FightResult
from .model import Combatant, Kind, Profile, Rules


U64_MAX = 2**64 - 1  # the engine stores XP as u64 and adds it with overflow checks
KILL_LIMIT = 100_000  # grinding simulates each fight; stop there rather than run for hours


@dataclass(frozen=True)
class XpRules:
    """The proposed XP rules. The amounts themselves are authored content (`Content`)."""

    decay: bool = True  # scale XP by level difference; nothing from monsters 5+ levels below
    # Whether levelling up restores HP and MP fully, as the Format 1 engine does. The
    # roadmap leaves this open for M3b; grinding rest counts depend on it.
    level_up_restores: bool = True

    def for_kill(self, authored: int, player_level: int, monster_level: int) -> int:
        """The opponent's authored XP, ±10% per level of difference (capped at ±4), rounded
        down: a scaled reward never exceeds its exact value."""
        if not self.decay:
            return authored
        diff = monster_level - player_level
        return 0 if diff <= -5 else authored * (10 + max(-4, min(4, diff))) // 10


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
    limited: bool = False  # stopped at KILL_LIMIT rather than by the rules

    def __str__(self) -> str:
        note = f" (stopped at the {KILL_LIMIT:,}-kill simulation limit)" if self.limited else ""
        return f"L{self.level}, {self.kills} kills, {self.rests} rests{note}"


def hp_lost_percent(player: Combatant, hp: int) -> int:
    """Rounded down from the HP actually lost, so a survivor never shows 100%."""
    return 100 * (player.hp - hp) // player.hp


class Simulator:
    """Experiments against one opponent, `foe`; `against` switches to another."""

    def __init__(self, rules: Rules = Rules(), content: Content = DEFAULT, foe: Profile | None = None) -> None:
        self.rules = rules
        self.content = content
        self._foe = foe

    @property
    def foe(self) -> Profile:
        """The opponent experiments run against: the chosen one, else the first sample."""
        if self._foe is not None:
            return self._foe
        if not self.content.monsters:
            raise ValueError("content has no sample opponents")
        return self.content.monsters[0]

    def against(self, foe: Profile) -> Simulator:
        return Simulator(self.rules, self.content, foe)

    def player(self, build: Profile, level: int, speed: int | None = None) -> Combatant:
        return build.at_level(self.rules, level, speed)

    def monster(self, level: int, kind: Kind = Kind.NORMAL) -> Combatant:
        return self.content.scaled(self.foe, kind).at_level(self.rules, level)

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
        return next((p for p in range(1, self.content.max_level + 1) if self.fight(self.player(build, p), [boss]).won), None)

    def grind(self, build: Profile, target: int, xp: XpRules = XpRules(),
              farm: int | None = None) -> GrindResult:
        """Grind from level 1 to `target`. Fights the highest monster costing <= 35% HP
        (or always `farm`) and rests when the next fight would be lost."""
        table = self.content.level_table  # cumulative: table[L - 1] is the total XP for level L
        target = min(target, self.content.max_level)  # grinding stops at the authored maximum
        level, total, kills, rests = 1, 0, 0, 0
        hp: int | None = None
        mp: int | None = None
        while level < target:
            player = self.player(build, level)
            if farm is not None:
                if not 1 <= farm <= self.content.max_level:
                    raise ValueError(f"farm level {farm} is outside the table's levels 1-{self.content.max_level}")
                monster_level = farm
            else:
                safe = [m for m in range(1, self.content.max_level + 1)
                        if xp.for_kill(self.monster(m).xp, level, m) > 0
                        and (o := self.outcome(build, level, [self.monster(m)])) and o.hp_lost <= 35]
                if not safe:
                    break
                monster_level = max(safe)
            gain = xp.for_kill(self.monster(monster_level).xp, level, monster_level)
            if gain <= 0:
                break
            if xp.level_up_restores or hp is None or mp is None:
                hp, mp = player.hp, player.mp
            while total < table[level]:
                result = self.fight(player, [self.monster(monster_level)], hp, mp)
                if not result.won:
                    if (hp, mp) == (player.hp, player.mp):  # lost at full: resting cannot help
                        return GrindResult(level, kills, rests)
                    rests += 1
                    result = self.fight(player, [self.monster(monster_level)])
                    if not result.won:  # unbeatable even from full health: grinding stops here
                        return GrindResult(level, kills, rests)
                hp, mp = result.hp, result.mp
                if total + gain > U64_MAX:  # the engine's checked addition refuses this reward
                    return GrindResult(level, kills, rests)
                if kills >= KILL_LIMIT:
                    return GrindResult(level, kills, rests, limited=True)
                kills += 1
                total += gain
            level += 1
        return GrindResult(level, kills, rests)
