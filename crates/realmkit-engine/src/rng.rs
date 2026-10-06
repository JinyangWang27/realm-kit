//! Seeded, versioned randomness. The generator is written out here rather
//! than taken from a crate so its output can never change under a save.

use serde::{Deserialize, Serialize};

/// The algorithm a saved stream uses: 1 is SplitMix64.
pub const RNG_VERSION: u32 = 1;
/// "combat" in ASCII: each domain starts from its own state, so draws in one
/// never shift another.
const DOMAIN_COMBAT: u64 = 0x636f_6d62_6174;
/// "world" in ASCII: characters who move.
const DOMAIN_WORLD: u64 = 0x77_6f72_6c64;
/// "market" in ASCII: the price tick.
const DOMAIN_MARKET: u64 = 0x6d61_726b_6574;
/// "battle" in ASCII: mass-battle rolls.
const DOMAIN_BATTLE: u64 = 0x6261_7474_6c65;
/// "stock" in ASCII: merchants restocking.
const DOMAIN_STOCK: u64 = 0x73_746f_636b;
/// "travel" in ASCII: roads whose time varies.
const DOMAIN_TRAVEL: u64 = 0x7472_6176_656c;

/// Saved generator state, one stream per random domain. A domain exists
/// only in worlds whose content draws from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RngState {
    pub version: u32,
    /// Critical hits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<u64>,
    /// Characters who move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<u64>,
    /// The price tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market: Option<u64>,
    /// Mass-battle rolls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battle: Option<u64>,
    /// Merchants restocking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<u64>,
    /// Roads whose time varies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub travel: Option<u64>,
}

impl RngState {
    /// The streams `world`'s content draws from, or `None` if it draws nothing.
    pub fn for_world(world: &realmkit_spec::WorldSpec, seed: u64) -> Option<Self> {
        let state = Self {
            version: RNG_VERSION,
            combat: world.random_combat().then_some(seed ^ DOMAIN_COMBAT),
            world: world.random_world().then_some(seed ^ DOMAIN_WORLD),
            market: world.random_market().then_some(seed ^ DOMAIN_MARKET),
            battle: world.random_battle().then_some(seed ^ DOMAIN_BATTLE),
            stock: world.random_stock().then_some(seed ^ DOMAIN_STOCK),
            travel: world.random_travel().then_some(seed ^ DOMAIN_TRAVEL),
        };
        world.stochastic().then_some(state)
    }
}

/// One SplitMix64 step: advances `state` and returns the next output.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A uniform draw below `n` (which is positive), scaled by multiplication
/// rather than a biased modulo.
pub(crate) fn below(state: &mut u64, n: u64) -> u64 {
    ((u128::from(splitmix64(state)) * u128::from(n)) >> 64) as u64
}

/// A draw that succeeds `percent` times in 100, scaled by multiplication
/// rather than a biased modulo.
pub(crate) fn chance(state: &mut u64, percent: u32) -> bool {
    ((u128::from(splitmix64(state)) * 100) >> 64) < u128::from(percent)
}
