//! Local save files: one JSON snapshot per save plus `lineage.json`, the active
//! recovery chain, oldest first. Loading an older save forks the chain; saves
//! from the abandoned future stay on disk but are no longer offered.

use realmkit_engine::{EngineError, SaveSnapshot};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const LINEAGE: &str = "lineage.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Auto,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: u64,
    pub kind: Kind,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lineage {
    /// Never reused, so an abandoned save file is never overwritten.
    next: u64,
    entries: Vec<Entry>,
}

/// Why a load failed, with the chosen save's position when one was chosen.
pub type LoadError = (Option<usize>, Box<dyn Error>);

/// One game's exclusive hold on a saves directory. Two games playing
/// different branches of one recovery chain cannot both be right, so a second
/// game is refused rather than allowed to interleave its saves.
pub struct Saves {
    dir: PathBuf,
    _lock: fs::File,
}

impl Saves {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let lock = fs::File::create(dir.join("lock"))?;
        lock.try_lock().map_err(|e| match e {
            fs::TryLockError::WouldBlock => {
                format!("another game is using the saves in {}", dir.display()).into()
            }
            fs::TryLockError::Error(e) => Box::<dyn Error>::from(e),
        })?;
        Ok(Self { dir, _lock: lock })
    }

    /// Reads the index and rejects one that cannot be trusted: saves listed
    /// out of order, or a save file at or past its counter. Every write
    /// reserves its ID first, so no crash can produce the latter; a missing,
    /// emptied or restored-from-backup index can, and would hide progress.
    fn lineage(&self) -> Result<Lineage, Box<dyn Error>> {
        let path = self.dir.join(LINEAGE);
        let lineage: Lineage = match fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Lineage::default(),
            Err(e) => return Err(format!("{}: {e}", path.display()).into()),
        };
        let ids: Vec<_> = lineage.entries.iter().map(|e| e.id).collect();
        if !ids.windows(2).all(|w| w[0] < w[1]) || ids.last().is_some_and(|id| *id >= lineage.next)
        {
            return Err(format!("{} lists saves out of order", path.display()).into());
        }
        for file in fs::read_dir(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))? {
            let file = file?.path();
            let id = file
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<u64>().ok());
            if file.extension().is_some_and(|x| x == "json")
                && id.is_some_and(|id| id >= lineage.next)
            {
                return Err(format!(
                    "{} does not list every save in {}",
                    path.display(),
                    self.dir.display()
                )
                .into());
            }
        }
        Ok(lineage)
    }

    pub fn entries(&self) -> Result<Vec<Entry>, Box<dyn Error>> {
        Ok(self.lineage()?.entries)
    }

    /// Reserves the ID, writes the snapshot, then lists it. A failure at any
    /// step leaves earlier saves and the chain untouched; at worst an unlisted
    /// file remains below the counter, like an abandoned save.
    pub fn write(&self, kind: Kind, snapshot: &SaveSnapshot) -> Result<(), Box<dyn Error>> {
        let mut lineage = self.lineage()?;
        let id = lineage.next;
        lineage.next = id.checked_add(1).ok_or("save counter exhausted")?;
        write_atomic(
            &self.dir.join(LINEAGE),
            &serde_json::to_vec_pretty(&lineage)?,
        )?;
        write_atomic(&self.file(id), &snapshot.to_json())?;
        lineage.entries.push(Entry { id, kind });
        write_atomic(
            &self.dir.join(LINEAGE),
            &serde_json::to_vec_pretty(&lineage)?,
        )?;
        Ok(())
    }

    pub fn read(&self, id: u64) -> Result<SaveSnapshot, Box<dyn Error>> {
        let path = self.file(id);
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        SaveSnapshot::from_json(&bytes).map_err(|error| match error {
            EngineError::InvalidSave(reason) => format!("{}: {reason}", path.display()).into(),
            other => other.into(),
        })
    }

    /// Loads the save at one-based-minus-one `index` (the newest when `None`)
    /// and, once `check` accepts it, makes it the newest. On failure, the
    /// position is given when one was chosen.
    pub fn load<T>(
        &self,
        index: Option<usize>,
        check: impl FnOnce(SaveSnapshot) -> Result<T, Box<dyn Error>>,
    ) -> Result<(usize, T), LoadError> {
        let mut lineage = self
            .lineage()
            .map_err(|e| (None, format!("Saves could not be read: {e}").into()))?;
        let index = index
            .or(lineage.entries.len().checked_sub(1))
            .ok_or((None, "There are no saves yet.".into()))?;
        let entry = lineage
            .entries
            .get(index)
            .ok_or((None, "That save is no longer in the recovery chain.".into()))?;
        let value = self
            .read(entry.id)
            .and_then(check)
            .map_err(|e| (Some(index), e))?;
        if index + 1 < lineage.entries.len() {
            lineage.entries.truncate(index + 1);
            let bytes = serde_json::to_vec_pretty(&lineage).map_err(|e| (Some(index), e.into()))?;
            write_atomic(&self.dir.join(LINEAGE), &bytes).map_err(|e| (Some(index), e.into()))?;
        }
        Ok((index, value))
    }

    fn file(&self, id: u64) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }
}

/// Replaces `path` whole or not at all.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    // The rename lives in the directory; sync it so a crash cannot publish the
    // lineage while losing the snapshot it names. The replacement is already
    // committed, so a sync failure must not report the write as failed.
    // ponytail: Unix only; Windows cannot open a directory as a file for syncing.
    #[cfg(unix)]
    let _ = fs::File::open(path.parent().unwrap_or(Path::new("."))).and_then(|d| d.sync_all());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use realmkit_engine::Engine;
    use realmkit_spec::WorldSpec;

    #[test]
    fn loading_an_older_save_forks_the_chain_without_reusing_files() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let snapshot = Engine::new(&world).unwrap().snapshot();
        let dir = std::env::temp_dir().join(format!("realmkit-fork-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let saves = Saves::open(&dir).unwrap();
        assert!(saves.entries().unwrap().is_empty());
        for kind in [Kind::Auto, Kind::Manual, Kind::Manual] {
            saves.write(kind, &snapshot).unwrap();
        }
        saves.load(Some(0), Ok).unwrap();
        saves.write(Kind::Auto, &snapshot).unwrap();
        let ids: Vec<_> = saves.entries().unwrap().iter().map(|e| e.id).collect();
        assert_eq!(ids, [0, 3]);
        assert!(
            saves.load(Some(2), Ok).is_err(),
            "abandoned saves are no longer offered"
        );
        assert!(dir.join("2.json").exists(), "abandoned saves are retained");
        let good = fs::read_to_string(dir.join(LINEAGE)).unwrap();
        // A snapshot written before its listing is just an unlisted save.
        fs::write(dir.join("4.json"), "{}").unwrap();
        let reserved = good.replace("\"next\": 4", "\"next\": 5");
        fs::write(dir.join(LINEAGE), &reserved).unwrap();
        assert_eq!(saves.entries().unwrap().len(), 2);
        // Missing, emptied or older indexes would hide saves, so they are refused.
        for damaged in [
            r#"{"next": 1, "entries": [{"id": 0, "kind": "auto"}]}"#,
            r#"{"next": 0, "entries": []}"#,
        ] {
            fs::write(dir.join(LINEAGE), damaged).unwrap();
            assert!(saves
                .entries()
                .unwrap_err()
                .to_string()
                .contains("does not list every save"));
            assert!(saves.write(Kind::Manual, &snapshot).is_err());
        }
        fs::write(dir.join(LINEAGE), &reserved).unwrap();
        assert_eq!(saves.read(3).unwrap(), snapshot);
        let lineage = fs::read_to_string(dir.join(LINEAGE)).unwrap();
        let reordered = lineage.replacen("\"id\": 0", "\"id\": 9", 1);
        fs::write(dir.join(LINEAGE), reordered).unwrap();
        assert!(saves
            .entries()
            .unwrap_err()
            .to_string()
            .contains("out of order"));
        fs::remove_file(dir.join(LINEAGE)).unwrap();
        assert!(saves
            .entries()
            .unwrap_err()
            .to_string()
            .contains("does not list every save"));
        assert!(saves.write(Kind::Manual, &snapshot).is_err());
        assert!(!fs::read_dir(&dir).unwrap().any(|f| f
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "tmp")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn saves_from_an_older_format_are_refused_by_version() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap();
        let dir = std::env::temp_dir().join(format!("realmkit-oldsave-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let saves = Saves::open(&dir).unwrap();
        saves
            .write(Kind::Auto, &Engine::new(&world).unwrap().snapshot())
            .unwrap();
        let old = r#"{"save_format_version": 1, "state": {"player": {"hp": 24}}}"#;
        fs::write(dir.join("0.json"), old).unwrap();
        let error = saves.read(0).unwrap_err().to_string();
        assert!(
            error.contains("unsupported save format version 1"),
            "{error}"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn only_one_game_uses_a_saves_directory_at_a_time() {
        let dir = std::env::temp_dir().join(format!("realmkit-exclusive-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let first = Saves::open(&dir).unwrap();
        let refused = Saves::open(&dir).err().unwrap().to_string();
        assert!(refused.contains("another game is using"), "{refused}");
        drop(first);
        Saves::open(&dir).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
