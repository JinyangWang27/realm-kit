//! Local save files: one JSON snapshot per save plus `lineage.json`, the active
//! recovery chain, oldest first. Loading an older save forks the chain; saves
//! from the abandoned future stay on disk but are no longer offered.

use realmkit_engine::SaveSnapshot;
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

pub struct Saves {
    dir: PathBuf,
}

impl Saves {
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
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
        write_atomic(&self.file(id), &serde_json::to_vec_pretty(snapshot)?)?;
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
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()).into())
    }

    /// Makes the save at `index` the newest in the chain.
    pub fn fork(&self, index: usize) -> Result<(), Box<dyn Error>> {
        let mut lineage = self.lineage()?;
        lineage.entries.truncate(index + 1);
        write_atomic(
            &self.dir.join(LINEAGE),
            &serde_json::to_vec_pretty(&lineage)?,
        )?;
        Ok(())
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
        saves.fork(0).unwrap();
        saves.write(Kind::Auto, &snapshot).unwrap();
        let ids: Vec<_> = saves.entries().unwrap().iter().map(|e| e.id).collect();
        assert_eq!(ids, [0, 3]);
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
        assert!(!fs::read_dir(&dir)
            .unwrap()
            .any(|f| f.unwrap().path().extension().unwrap() == "tmp"));
        fs::remove_dir_all(dir).unwrap();
    }
}
