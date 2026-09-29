//! Combat numbers shared by the level table, profiles and bonuses.

use crate::*;

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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Physical,
    Special,
}
