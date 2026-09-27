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

    fn lineage(&self) -> Result<Lineage, Box<dyn Error>> {
        let path = self.dir.join(LINEAGE);
        match fs::read(&path) {
            Ok(bytes) => {
                let lineage: Lineage = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                // Chronological order decides the newest save; never guess past damage.
                let ids: Vec<_> = lineage.entries.iter().map(|e| e.id).collect();
                if ids.windows(2).all(|w| w[0] < w[1])
                    && ids.last().is_none_or(|id| *id < lineage.next)
                {
                    Ok(lineage)
                } else {
                    Err(format!("{} lists saves out of order", path.display()).into())
                }
            }
            // A missing index is a new directory only if it holds no saves;
            // otherwise report it rather than hide existing progress.
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let orphaned = fs::read_dir(&self.dir)
                    .map_err(|e| format!("{}: {e}", self.dir.display()))?
                    .filter_map(Result::ok)
                    .any(|f| {
                        let path = f.path();
                        path.extension().is_some_and(|x| x == "json")
                            && path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .is_some_and(|s| s.parse::<u64>().is_ok())
                    });
                if orphaned {
                    Err(format!("{} is missing but saves exist", path.display()).into())
                } else {
                    Ok(Lineage::default())
                }
            }
            Err(e) => Err(format!("{}: {e}", path.display()).into()),
        }
    }

    pub fn entries(&self) -> Result<Vec<Entry>, Box<dyn Error>> {
        Ok(self.lineage()?.entries)
    }

    /// Writes the snapshot before listing it, so a failed write leaves every
    /// earlier save and the chain untouched.
    pub fn write(&self, kind: Kind, snapshot: &SaveSnapshot) -> Result<(), Box<dyn Error>> {
        let mut lineage = self.lineage()?;
        let mut id = lineage.next;
        // A stale counter (say, lineage.json restored from a backup) must not
        // overwrite an existing save.
        while self.file(id).exists() {
            id = id.checked_add(1).ok_or("save counter exhausted")?;
        }
        lineage.next = id.checked_add(1).ok_or("save counter exhausted")?;
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
    // lineage while losing the snapshot it names.
    // ponytail: Unix only; Windows cannot open a directory as a file for syncing.
    #[cfg(unix)]
    fs::File::open(path.parent().unwrap_or(Path::new(".")))?.sync_all()?;
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
        // An older, self-consistent index (say, from a backup) must not
        // overwrite saves written after it.
        let stale = r#"{"next": 1, "entries": [{"id": 0, "kind": "auto"}]}"#;
        fs::write(dir.join(LINEAGE), stale).unwrap();
        saves.write(Kind::Manual, &snapshot).unwrap();
        let ids: Vec<_> = saves.entries().unwrap().iter().map(|e| e.id).collect();
        assert_eq!(ids, [0, 4], "a stale counter skips existing files");
        assert!(dir.join("2.json").exists(), "abandoned saves are retained");
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
        assert!(saves.entries().unwrap_err().to_string().contains("missing"));
        assert!(saves.write(Kind::Manual, &snapshot).is_err());
        assert!(!fs::read_dir(&dir)
            .unwrap()
            .any(|f| f.unwrap().path().extension().unwrap() == "tmp"));
        fs::remove_dir_all(dir).unwrap();
    }
}
