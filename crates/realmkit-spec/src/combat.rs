//! A world's fighting rules. Most of them belong to the universal character
//! core: levels, stats, skills, resources, gear and techniques, which hold
//! however a fight is run. The timeline is the optional turn-based layer's
//! and `realtime` the optional real-time layer's; groups and the narrative
//! are read by whichever fights.

use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Combat {
    /// The world's name for the special damage channel: magic, 内力, mana.
    pub special_name: String,
    /// Percentage of the other channel's attack and defence added to a hit.
    #[serde(default = "default_cross_share")]
    pub cross_share: u32,
    /// Turn-based encounters on the initiative timeline; without one, nobody
    /// can be engaged in a personal fight, while everything else stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Timeline>,
    /// Real-time duels on a fixed-step clock; without it, nobody can be
    /// fought one-on-one in real time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realtime: Option<Realtime>,
    #[serde(default)]
    pub resources: Resources,
    /// Which resources the player uses; absent, they follow from the world.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_resources: Option<ResourceUse>,
    pub levels: Vec<Level>,
    #[serde(default)]
    pub skills: Vec<Skill>,
    /// Skills the player can use once their unlock level is reached.
    #[serde(default)]
    pub player_skills: Vec<Id>,
    #[serde(default)]
    pub player_basic_channel: Channel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_basic_crit: Option<Crit>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_points: Option<StatPoints>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub techniques: Vec<Technique>,
    /// Techniques the player knows from the start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_techniques: Vec<TechniqueGrant>,
    /// The internal art whose rank name is shown as the player's realm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_art: Option<Id>,
    /// Technique XP for each use of a technique's skill, before falloff.
    #[serde(default = "default_technique_xp")]
    pub technique_xp_per_use: u32,
    /// The slots equipment can occupy in this world.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Id>,
    /// Gear the player starts with, equipped in order while its slots are free.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_equipment: Vec<Id>,
    /// What can be forged, and at which stations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipes: Vec<Recipe>,
    /// Enchantments that can be laid on a piece, and at which stations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enchantments: Vec<Enchantment>,
    pub narrative: Narrative,
}

fn default_technique_xp() -> u32 {
    10
}

fn default_cross_share() -> u32 {
    25
}

/// Turn-based encounter scheduling: an action at speed `s` and time `t`% delays the actor's
/// next turn by `ceil(action_cost × t / (100 × min(s, speed_cap)))` ticks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    /// Ticks have no real-time meaning; the size only sets integer precision.
    pub action_cost: u64,
    pub speed_cap: u32,
}

/// Real-time duel timing, in fixed steps of the duel's own clock. Phase
/// lengths are a basic action's at speed 100; a fighter at speed `s` using
/// an action of time `t`% holds each phase `max(1, ceil(length × t / s))` steps.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Realtime {
    /// Steps in one second of duel time: the rate a host steps at, and the
    /// span `mp_regen_percent` is measured in. The engine reads no clock.
    pub steps_per_second: u32,
    /// Committed steps before an attack can connect.
    pub windup: u32,
    /// Steps during which an attack can connect, once.
    pub active: u32,
    /// Steps after the active window before the attacker can act again.
    pub recovery: u32,
    /// Steps from raising a guard until it stops hits; 0 is at once.
    #[serde(default)]
    pub guard_raise: u32,
    /// Percent of a hit's damage a raised guard lets through.
    pub guard_percent: u32,
    /// Steps from turning to retreat until the player is away.
    pub retreat: u32,
    /// Percent of maximum MP regained per second of duel time. Duel time is
    /// neither world time nor encounter time.
    #[serde(default)]
    pub mp_regen_percent: u32,
}

/// MP and Rage: what builds, restores and drains them. Every field is
/// optional, and zero or absent leaves that rule out.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// Turn-based only: percent of maximum MP regained per baseline turn of
    /// encounter time. Encounter time is not world time.
    #[serde(default)]
    pub mp_regen_percent: u32,
    /// Rage gained by each attack or skill the character resolves.
    #[serde(default)]
    pub rage_per_action: u32,
    /// Rage gained from taking damage equal to maximum HP, proportionally.
    #[serde(default)]
    pub rage_per_max_hp: u32,
    /// The most Rage a character can hold; absent, it saturates instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rage_cap: Option<u32>,
    /// Rage is kept after a fight and drains by `rage_decay`. Otherwise it
    /// belongs to one fight: it clears when the fight ends.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rage_carries: bool,
    /// MP regained over world time, at fixed minutes of the clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mp_recovery: Option<Recovery>,
    /// Rage lost over world time, at fixed minutes of the clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rage_decay: Option<Decay>,
}

impl Resources {
    /// Whether anything in the world builds Rage.
    pub fn gains_rage(&self) -> bool {
        self.rage_per_action > 0 || self.rage_per_max_hp > 0
    }
}

/// `percent` of maximum MP, rounded down but at least 1, each time world time
/// reaches a multiple of `every` minutes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    pub percent: u32,
    pub every: u64,
}

/// `points` of Rage lost each time world time reaches a multiple of `every`
/// minutes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Decay {
    pub points: u32,
    pub every: u64,
}

/// Which of the two resources a character uses. A resource it does not use
/// stays at 0, gains and regains nothing, and pays for nothing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResourceUse {
    pub mp: bool,
    pub rage: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// A reserve: starts full, regenerates over time and by resting, and is
    /// kept between fights.
    #[default]
    Mp,
    /// Momentum: starts at 0 and builds from acting and being hit.
    Rage,
}

/// What one use of a skill spends from each resource.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Costs {
    #[serde(default)]
    pub mp: u32,
    #[serde(default)]
    pub rage: u32,
}

/// Most steps per second a real-time world declares.
pub const STEP_RATE_BOUND: u32 = 1_000;
/// Longest authored real-time phase, in steps.
pub const PHASE_BOUND: u32 = 10_000;
/// Most steps one advance of a duel's clock may take.
pub const ADVANCE_BOUND: u32 = 10_000;

/// Engine bound for every authored stat; keeps damage arithmetic small.
pub const STAT_BOUND: u32 = 9_999;
/// Upper bound for a timeline's action cost: enough precision for any speed
/// cap, and small enough that timeline arithmetic stays within `u64`.
pub const ACTION_COST_BOUND: u64 = 1_000_000_000_000;
/// An action's time, in percent of a basic action: up to ten basic actions long.
pub const TIME_BOUNDS: (u32, u32) = (1, 1_000);
/// Most pieces of equipment one grant creates, since each is its own item.
pub const GEAR_STACK_BOUND: u64 = 100;
/// Most equipment slots a world declares; keeps worn modifier products exact.
pub const SLOT_BOUND: usize = 32;
/// Skill power is a percentage of a basic attack, which is 100.
pub const POWER_BOUNDS: (u32, u32) = (1, 1_000);
pub const BASIC_POWER: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    pub id: Id,
    pub name: String,
    pub power: u32,
    pub channel: Channel,
    /// Spent per use from `resource`, exactly as authored.
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub resource: Resource,
    /// Costs in both resources, in place of `cost` and `resource`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub costs: Option<Costs>,
    /// Action length in percent of a basic attack. The turn-based timeline
    /// delays the user's next turn by it.
    #[serde(default = "full_time")]
    pub time: u32,
    /// Character level at which the player can use it.
    #[serde(default = "first_level")]
    pub level: usize,
    /// Overrides the world's cross share for this skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_share: Option<u32>,
    /// Allows `{attacker}`, `{target}` and `{damage}`.
    pub text: TextTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crit: Option<Crit>,
}

/// A chance for a landed hit to deal more damage, drawn from the world's
/// seeded random stream.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Crit {
    /// 1 to 100.
    pub chance_percent: u32,
    /// Damage in percent of a normal hit, 101 to 1,000.
    pub multiplier_percent: u32,
}

impl Skill {
    /// What one use spends from `resource`.
    pub fn cost_in(&self, resource: Resource) -> u32 {
        match (self.costs, resource) {
            (Some(costs), Resource::Mp) => costs.mp,
            (Some(costs), Resource::Rage) => costs.rage,
            (None, r) if r == self.resource => self.cost,
            (None, _) => 0,
        }
    }
}

fn full_time() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// Cumulative experience required for this level; level one starts at zero.
    pub xp: u64,
    pub stats: Stats,
    /// Stat points granted on reaching this level; the first level's are the
    /// starting pool.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub points: u32,
    /// Proficiency points granted on reaching this level; the first level's
    /// are the starting pool.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub proficiency_points: u32,
}

/// Player-allocated stat points: what one point adds to each stat that
/// accepts points, optional per-stat caps, and whether they can be refunded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatPoints {
    pub values: BTreeMap<Stat, u32>,
    /// Most points one stat may take.
    #[serde(default)]
    pub caps: BTreeMap<Stat, u32>,
    #[serde(default)]
    pub respec: Respec,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Respec {
    /// Allocation is permanent.
    #[default]
    Never,
    /// Everything can be refunded at a safe location.
    Safe,
}

/// Opponents who fight together, and how their encounters end.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: Id,
    /// Defeats are never recorded, so the group can be fought again at once.
    #[serde(default)]
    pub repeatable: bool,
    /// Participants yield at this percentage of maximum HP instead of dying.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_share: Option<u32>,
    #[serde(default)]
    pub no_flee: bool,
    /// Set when the player's side wins.
    #[serde(default)]
    pub victory_flags: Vec<Id>,
    /// Set when the player yields.
    #[serde(default)]
    pub defeat_flags: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct TextTemplate(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Narrative {
    pub attack: Vec<TextTemplate>,
    pub hurt: Vec<TextTemplate>,
    pub victory: TextTemplate,
    pub death: String,
}
