"""The proposed M3 combat model: rules and formulas, skills and characters."""
from __future__ import annotations

import math
from dataclasses import dataclass
from enum import Enum
from fractions import Fraction


class Formula(Enum):
    RATIO = "ratio"  # attack × power × attack / (attack + defence): no K, no level
    K_SCALED = "k-scaled"  # attack × power × K / (K + defence), K grows with attacker level
    K_FIXED = "k-fixed"  # the same with K fixed at base_k


class Channel(Enum):
    PHYSICAL = "physical"
    SPECIAL = "special"  # magic, 内力, mana...: each world names it


class Resource(Enum):
    MP = "mp"  # regenerates over encounter time; fully restored by resting
    RAGE = "rage"  # starts at 0 every fight; builds from acting and from damage taken


class Kind(Enum):
    """Monster tiers; their multipliers are content (see content.Tier)."""

    NORMAL = "normal"
    MINION = "minion"
    BOSS = "boss"


@dataclass(frozen=True)
class Skill:
    name: str
    power: int  # percent; a basic attack is 100
    channel: Channel
    cost: int = 0  # flat, exactly as authored: a deeper MP pool means more casts
    resource: Resource = Resource.MP
    time: int = 100  # action cost in percent of a basic attack; delays the next turn
    cross_share: int | None = None  # overrides the world's share, e.g. a 内力-heavy palm
    level: int = 1  # minimum character level to use it; later skills are stronger or cheaper


def round_half_up(value: Fraction) -> int:
    """The one rounding rule for stats and costs: exact, halves round up. Python's round()
    rounds halves to even and Rust's f64::round rounds them up, so neither is used."""
    return math.floor(value + Fraction(1, 2))


# An authored amount: a decimal as written, or an exact fraction from scaling one.
Amount = float | Fraction


def exact(value: Amount) -> Fraction:
    """An authored decimal as the exact fraction it was written as (0.6 is 3/5, not the
    nearest binary float); fractions pass through unchanged."""
    if isinstance(value, (Fraction, int)):
        return Fraction(value)  # exact already; no str() round trip, which limits digits
    return Fraction(str(value))


# Proposed engine validation bounds (ROADMAP.md, "Engine bounds").
STAT_BOUND = 9_999
POWER_BOUNDS = (1, 1_000)

BASIC_ATTACK = Skill("attack", power=100, channel=Channel.PHYSICAL)
SPECIAL_BASIC_ATTACK = Skill("touch", power=100, channel=Channel.SPECIAL)  # e.g. a spirit or a 内力 palm


@dataclass(frozen=True)
class Combatant:
    """A character's combat stats at one level."""

    name: str
    level: int
    hp: int
    mp: int
    patk: int
    pdef: int
    satk: int
    sdef: int
    speed: int
    skills: tuple[Skill, ...]
    basic: Skill = BASIC_ATTACK  # free fallback action; its channel belongs to the character
    growth: float | None = None  # this character's per-level growth; None means the world's
    xp: int = 0  # XP granted when defeated, authored on the combat profile


@dataclass(frozen=True)
class Profile:
    """Level-1 stats. Floats are allowed because monster tiers scale them."""

    name: str
    hp: Amount
    mp: Amount
    patk: Amount
    pdef: Amount
    satk: Amount
    sdef: Amount
    speed: int
    skills: tuple[Skill, ...] = ()
    basic: Skill = BASIC_ATTACK  # free fallback action; its channel belongs to the character
    growth: float | None = None  # overrides the world's per-level growth for this profile
    mp_growth: float | None = None  # overrides `growth` for maximum MP alone
    # XP granted when defeated: `xp` at level 1 plus `xp_per_level` for each level above.
    xp: int = 0
    xp_per_level: int = 0

    def growth_for(self, stat: str) -> float | None:
        """This profile's growth for one stat; None means the world's."""
        return self.mp_growth if stat == "mp" and self.mp_growth is not None else self.growth

    def at_level(self, rules: Rules, level: int, speed: int | None = None) -> Combatant:
        # Sample content generation, not an engine rule: the engine reads authored
        # per-level stats. Speed does not grow.
        def grow(value: Amount) -> int:
            return rules.grow(value, level, self.growth)

        return Combatant(
            name=self.name,
            level=level,
            hp=grow(self.hp),
            mp=rules.grow(self.mp, level, self.growth_for("mp")),
            patk=grow(self.patk),
            pdef=grow(self.pdef),
            satk=grow(self.satk),
            sdef=grow(self.sdef),
            speed=self.speed if speed is None else speed,  # speed does not grow
            skills=tuple(s for s in self.skills if s.level <= level),
            basic=self.basic,
            growth=self.growth,
            xp=self.xp + self.xp_per_level * (level - 1),
        )


@dataclass(frozen=True)
class Rules:
    """World-level combat constants and the formulas that use them."""

    action_cost: int = 100_000
    speed_cap: int = 200
    growth: float = 1.10  # every stat grows 10% per level, players and monsters alike
    formula: Formula = Formula.RATIO
    base_k: int = 100  # only for the K formulas
    cross_share: int = 25  # % of the other channel's stats added to attack and defence
    mp_regen_percent: int = 3  # % of max MP regained per baseline turn of encounter time
    rage_per_action: int = 1
    rage_per_max_hp: int = 20  # rage gained from taking damage equal to max HP

    @property
    def baseline_turn(self) -> int:
        """Encounter time of one basic action at speed 100, scheduled exactly as `delay` does."""
        return self.delay(100)

    def grown(self, value: Amount, level: int, growth: float | None = None) -> Fraction:
        """A level-1 value at `level`, exactly, before rounding."""
        rate = exact(self.growth if growth is None else growth)
        return exact(value) * rate ** (level - 1)

    def grow(self, value: Amount, level: int, growth: float | None = None) -> int:
        """A level-1 value at `level`, generated exactly and rounded half up."""
        return round_half_up(self.grown(value, level, growth))

    def k_for(self, level: int) -> int:
        return self.base_k if self.formula is Formula.K_FIXED else self.grow(self.base_k, level)

    def combined(self, character: Combatant, skill: Skill, defence: bool = False) -> int:
        """The skill's matching attack (or defence) plus the other channel's share, scaled by 100."""
        share = self.cross_share if skill.cross_share is None else skill.cross_share
        if skill.channel is Channel.PHYSICAL:
            main, other = (character.pdef, character.sdef) if defence else (character.patk, character.satk)
        else:
            main, other = (character.sdef, character.pdef) if defence else (character.satk, character.patk)
        return 100 * main + share * other

    def damage(self, attacker: Combatant, defender: Combatant, skill: Skill) -> int:
        """Matching stats dominate; the other channel adds `cross_share` percent.
        Everything is kept scaled by 100 so the result rounds once, minimum 1. A skill needs a
        positive combined attack; content without one is invalid, as validation reports."""
        a = self.combined(attacker, skill)
        d = self.combined(defender, skill, defence=True)
        if a <= 0:
            raise ValueError(f"{attacker.name} has no attack for {skill.name}")
        if self.formula is Formula.RATIO:
            return max(1, (a * skill.power * a) // (100 * 100 * (a + d)))
        k = self.k_for(attacker.level)
        return max(1, (a * skill.power * k) // (100 * (100 * k + d)))

    def delay(self, speed: int, time: int = 100) -> int:
        """Recovery before the actor's next turn; `time` is the action's cost in percent.
        Effective speed is clamped to [1, speed_cap]; the delay is rounded up once, minimum 1."""
        speed = max(1, min(speed, self.speed_cap))
        return max(1, -(-self.action_cost * time // (100 * speed)))
