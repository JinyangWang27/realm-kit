"""The balance targets as assertions; each failure names the broken target."""
from __future__ import annotations

from dataclasses import replace

from .model import Channel, Combatant, Formula, Kind, Rules, Skill
from .simulator import Simulator

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
        assert hit(0, 0, Channel.PHYSICAL) == 45 and hit(45, 0, Channel.PHYSICAL) == 22
        # high 内力 (special defence 80) blocks a physical hit like 20 physical defence
        assert hit(0, 80, Channel.PHYSICAL) == hit(20, 0, Channel.PHYSICAL) == 31
        # special: 20 + 25% of 40 = 30
        assert hit(0, 0, Channel.SPECIAL) == 30


def check(sim: Simulator) -> None:
    check_formula(sim.rules)
    warrior, mage = sim.content.warrior, sim.content.mage
    for build in sim.content.builds:
        for p in (1, 5, 10, 20, 30):
            same = sim.outcome(build, p, [sim.monster(p)])
            assert same and SAME_LEVEL_ACTIONS[0] <= same.actions <= SAME_LEVEL_ACTIONS[1] \
                and 15 <= same.hp_lost <= 35, f"{build.name} L{p} same-level: {same}"
            harder = sim.outcome(build, p, [sim.monster(p + 4)])
            assert harder is None or harder.hp_lost >= 40, f"{build.name} L{p} +4 levels too easy: {harder}"
            assert 2 <= sim.fights_per_rest(build, p, p) <= 5, f"{build.name} L{p} fights per rest"
            assert sim.outcome(build, p, [sim.monster(p, Kind.MINION)] * 2), f"{build.name} L{p} loses to 2 minions"
        for b in (5, 10, 20):
            need = sim.boss_level_needed(build, b)
            assert need is not None and b <= need <= b + 3, f"{build.name} L{b} boss needs L{need}"
        assert sim.grind(build, 10).level == 10, f"{build.name} cannot grind to L10"
        assert sim.grind(build, 10, farm=1).level < 10, f"{build.name} can farm L1 monsters to L10"
    # Build identity: the mage bursts, the warrior sustains; neither dominates.
    for p in (5, 10, 20):
        w, m = sim.outcome(warrior, p, [sim.monster(p)]), sim.outcome(mage, p, [sim.monster(p)])
        assert w and m and m.actions < w.actions, f"L{p}: mage should finish faster (mage {m}, warrior {w})"
        assert sim.fights_per_rest(warrior, p, p) > sim.fights_per_rest(mage, p, p), \
            f"L{p}: warrior should fight longer between rests"
    for b in (5, 10, 20):
        needs = [sim.boss_level_needed(build, b) or 99 for build in sim.content.builds]
        assert max(needs) - min(needs) <= BOSS_LEVEL_GAP, f"L{b} boss: builds need levels {needs}"
    # Later skills must matter: at level 20, a build limited to its level-1 skills does clearly worse.
    for build in sim.content.builds:
        full = sim.outcome(build, 20, [sim.monster(20)])
        basic = sim.outcome(replace(build, skills=tuple(s for s in build.skills if s.level == 1)),
                            20, [sim.monster(20)])
        assert full and (basic is None or basic.hp_lost >= full.hp_lost + 5), \
            f"{build.name} L20: later skills barely help (with {full}, level-1 skills only {basic})"
    boss = sim.monster(5, Kind.BOSS)
    boss_hp = [sim.fight(sim.player(warrior, 5, s), [boss]).hp for s in (100, 160, 200)]
    assert boss_hp[0] < boss_hp[1] <= boss_hp[2], f"speed gives no boss-fight benefit: {boss_hp}"


def failure(sim: Simulator) -> str | None:
    """The first broken target, or None when every target holds."""
    try:
        check(sim)
    except AssertionError as error:
        return str(error)
    return None
