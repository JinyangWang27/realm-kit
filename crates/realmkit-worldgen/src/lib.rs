//! Build-time world authoring and export; never a gameplay dependency.

use realmkit_spec::*;
use std::path::Path;

/// Typed authoring core shared by future CLI/MCP adapters. Drafts may be incomplete.
pub struct WorldDraft {
    world: WorldSpec,
    source_language: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthoringError {
    #[error("location already exists: {0}")]
    DuplicateLocation(Id),
    #[error("location does not exist: {0}")]
    MissingLocation(Id),
    #[error(transparent)]
    Package(#[from] SpecError),
}

impl WorldDraft {
    pub fn new(world: WorldSpec) -> Self {
        Self {
            world,
            source_language: None,
        }
    }

    /// Declare the source language. All generated player-facing text must retain it.
    /// Language metadata is checked; prose language/fidelity requires author review.
    pub fn from_source(world: WorldSpec, language: impl Into<String>) -> Self {
        Self {
            world,
            source_language: Some(language.into()),
        }
    }

    pub fn get_world(&self) -> &WorldSpec {
        &self.world
    }

    pub fn create_location(&mut self, location: Location) -> Result<(), AuthoringError> {
        if self.world.location(&location.id).is_some() {
            return Err(AuthoringError::DuplicateLocation(location.id));
        }
        self.world.locations.push(location);
        Ok(())
    }

    pub fn update_location(&mut self, location: Location) -> Result<(), AuthoringError> {
        let existing = self
            .world
            .locations
            .iter_mut()
            .find(|l| l.id == location.id)
            .ok_or_else(|| AuthoringError::MissingLocation(location.id.clone()))?;
        *existing = location;
        Ok(())
    }

    /// Set one directed exit. Call twice to connect both directions.
    pub fn link_locations(
        &mut self,
        from: &str,
        direction: Direction,
        exit: Exit,
    ) -> Result<(), AuthoringError> {
        if self.world.location(&exit.destination).is_none() {
            return Err(AuthoringError::MissingLocation(exit.destination));
        }
        let location = self
            .world
            .locations
            .iter_mut()
            .find(|l| l.id == from)
            .ok_or_else(|| AuthoringError::MissingLocation(from.into()))?;
        location.exits.insert(direction, exit);
        Ok(())
    }

    pub fn validate_world(&self) -> Vec<Diagnostic> {
        let mut diagnostics = self.world.diagnostics();
        if self.source_language.as_ref().is_some_and(|language| {
            language.trim().is_empty() || !language.eq_ignore_ascii_case(&self.world.world.language)
        }) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                entity_id: Some(self.world.world.id.clone()),
                code: "source_language_mismatch".into(),
                message: format!("world language {:?} must match source language {:?}; all generated player-facing content must retain the source language", self.world.world.language, self.source_language.as_ref().unwrap()),
            });
        }
        diagnostics
    }

    /// Validate, then export to a NEW directory; existing paths are never overwritten.
    /// An I/O failure can leave a partial directory, which is not a published package.
    pub fn export(&self, directory: impl AsRef<Path>) -> Result<(), AuthoringError> {
        let diagnostics = self.validate_world();
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            return Err(SpecError::Validation(diagnostics).into());
        }
        let directory = directory.as_ref();
        std::fs::create_dir(directory).map_err(|source| SpecError::Io {
            path: directory.into(),
            source,
        })?;
        write_json(directory, "world.json", &self.world.world)?;
        write_json(directory, "locations.json", &self.world.locations)?;
        write_json(directory, "npcs.json", &self.world.npcs)?;
        write_json(directory, "monsters.json", &self.world.monsters)?;
        write_json(directory, "items.json", &self.world.items)?;
        write_json(directory, "quests.json", &self.world.quests)?;
        write_json(directory, "dialogues.json", &self.world.dialogues)?;
        write_json(directory, "narrative.json", &self.world.narrative)?;
        Ok(())
    }
}

fn write_json(
    directory: &Path,
    file: &str,
    value: &impl serde::Serialize,
) -> Result<(), SpecError> {
    let path = directory.join(file);
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| SpecError::Json {
        path: path.clone(),
        source,
    })?;
    bytes.push(b'\n');
    std::fs::write(&path, bytes).map_err(|source| SpecError::Io { path, source })
}
