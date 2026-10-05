//! Versioned, static world content. No game state or gameplay rules execute here.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

mod combat;
mod crafting;
mod economy;
mod items;
mod places;
mod stats;
mod story;
mod techniques;
mod text;
mod time;
mod troops;
mod validation;
pub use combat::*;
pub use crafting::*;
pub use economy::*;
pub use items::*;
pub use places::*;
pub use stats::*;
pub use story::*;
pub use techniques::*;
pub use time::*;
pub use troops::*;
pub use validation::{Diagnostic, Severity, SpecError};

pub type Id = String;
pub const FORMAT_VERSION: u32 = 19;
/// The files every package holds, by name.
pub const PACKAGE_FILES: [&str; 6] = [
    "world.json",
    "locations.json",
    "characters.json",
    "items.json",
    "quests.json",
    "dialogues.json",
];

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
    /// Absent in a world without a clock; then nothing takes time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<WorldTime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roads: Vec<Road>,
    /// Effects that happen on a schedule; they need world time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<WorldEvent>,
    /// Absent in a world without currency or trade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economy: Option<Economy>,
    /// Absent in a world without soldiers; needs combat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub troops: Option<Troops>,
    /// Absent in a world without mass battles; needs troops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battle: Option<Battle>,
    /// Questions answered at New Game, each option shaping the start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub start_questions: Vec<StartQuestion>,
    /// What the player can discover in an investigation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceDefinition>,
    /// The story's phases in order; absent in a world without them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<Phase>,
    /// The route's authored endings; absent in a world without them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<RouteOutcome>,
}

// Serde defaults shared by several content types.
fn first_level() -> usize {
    1
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl WorldSpec {
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, SpecError> {
        let directory = directory.as_ref();
        Self::parse(directory, |file| std::fs::read(directory.join(file)))
    }

    /// Loads a package from wherever `read` finds each of [`PACKAGE_FILES`],
    /// such as memory or a browser fetch, so a host needs no filesystem.
    pub fn from_files(read: impl FnMut(&str) -> io::Result<Vec<u8>>) -> Result<Self, SpecError> {
        Self::parse(Path::new(""), read)
    }

    fn parse(
        base: &Path,
        mut read: impl FnMut(&str) -> io::Result<Vec<u8>>,
    ) -> Result<Self, SpecError> {
        let mut file = |name: &str| -> Result<(Vec<u8>, PathBuf), SpecError> {
            let path = base.join(name);
            match read(name) {
                Ok(bytes) => Ok((bytes, path)),
                Err(source) => Err(SpecError::Io { path, source }),
            }
        };
        fn typed<T: serde::de::DeserializeOwned>(
            (bytes, path): (Vec<u8>, PathBuf),
        ) -> Result<T, SpecError> {
            serde_json::from_slice(&bytes).map_err(|source| SpecError::Json { path, source })
        }
        // Check the version before the typed parse, which would fail on older
        // packages' fields with an obscure JSON error.
        let (bytes, path) = file("world.json")?;
        let header: serde_json::Value = typed((bytes, path.clone()))?;
        let found = header
            .get("format_version")
            .and_then(serde_json::Value::as_u64);
        if found != Some(FORMAT_VERSION.into()) {
            return Err(SpecError::UnsupportedFormat { found });
        }
        let world = Self {
            world: serde_json::from_value(header)
                .map_err(|source| SpecError::Json { path, source })?,
            locations: typed(file("locations.json")?)?,
            characters: typed(file("characters.json")?)?,
            items: typed(file("items.json")?)?,
            quests: typed(file("quests.json")?)?,
            dialogues: typed(file("dialogues.json")?)?,
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
        self.random_combat()
            || self.random_world()
            || self.random_market()
            || self.random_battle()
            || self.random_stock()
    }
    /// Whether restocking draws random numbers: the economy keeps stock.
    pub fn random_stock(&self) -> bool {
        self.economy().is_some_and(|e| e.stock.is_some())
    }
    /// Whether battles draw random numbers: every round rolls.
    pub fn random_battle(&self) -> bool {
        self.battle().is_some()
    }
    pub fn troops(&self) -> Option<&Troops> {
        self.world.troops.as_ref()
    }
    pub fn battle(&self) -> Option<&Battle> {
        self.world.battle.as_ref()
    }
    /// Whether fights draw random numbers: some hit can be critical.
    pub fn random_combat(&self) -> bool {
        self.combat().is_some_and(|c| {
            c.player_basic_crit.is_some() || c.skills.iter().any(|s| s.crit.is_some())
        }) || self
            .characters
            .iter()
            .any(|c| c.combat.as_ref().is_some_and(|p| p.basic_crit.is_some()))
    }
    /// Whether the world draws random numbers: some character moves.
    pub fn random_world(&self) -> bool {
        self.characters.iter().any(|c| c.moves.is_some())
    }
    /// Whether prices draw random numbers: the economy has a price tick.
    pub fn random_market(&self) -> bool {
        self.economy().is_some_and(|e| e.tick.is_some())
    }
    pub fn economy(&self) -> Option<&Economy> {
        self.world.economy.as_ref()
    }
    /// The highest rank of a proficiency, if the world defines it.
    pub fn proficiency_max(&self, proficiency: Proficiency) -> Option<u32> {
        match proficiency {
            Proficiency::Trading => self.economy()?.trading.as_ref().map(|t| t.max),
        }
    }
    /// A proficiency's name in the world's language, if the world defines it.
    pub fn proficiency_name(&self, proficiency: Proficiency) -> Option<&str> {
        match proficiency {
            Proficiency::Trading => self.economy()?.trading.as_ref().map(|t| t.name.as_str()),
        }
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
    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.combat()?.recipes.iter().find(|v| v.id == id)
    }
    pub fn enchantment(&self, id: &str) -> Option<&Enchantment> {
        self.combat()?.enchantments.iter().find(|v| v.id == id)
    }
    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|v| v.id == id)
    }
    pub fn quest(&self, id: &str) -> Option<&Quest> {
        self.quests.iter().find(|v| v.id == id)
    }
    pub fn evidence(&self, id: &str) -> Option<&EvidenceDefinition> {
        self.world.evidence.iter().find(|v| v.id == id)
    }
    /// A phase's place in the story's order.
    pub fn phase_index(&self, id: &str) -> Option<usize> {
        self.world.phases.iter().position(|p| p.id == id)
    }
    pub fn dialogue(&self, id: &str) -> Option<&Dialogue> {
        self.dialogues.iter().find(|v| v.id == id)
    }
    /// Every authored effect, wherever it is written.
    pub fn effects(&self) -> impl Iterator<Item = &Effect> {
        self.dialogues
            .iter()
            .flat_map(|d| &d.nodes)
            .flat_map(|n| &n.choices)
            .flat_map(|c| &c.effects)
            .chain(self.world.events.iter().flat_map(|e| &e.effects))
            .chain(self.start_options().flat_map(|o| &o.effects))
    }
    /// Every option of every start question.
    pub fn start_options(&self) -> impl Iterator<Item = &StartOption> {
        self.world.start_questions.iter().flat_map(|q| &q.options)
    }
    /// The road from `from` to `to`, if one joins them.
    pub fn road(&self, from: &str, to: &str) -> Option<&Road> {
        self.world
            .roads
            .iter()
            .find(|r| r.leads(from).is_some_and(|other| other == to))
    }
}
