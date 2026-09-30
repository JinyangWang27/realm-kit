//! Versioned, static world content. No game state or gameplay rules execute here.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

mod combat;
mod crafting;
mod items;
mod places;
mod stats;
mod story;
mod techniques;
mod validation;
pub use combat::*;
pub use crafting::*;
pub use items::*;
pub use places::*;
pub use stats::*;
pub use story::*;
pub use techniques::*;
pub use validation::{Diagnostic, Severity, SpecError};

pub type Id = String;
pub const FORMAT_VERSION: u32 = 11;

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
