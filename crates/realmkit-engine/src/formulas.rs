//! Pure combat and trade arithmetic, shared with `scripts/combat_sim`.

use super::*;

/// Damage of one landed hit: the channel's combined attack `A` against its
/// combined defence `D` (each 100 × main + share × other), then
/// `max(1, A × power × A / (100 × 100 × (A + D)))`, rounded down once.
/// Defence equal to the attack halves damage; no scale constant or level enters.
pub fn damage(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
) -> Result<u32, EngineError> {
    damage_scaled(attacker, defender, channel, power, share, 100)
}

/// [`damage`] scaled by `multiplier` percent (a critical hit) before the
/// single final rounding.
pub fn damage_scaled(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
    multiplier: u32,
) -> Result<u32, EngineError> {
    damage_modified(
        attacker,
        defender,
        channel,
        power,
        share,
        multiplier,
        (1, 1),
    )
}

/// [`damage_scaled`] with the defender's damage modifier `num / den` for the
/// channel, before the single rounding. Immunity (`num` 0) deals nothing, not
/// the minimum 1.
pub fn damage_modified(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
    multiplier: u32,
    (num, den): (u64, u64),
) -> Result<u32, EngineError> {
    if num == 0 {
        return Ok(0);
    }
    let a = attacker.combined(channel, share, false);
    let d = defender.combined(channel, share, true);
    let hit = a
        .checked_mul(u128::from(power))
        .and_then(|v| v.checked_mul(a))
        .and_then(|v| v.checked_mul(u128::from(multiplier)))
        .and_then(|v| v.checked_mul(u128::from(num)))
        .and_then(|v| v.checked_div(100 * 100 * 100 * (a + d) * u128::from(den)))
        .ok_or(EngineError::NumericLimit)?;
    u32::try_from(hit.max(1)).map_err(|_| EngineError::NumericLimit)
}

/// XP for defeating an opponent, by level difference `opponent − player`:
/// ±10% per level, capped at ±40%, rounded down, and nothing five or more
/// levels below. Mirrors `scripts/combat_sim` `XpRules.for_kill`.
pub fn xp_for_defeat(xp: u64, player_level: usize, opponent_level: usize) -> u64 {
    let diff = opponent_level as i128 - player_level as i128;
    if diff <= -5 {
        return 0;
    }
    let scaled = u128::from(xp) * (10 + diff.clamp(-4, 4)) as u128 / 10;
    // At most 140% of a u64 value: saturate rather than wrap on absurd content.
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

/// What one unit costs to buy: the base price at `index` thousandths, raised
/// by the spread, rounded down once, and never free.
pub fn buy_price(price: u64, index: u32, spread: u32) -> u64 {
    levied_buy_price(price, index, spread, 0)
}

/// What one unit sells for: the base price at `index` thousandths, divided
/// by the spread, rounded down once.
pub fn sell_price(price: u64, index: u32, spread: u32) -> u64 {
    levied_sell_price(price, index, spread, 0)
}

/// Buying with additive levies, applied before the single final rounding.
/// Validated content bounds every factor; buying always costs at least one.
pub fn levied_buy_price(price: u64, index: u32, spread: u32, levy: u32) -> u64 {
    let raw = u128::from(price)
        * u128::from(index)
        * (100 + u128::from(spread))
        * (100 + u128::from(levy))
        / 10_000_000;
    u64::try_from(raw).unwrap_or(u64::MAX).max(1)
}

/// Selling with additive levies (0..=100), before the single final rounding.
pub fn levied_sell_price(price: u64, index: u32, spread: u32, levy: u32) -> u64 {
    let raw = u128::from(price) * u128::from(index) * u128::from(100_u32.saturating_sub(levy))
        / (1_000 * (100 + u128::from(spread)));
    u64::try_from(raw).unwrap_or(u64::MAX)
}

/// One charge applied to a quote or a completed trade, in authored order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedLevy {
    pub id: Id,
    pub text: String,
    pub percent: u32,
}

/// One unit of a good at a market: what it costs and fetches now, and at
/// the next index using the same pre-trade facts (the next unit in a batch).
/// A new command re-evaluates conditions, including cargo/currency conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    /// Charges already included in these prices.
    pub levies: Vec<AppliedLevy>,
    pub buy: u64,
    pub sell: u64,
    pub next_buy: u64,
    pub next_sell: u64,
    /// Units the merchants hold, where the world keeps stock.
    pub stock: Option<u64>,
    /// What the merchants can pay out, where the world keeps stock.
    pub purse: Option<u64>,
}
