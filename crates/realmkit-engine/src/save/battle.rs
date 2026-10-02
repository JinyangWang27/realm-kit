//! Save checks for a battle in progress: the army and allies present, every
//! stack the one the battle began with or fewer, and the battle undecided.

use super::*;

pub(super) fn check(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    battle: &BattleState,
) -> Result<(), String> {
    let invalid = || "invalid battle state".to_string();
    let army = rules::character_here(world, state, &battle.army)
        .filter(|c| !combat.defeated.contains(&c.id))
        .and_then(|c| c.army.as_ref())
        .filter(|a| a.joins.is_none())
        .ok_or_else(invalid)?;
    let rules = world.battle().ok_or_else(invalid)?;
    let max = rules::player_stats(world, combat);
    // Conditions cannot change mid-battle, so the allies are those here now.
    let allies_ok = battle.allies == crate::battle::allies_here(world, state);
    let mut ours = crate::battle::roster_stacks(world, state);
    for ally in &battle.allies {
        let joined = world.character(ally).and_then(|c| c.army.as_ref());
        ours.extend(joined.map(crate::battle::army_stacks).unwrap_or_default());
    }
    let theirs = crate::battle::army_stacks(army);
    let troops = world.troops().ok_or_else(invalid)?;
    // Each stack is the one the side began with, with no more soldiers, and
    // less damage carried than one more loss.
    let side_ok = |side: &BattleSide, start: &[BattleStack], extra: u64| {
        side.stacks.len() == start.len()
            && side.start_size == extra + start.iter().map(|s| s.count).sum::<u64>()
            && side.morale <= 100
            && side.morale_remainder < side.start_size
            && rules.morale.is_none_or(|m| side.morale >= m.rout)
            && side.stacks.iter().zip(start).all(|(now, then)| {
                let hp = troops
                    .line(&now.line)
                    .and_then(|l| l.levels.get(now.level.checked_sub(1)?))
                    .map(|l| l.stats.hp);
                now.line == then.line
                    && now.level == then.level
                    && now.count <= then.count
                    && hp.is_some_and(|hp| now.remainder < u64::from(hp))
                    && (now.count > 0 || now.remainder == 0)
            })
    };
    // An undecided battle: both sides still have someone standing.
    let standing = [
        battle.hp > 0 || battle.sides[0].stacks.iter().any(|s| s.count > 0),
        battle.sides[1].stacks.iter().any(|s| s.count > 0),
    ];
    ensure(
        allies_ok
            && side_ok(&battle.sides[0], &ours, 1)
            && side_ok(&battle.sides[1], &theirs, 0)
            && battle.round < rules.rounds
            && standing == [true, true]
            && battle.hp <= max.hp
            && battle.mp <= max.mp
            && state.dialogue.is_none(),
        "invalid battle state",
    )
}
