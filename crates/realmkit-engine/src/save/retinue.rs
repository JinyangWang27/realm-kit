//! Save checks for soldiers: squads of real lines and levels, within the
//! roster limit, with every due promotion made, and pools within their size.

use super::*;

pub(super) fn check(world: &WorldSpec, state: &GameState) -> Result<(), String> {
    let (Some(troops), Some(retinue)) = (world.troops(), &state.retinue) else {
        return ensure(
            state.retinue.is_none() && world.troops().is_none(),
            "retinue state does not match the world",
        );
    };
    let squads_ok = retinue.roster.iter().all(|(id, levels)| {
        let Some(line) = troops.line(id) else {
            return false;
        };
        !levels.is_empty()
            && levels.iter().all(|(level, squad)| {
                // A squad short of its last level would have risen already.
                let next = line.levels.get(*level).map(|l| l.xp);
                (1..=line.levels.len()).contains(level)
                    && squad.heads() > 0
                    && next.is_none_or(|xp| squad.share() < xp)
            })
    });
    ensure(squads_ok, "invalid roster")?;
    let heads: u64 = retinue
        .roster
        .values()
        .flat_map(|l| l.values())
        .map(Squad::heads)
        .sum();
    ensure(heads <= troops.limit, "the roster is past its limit")?;
    let recruiting: Vec<_> = world
        .locations
        .iter()
        .filter_map(|l| Some((&l.id, l.recruits.as_ref()?)))
        .collect();
    ensure(
        retinue.pools.len() == recruiting.len()
            && recruiting.iter().all(|(id, recruits)| {
                retinue.pools.get(*id).is_some_and(|pools| {
                    pools.len() == recruits.troops.len()
                        && recruits
                            .troops
                            .iter()
                            .all(|o| pools.get(&o.line).is_some_and(|n| *n <= o.size))
                })
            }),
        "invalid recruiting pools",
    )
}
