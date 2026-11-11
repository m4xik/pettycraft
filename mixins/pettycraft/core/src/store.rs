//! The save file: `<world>/pettycraft/state.json`, versioned JSON beside the
//! world's own data. Vanilla never reads it; removing the mod leaves it
//! inert.

use crate::engine::WorldState;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const FORMAT: u32 = 1;
pub const DIR: &str = "pettycraft";
pub const FILE: &str = "state.json";

/// The state file of the world saved in `world_root`.
pub fn path_in(world_root: &Path) -> PathBuf {
    world_root.join(DIR).join(FILE)
}

#[derive(Serialize, Deserialize)]
struct SaveFile {
    format: u32,
    state: WorldState,
}

#[derive(Serialize)]
struct SaveFileRef<'a> {
    format: u32,
    state: &'a WorldState,
}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Corrupt(String),
    /// Written by a newer PETTYCRAFT.
    Newer(u32),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Corrupt(e) => write!(f, "unreadable state: {e}"),
            Self::Newer(v) => write!(f, "state format {v} is newer than {FORMAT}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn encode(state: &WorldState) -> String {
    serde_json::to_string_pretty(&SaveFileRef {
        format: FORMAT,
        state,
    })
    .expect("the state serializes")
}

pub fn decode(text: &str) -> Result<WorldState, StoreError> {
    #[derive(Deserialize)]
    struct Head {
        format: u32,
    }
    let head: Head = serde_json::from_str(text).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    if head.format > FORMAT {
        return Err(StoreError::Newer(head.format));
    }
    let file: SaveFile =
        serde_json::from_str(text).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    Ok(file.state)
}

/// Writes `state` to `path` through a temporary file and a rename, so a crash
/// mid-write leaves the previous save.
pub fn save(path: &Path, state: &WorldState) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, encode(state))?;
    fs::rename(&tmp, path)
}

/// The saved state, or none when the world has none yet.
pub fn load(path: &Path) -> Result<Option<WorldState>, StoreError> {
    match fs::read_to_string(path) {
        Ok(text) => decode(&text).map(Some),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// [`load`], but a corrupt file is moved aside (`state.json.corrupt`) and the
/// world starts with an empty state rather than failing. A file from a newer
/// version is left untouched and reported, so it is never overwritten: the
/// caller should then not save.
pub fn load_or_quarantine(path: &Path) -> Result<(WorldState, Option<String>), StoreError> {
    match load(path) {
        Ok(state) => Ok((state.unwrap_or_default(), None)),
        Err(StoreError::Corrupt(e)) => {
            let aside = path.with_extension("json.corrupt");
            fs::rename(path, &aside)?;
            Ok((
                WorldState::default(),
                Some(format!("{e}; moved to {}", aside.display())),
            ))
        }
        Err(e) => Err(e),
    }
}
