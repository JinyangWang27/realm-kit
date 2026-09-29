//! Versioned, static world content. No game state or gameplay rules execute here.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

mod validation;
pub use validation::{Diagnostic, Severity, SpecError};

pub type Id = String;
pub const FORMAT_VERSION: u32 = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorldSpec {
    pub world: World,
    pub locations: Vec<Location>,
    pub characters: Vec<Character>,
    pub items: Vec<Item>,
    pub quests: Vec<Quest>,
    pub dialogues: Vec<Dialogue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct World {
    pub format_version: u32,
    pub id: Id,
    pub name: String,
    /// Language tag for authored player-facing content (for example "en" or "zh-Hans").
    pub language: String,
    pub start: Id,
    /// The player-controlled character.
    pub player: Id,
    #[serde(default)]
    pub flags: Vec<Id>,
    /// Absent in a world without fighting; then there is no XP or level either.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<Combat>,
}

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
    pub narrative: Narrative,
}

fn default_technique_xp() -> u32 {
    10
}

/// A technique mastered rank by rank. Each rank is named by the author.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Technique {
    pub id: Id,
    pub name: String,
    pub ranks: Vec<TechniqueRank>,
    /// Percent of the character XP from each victory this technique gains;
    /// small values make internal arts rise slowly.
    #[serde(default)]
    pub xp_share_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TechniqueRank {
    /// Shown to the player instead of a number, in the source's own terms.
    pub name: String,
    /// Cumulative technique XP for this rank; the first rank's is 0.
    pub xp: u64,
    /// The skill the technique is used as at this rank; passive arts have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<Id>,
    /// The technique's whole stat bonus at this rank, replacing the previous rank's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub passive: BTreeMap<Stat, u32>,
    /// A breakthrough gate: technique XP waits at this rank's threshold until it holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Condition>,
}

/// Teaches a technique if it is unknown, raises it to at least `rank`
/// (1-based; teaching passes gates), then adds `xp`, which gates can hold back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TechniqueGrant {
    pub technique: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<usize>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub xp: u64,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
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
/// Skill power is a percentage of a basic attack, which is 100.
pub const POWER_BOUNDS: (u32, u32) = (1, 1_000);
pub const BASIC_POWER: u32 = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub hp: u32,
    #[serde(default)]
    pub mp: u32,
    pub patk: u32,
    pub pdef: u32,
    pub satk: u32,
    pub sdef: u32,
    /// How often the character acts in an encounter.
    pub speed: u32,
}

/// One of the seven combat stats, for content that names a stat.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Hp,
    Mp,
    Patk,
    Pdef,
    Satk,
    Sdef,
    Speed,
}

impl Stat {
    pub const ALL: [Stat; 7] = [
        Stat::Hp,
        Stat::Mp,
        Stat::Patk,
        Stat::Pdef,
        Stat::Satk,
        Stat::Sdef,
        Stat::Speed,
    ];
}

impl Stats {
    pub fn get(&self, stat: Stat) -> u32 {
        match stat {
            Stat::Hp => self.hp,
            Stat::Mp => self.mp,
            Stat::Patk => self.patk,
            Stat::Pdef => self.pdef,
            Stat::Satk => self.satk,
            Stat::Sdef => self.sdef,
            Stat::Speed => self.speed,
        }
    }
    pub fn get_mut(&mut self, stat: Stat) -> &mut u32 {
        match stat {
            Stat::Hp => &mut self.hp,
            Stat::Mp => &mut self.mp,
            Stat::Patk => &mut self.patk,
            Stat::Pdef => &mut self.pdef,
            Stat::Satk => &mut self.satk,
            Stat::Sdef => &mut self.sdef,
            Stat::Speed => &mut self.speed,
        }
    }

    /// The channel's attack (or defence) plus `share` percent of the other
    /// channel's, scaled by 100 so the share adds no rounding step.
    /// Wide enough that unvalidated stats and shares cannot overflow.
    pub fn combined(&self, channel: Channel, share: u32, defence: bool) -> u128 {
        let (physical, special) = if defence {
            (self.pdef, self.sdef)
        } else {
            (self.patk, self.satk)
        };
        let (main, other) = match channel {
            Channel::Physical => (physical, special),
            Channel::Special => (special, physical),
        };
        100 * u128::from(main) + u128::from(share) * u128::from(other)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Physical,
    Special,
}

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

fn first_level() -> usize {
    1
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

fn is_zero(value: &u32) -> bool {
    *value == 0
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub id: Id,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub exits: BTreeMap<Direction, Exit>,
    #[serde(default)]
    pub characters: Vec<Id>,
    /// Resting here restores HP and MP. Requires combat.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub safe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    pub destination: Id,
    #[serde(default)]
    pub requires: Vec<Condition>,
    pub blocked_text: String,
}

/// Anyone in the world. Talking and fighting are optional components.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: Id,
    pub name: String,
    pub description: String,
    /// Conditions for the character to be present where it is placed.
    #[serde(default)]
    pub requires: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<CombatProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CombatProfile {
    pub stats: Stats,
    pub xp: u64,
    #[serde(default)]
    pub loot: Vec<ItemStack>,
    /// Usable once the profile's level reaches each skill's unlock level.
    #[serde(default)]
    pub skills: Vec<Id>,
    #[serde(default)]
    pub basic_channel: Channel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_crit: Option<Crit>,
    /// Gates the profile's skills and scales the XP it grants.
    #[serde(default = "first_level")]
    pub level: usize,
    /// Engaging any member brings in every present, undefeated member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<Id>,
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
#[serde(deny_unknown_fields)]
pub struct ItemStack {
    pub item: Id,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: Id,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Quest {
    pub id: Id,
    pub name: String,
    pub giver: Id,
    pub objective: QuestObjective,
    pub introduction: String,
    pub progress: String,
    pub completion: String,
    #[serde(default)]
    pub reward_xp: u64,
    #[serde(default)]
    pub reward_items: Vec<ItemStack>,
    #[serde(default)]
    pub completion_flags: Vec<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reward_techniques: Vec<TechniqueGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuestObjective {
    Defeat { character: Id },
    Flag { flag: Id },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Available,
    Active,
    Ready,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    Flag {
        flag: Id,
    },
    Quest {
        quest: Id,
        status: QuestStatus,
    },
    /// The player has learned `technique` at least to `rank` (1-based).
    Technique {
        technique: Id,
        rank: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dialogue {
    pub id: Id,
    pub start: Id,
    pub nodes: Vec<DialogueNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueNode {
    pub id: Id,
    pub text: String,
    #[serde(default)]
    pub choices: Vec<DialogueChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueChoice {
    pub text: String,
    pub next: Option<Id>,
    #[serde(default)]
    pub requires: Vec<Condition>,
    pub effect: Option<DialogueEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DialogueEffect {
    AcceptQuest {
        quest: Id,
    },
    CompleteQuest {
        quest: Id,
    },
    SetFlag {
        flag: Id,
    },
    /// A master, manual or chance encounter teaches or deepens a technique.
    GrantTechnique(TechniqueGrant),
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

impl WorldSpec {
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, SpecError> {
        let directory = directory.as_ref();
        // Check the version before the typed parse, which would fail on older
        // packages' fields with an obscure JSON error.
        let header: serde_json::Value = read_json(directory, "world.json")?;
        let found = header
            .get("format_version")
            .and_then(serde_json::Value::as_u64);
        if found != Some(FORMAT_VERSION.into()) {
            return Err(SpecError::UnsupportedFormat { found });
        }
        let world = Self {
            world: serde_json::from_value(header).map_err(|source| SpecError::Json {
                path: directory.join("world.json"),
                source,
            })?,
            locations: read_json(directory, "locations.json")?,
            characters: read_json(directory, "characters.json")?,
            items: read_json(directory, "items.json")?,
            quests: read_json(directory, "quests.json")?,
            dialogues: read_json(directory, "dialogues.json")?,
        };
        world.validate()?;
        Ok(world)
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        let diagnostics = self.diagnostics();
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            Err(SpecError::Validation(diagnostics))
        } else {
            Ok(())
        }
    }

    /// Collect all content errors for an author/validate/repair loop.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        validation::diagnostics(self)
    }

    /// Identifies this exact content. Saves bind to it, so any edit makes older
    /// saves incompatible until a migration exists.
    // ponytail: 64-bit FNV-1a over canonical JSON detects edits, not tampering;
    // switch to SHA-256 if revisions must be adversarially unique.
    pub fn revision(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("world specs always serialize");
        let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
        format!("fnv1a64:{hash:016x}")
    }

    // ponytail: linear lookup suits small authored worlds; index IDs if profiling warrants it.
    pub fn location(&self, id: &str) -> Option<&Location> {
        self.locations.iter().find(|v| v.id == id)
    }
    pub fn character(&self, id: &str) -> Option<&Character> {
        self.characters.iter().find(|v| v.id == id)
    }
    pub fn combat(&self) -> Option<&Combat> {
        self.world.combat.as_ref()
    }
    /// Whether any content draws random numbers; only then does play keep a
    /// seeded generator.
    pub fn stochastic(&self) -> bool {
        self.combat().is_some_and(|c| {
            c.player_basic_crit.is_some() || c.skills.iter().any(|s| s.crit.is_some())
        }) || self
            .characters
            .iter()
            .any(|c| c.combat.as_ref().is_some_and(|p| p.basic_crit.is_some()))
    }
    pub fn technique(&self, id: &str) -> Option<&Technique> {
        self.combat()?.techniques.iter().find(|v| v.id == id)
    }
    pub fn group(&self, id: &str) -> Option<&Group> {
        self.combat()?.groups.iter().find(|v| v.id == id)
    }
    pub fn skill(&self, id: &str) -> Option<&Skill> {
        self.combat()?.skills.iter().find(|v| v.id == id)
    }
    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|v| v.id == id)
    }
    pub fn quest(&self, id: &str) -> Option<&Quest> {
        self.quests.iter().find(|v| v.id == id)
    }
    pub fn dialogue(&self, id: &str) -> Option<&Dialogue> {
        self.dialogues.iter().find(|v| v.id == id)
    }
}

fn read_json<T: serde::de::DeserializeOwned>(directory: &Path, file: &str) -> Result<T, SpecError> {
    let path = directory.join(file);
    let bytes = std::fs::read(&path).map_err(|source| SpecError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SpecError::Json { path, source })
}
