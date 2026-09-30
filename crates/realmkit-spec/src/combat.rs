//! A world's fighting rules: timeline, resources, levels, skills and groups.

use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Combat {
    /// The world's name for the special damage channel: magic, 内力, mana.
    pub special_name: String,
    /// Percentage of the other channel's attack and defence added to a hit.
    #[serde(default = "default_cross_share")]
    pub cross_share: u32,
    pub timeline: Timeline,
    #[serde(default)]
    pub resources: Resources,
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
    pub narrative: Narrative,
}

fn default_technique_xp() -> u32 {
    10
}

fn default_cross_share() -> u32 {
    25
}

/// Encounter scheduling: an action at speed `s` and time `t`% delays the actor's
/// next turn by `ceil(action_cost × t / (100 × min(s, speed_cap)))` ticks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    /// Ticks have no real-time meaning; the size only sets integer precision.
    pub action_cost: u64,
    pub speed_cap: u32,
}

/// Skill resources during encounters; zero leaves a resource unused.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// Percent of maximum MP regained per baseline turn of encounter time.
    #[serde(default)]
    pub mp_regen_percent: u32,
    #[serde(default)]
    pub rage_per_action: u32,
    /// Rage gained from taking damage equal to maximum HP, proportionally.
    #[serde(default)]
    pub rage_per_max_hp: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// Kept between encounters; regenerates over encounter time and by resting.
    #[default]
    Mp,
    /// Starts at 0 in every encounter; builds from acting and being hit.
    Rage,
}

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
    /// Spent per use, exactly as authored.
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub resource: Resource,
    /// Action length in percent of a basic attack; delays the user's next turn.
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
