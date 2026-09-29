"""Tuned values: builds, skills and sample opponents. Swap these to prototype."""

from __future__ import annotations

from dataclasses import dataclass, field, replace

from .model import SPECIAL_BASIC_ATTACK, Channel, Kind, Profile, Resource, Skill, exact


@dataclass(frozen=True)
class Tier:
    hp: float  # multiplier on the opponent's HP
    attack: float  # multiplier on the opponent's physical and special attack


@dataclass(frozen=True)
class Content:
    warrior: Profile  # the sustain build: rage skills
    mage: Profile  # the burst build: MP skills
    # Sample opponents. They are ordinary characters with different numbers, not
    # types; every target runs against each so no build is judged on one matchup.
    monsters: tuple[Profile, ...]
    # The world's authored level table, in the engine's format: the cumulative XP needed to
    # reach each level, starting at 0 for level 1 (100 × L to advance from level L). It ends
    # at level 35, where the largest sample stat (a boss's HP) stays within the proposed
    # 9,999 bound at 10% growth per level. Opponents author their own XP on their profiles.
    level_table: tuple[int, ...] = tuple(50 * (level - 1) * level for level in range(1, 36))
    # Normal, minion and boss are the same character scaled: a sim shortcut, not an engine concept.
    tiers: dict[Kind, Tier] = field(
        default_factory=lambda: {
            Kind.NORMAL: Tier(1.0, 1.0),
            Kind.MINION: Tier(0.5, 0.6),
            Kind.BOSS: Tier(4.0, 1.3),
        }
    )

    @property
    def builds(self) -> tuple[Profile, ...]:
        return self.warrior, self.mage

    @property
    def max_level(self) -> int:
        """The highest level the level table reaches."""
        return len(self.level_table)

    def scaled(self, opponent: Profile, kind: Kind) -> Profile:
        tier = self.tiers[kind]
        # Exact decimal arithmetic, so a half-integer result rounds half up as documented.
        return replace(
            opponent,
            hp=exact(opponent.hp) * exact(tier.hp),
            patk=exact(opponent.patk) * exact(tier.attack),
            satk=exact(opponent.satk) * exact(tier.attack),
        )


# Each later skill trades up: more power for the same cost, so damage per MP or rage rises.
SPARK = Skill("spark", power=80, channel=Channel.SPECIAL)  # free, so an empty MP pool is not helpless
BOLT = Skill("bolt", power=170, channel=Channel.SPECIAL, cost=12)
FIREBALL = Skill("fireball", power=210, channel=Channel.SPECIAL, cost=12, level=10)
STARFALL = Skill("starfall", power=250, channel=Channel.SPECIAL, cost=12, level=20)
RAGE_STRIKE = Skill("rage strike", power=150, channel=Channel.PHYSICAL, cost=5, resource=Resource.RAGE)
CLEAVE = Skill("cleave", power=185, channel=Channel.PHYSICAL, cost=5, resource=Resource.RAGE, level=10)
EXECUTE = Skill("execute", power=220, channel=Channel.PHYSICAL, cost=5, resource=Resource.RAGE, level=20)
# Opponents follow the same rules: level-gated skills keep them in step with players.
REND = Skill("rend", power=130, channel=Channel.PHYSICAL, cost=5, resource=Resource.RAGE)
MAUL = Skill("maul", power=155, channel=Channel.PHYSICAL, cost=5, resource=Resource.RAGE, level=10)
SAVAGE = Skill("savage", power=180, channel=Channel.PHYSICAL, cost=5, resource=Resource.RAGE, level=20)
HEX = Skill("hex", power=130, channel=Channel.SPECIAL, cost=5, resource=Resource.RAGE)
CURSE = Skill("curse", power=155, channel=Channel.SPECIAL, cost=5, resource=Resource.RAGE, level=10)
WITHER = Skill("wither", power=180, channel=Channel.SPECIAL, cost=5, resource=Resource.RAGE, level=20)

DEFAULT = Content(
    warrior=Profile(
        "warrior", hp=200, mp=0, patk=24, pdef=15, satk=5, sdef=10, speed=100, skills=(RAGE_STRIKE, CLEAVE, EXECUTE)
    ),
    mage=Profile(
        "mage",
        hp=170,
        mp=75,
        patk=8,
        pdef=10,
        satk=20,
        sdef=15,
        speed=100,
        mp_growth=1.0,
        skills=(SPARK, BOLT, FIREBALL, STARFALL),
    ),
    monsters=(
        Profile(
            "beast",
            hp=80,
            mp=0,
            patk=16,
            pdef=10,
            satk=0,
            sdef=10,
            speed=110,
            skills=(REND, MAUL, SAVAGE),
            xp=20,
            xp_per_level=15,
        ),
        Profile(
            "spirit",
            hp=80,
            mp=0,
            patk=0,
            pdef=10,
            satk=16,
            sdef=10,
            speed=110,
            skills=(HEX, CURSE, WITHER),
            basic=SPECIAL_BASIC_ATTACK,
            xp=20,
            xp_per_level=15,
        ),
    ),
)
