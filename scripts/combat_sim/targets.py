"""The balance targets as explicit checks; each failure names the broken target."""
from __future__ import annotations

from dataclasses import replace

from .encounter import Encounter
from .model import POWER_BOUNDS, STAT_BOUND, Channel, Combatant, Formula, Kind, Profile, Resource, Rules, Skill
from .simulator import Simulator

class TargetMissed(Exception):
    """A balance target does not hold."""


def require(condition: object, message: str) -> None:
    """Explicit failure rather than `assert`, which `python -O` would strip."""
    if not condition:
        raise TargetMissed(message)


SAME_LEVEL_ACTIONS = (3, 6)  # player actions to beat a same-level ordinary monster
BOSS_LEVEL_GAP = 1  # most levels one build may need beyond another to beat the same boss


def check_formula(rules: Rules) -> None:
    """Worked examples: physical attack 40 and special attack 20 against varied defences."""
    def hit(pdef: int, sdef: int, channel: Channel) -> int:
        a = Combatant("a", 1, 1, 0, patk=40, pdef=0, satk=20, sdef=0, speed=100, skills=())
        d = Combatant("d", 1, 1, 0, patk=0, pdef=pdef, satk=0, sdef=sdef, speed=100, skills=())
        return rules.damage(a, d, Skill("hit", 100, channel))
    if rules.formula is Formula.RATIO and rules.cross_share == 25:
        # physical: attack 40 + 25% of 20 = 45; no defence -> 45; defence 45 halves it
        require(hit(0, 0, Channel.PHYSICAL) == 45 and hit(45, 0, Channel.PHYSICAL) == 22,
                "formula example: combined physical attack 45, halved by defence 45")
        # high 内力 (special defence 80) blocks a physical hit like 20 physical defence
        require(hit(0, 80, Channel.PHYSICAL) == hit(20, 0, Channel.PHYSICAL) == 31,
                "formula example: special defence 80 should block like physical defence 20")
        # special: 20 + 25% of 40 = 30
        require(hit(0, 0, Channel.SPECIAL) == 30,
                "formula example: combined special attack 30")


def check_resource_timing(sim: Simulator) -> None:
    """Action rage is credited after the action resolves, so rage one short of a skill's
    cost cannot pay for it that turn. Runs one real action through the encounter."""
    rager = next((b for b in sim.content.builds
                  if any(s.resource is Resource.RAGE for s in b.skills)), None)
    if rager is None:
        return
    player = sim.player(rager, 1)
    cost = min(s.cost for s in player.skills if s.resource is Resource.RAGE)
    encounter = Encounter(sim.rules, player, [sim.monster(1)])
    encounter.player.rage = cost - 1
    encounter._act(encounter.player)
    require(encounter.player.rage == cost - 1 + sim.rules.rage_per_action,
            "rage from the current action must not be spendable in the same action")


def check_bounds(sim: Simulator) -> None:
    """Sample content must itself pass the proposed engine validation: every derived stat of
    every character and tier stays within the stat bound up to the maximum level, and every
    skill's power is in range."""
    table = sim.content.level_table
    require(table[:1] == (0,) and all(a < b for a, b in zip(table, table[1:])),
            "the level table must start at 0 XP and rise strictly, as the engine requires")
    for profile in (*sim.content.builds, *sim.content.monsters):
        for skill in (*profile.skills, profile.basic):  # every authored skill, reachable or not
            require(POWER_BOUNDS[0] <= skill.power <= POWER_BOUNDS[1],
                    f"{profile.name} {skill.name} power {skill.power} is outside {POWER_BOUNDS}")
            require(skill.cross_share is None or 0 <= skill.cross_share <= 100,
                    f"{profile.name} {skill.name} cross share {skill.cross_share} is outside 0-100")
            require(skill.cost >= 0 and skill.time >= 1,
                    f"{profile.name} {skill.name} needs a nonnegative cost and positive action time")
            require(skill.level >= 1, f"{profile.name} {skill.name} unlocks below level 1")
        require(profile.basic.cost == 0, f"{profile.name}'s basic attack must be free")
        require(profile.growth is None or profile.growth >= 1,
                f"{profile.name} growth {profile.growth} would lower stats as levels rise")
    require(sim.rules.growth >= 1, f"world growth {sim.rules.growth} would lower stats as levels rise")
    require(0 <= sim.rules.cross_share <= 100, f"world cross share {sim.rules.cross_share} is outside 0-100")
    for level in (1, sim.content.max_level):  # growth >= 1 keeps stats monotonic: both ends suffice
        characters = [sim.player(build, level) for build in sim.content.builds]
        characters += [sim.content.scaled(foe, kind).at_level(sim.rules, level)
                       for foe in sim.content.monsters for kind in Kind]
        for c in characters:
            for stat in ("hp", "mp", "patk", "pdef", "satk", "sdef", "speed"):
                low = 1 if stat in ("hp", "speed") else 0
                require(low <= getattr(c, stat) <= STAT_BOUND,
                        f"{c.name} {stat} {getattr(c, stat)} is outside {low}-{STAT_BOUND} at level {level}")
            require(c.xp >= 0, f"{c.name} grants negative XP ({c.xp}) at level {level}")
    # Each skill at its unlock level: attack only rises with level (growth >= 1), so that is
    # where rounding could leave a newly unlocked skill with no attack.
    check_unlocks(sim, [*sim.content.builds,
                        *(sim.content.scaled(foe, kind) for foe in sim.content.monsters for kind in Kind)])


def check_unlocks(sim: Simulator, profiles: list[Profile]) -> None:
    for profile in profiles:
        for skill in (*profile.skills, profile.basic):
            level = max(1, skill.level)
            if level > sim.content.max_level:
                continue  # beyond the table: never reachable
            character = profile.at_level(sim.rules, level)
            require(sim.rules.combined(character, skill) > 0,
                    f"{character.name} has no attack for {skill.name} when it unlocks at level {level}")


def check(sim: Simulator) -> None:
    check_bounds(sim)
    check_formula(sim.rules)
    check_resource_timing(sim)
    for foe in sim.content.monsters:
        check_against(sim.against(foe))
    check_identity(sim)


def check_against(sim: Simulator) -> None:
    """Hard targets against one opponent: every build can win, grind and rest sensibly."""
    foe = sim.foe.name
    for build in sim.content.builds:
        who = f"{build.name} vs {foe}"
        for p in (1, 5, 10, 20, 30):
            same = sim.outcome(build, p, [sim.monster(p)])
            require(same and SAME_LEVEL_ACTIONS[0] <= same.actions <= SAME_LEVEL_ACTIONS[1] and 15 <= same.hp_lost <= 35,
                    f"{who} L{p} same-level: {same}")
            harder = sim.outcome(build, p, [sim.monster(p + 4)])
            require(harder is None or harder.hp_lost >= 40,
                    f"{who} L{p} +4 levels too easy: {harder}")
            require(2 <= sim.fights_per_rest(build, p, p) <= 5,
                    f"{who} L{p} fights per rest")
            require(sim.outcome(build, p, [sim.monster(p, Kind.MINION)] * 2),
                    f"{who} L{p} loses to 2 minions")
        for b in (5, 10, 20):
            need = sim.boss_level_needed(build, b)
            require(need is not None and b <= need <= b + 3,
                    f"{who} L{b} boss needs L{need}")
        require(sim.grind(build, 10).level == 10,
                f"{who} cannot grind to L10")
        require(sim.grind(build, 10, farm=1).level < 10,
                f"{who} can farm L1 opponents to L10")
        # Later skills must matter: at level 20, a build limited to its level-1 skills does clearly worse.
        full = sim.outcome(build, 20, [sim.monster(20)])
        basic = sim.outcome(replace(build, skills=tuple(s for s in build.skills if s.level == 1)),
                            20, [sim.monster(20)])
        require(full and (basic is None or basic.hp_lost >= full.hp_lost + 5),
                f"{who} L20: later skills barely help (with {full}, level-1 skills only {basic})")
    for b in (5, 10, 20):
        needs = [sim.boss_level_needed(build, b) or 99 for build in sim.content.builds]
        require(max(needs) - min(needs) <= BOSS_LEVEL_GAP,
                f"L{b} {foe} boss: builds need levels {needs}")
    boss = sim.monster(5, Kind.BOSS)
    boss_hp = [sim.fight(sim.player(sim.content.warrior, 5, s), [boss]).hp for s in (100, 160, 200)]
    require(boss_hp[0] < boss_hp[1] <= boss_hp[2],
            f"speed gives no benefit against a {foe} boss: {boss_hp}")


IDENTITY_LEVELS = (5, 10, 20)
HP_TIE = 3  # HP-lost differences within this many percentage points count as a tie
MIN_WIN_SHARE = 33  # % of the comparisons where the builds differ that each build must win


def check_identity(sim: Simulator) -> None:
    """Each build must win a real share of the comparisons where the builds differ, across
    every sample opponent and level: a trade-off, not one build ahead with a token edge."""
    warrior, mage = sim.content.warrior, sim.content.mage
    edges: list[tuple[str, int]] = []  # positive: the warrior is better; negative: the mage is
    for foe in sim.content.monsters:
        s = sim.against(foe)
        for p in IDENTITY_LEVELS:
            w, m = s.outcome(warrior, p, [s.monster(p)]), s.outcome(mage, p, [s.monster(p)])
            if w is None or m is None:
                raise TargetMissed(f"L{p} vs {foe.name}: a build loses a same-level fight")
            hp = m.hp_lost - w.hp_lost
            edges += [
                (f"{foe.name} L{p} actions", m.actions - w.actions),
                (f"{foe.name} L{p} HP lost", hp if abs(hp) > HP_TIE else 0),
                (f"{foe.name} L{p} fights/rest", s.fights_per_rest(warrior, p, p) - s.fights_per_rest(mage, p, p)),
                (f"{foe.name} L{p} boss level", (s.boss_level_needed(mage, p) or 99)
                 - (s.boss_level_needed(warrior, p) or 99)),
            ]
    decisive = [edge for _, edge in edges if edge]
    require(decisive, "the builds never differ: there is no trade-off to judge")
    for name, sign in ((warrior.name, 1), (mage.name, -1)):
        won = sum(1 for edge in decisive if edge * sign > 0)
        require(100 * won >= MIN_WIN_SHARE * len(decisive),
                f"{name} wins only {won} of {len(decisive)} comparisons where the builds differ")


def failure(sim: Simulator) -> str | None:
    """The first broken target, or None when every target holds."""
    try:
        check(sim)
    except TargetMissed as error:
        return str(error)
    return None
