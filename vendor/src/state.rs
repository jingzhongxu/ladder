use crate::error::{Error, ErrorKind, ResultExt};
use serde_json;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, PartialEq, Default, Clone)]
pub struct State {
    pub ingress: u64,
    pub egress: u64,
    pub deposit: u64,
    pub withdraw: u64,
    pub authority: u64,
}

pub struct StateStorage {
    pub file_path: PathBuf,
    pub state: State,
}

impl StateStorage {
    pub fn save(&mut self, state: &State) -> Result<(), Error> {
        if self.state != *state {
            self.state = (*state).clone();
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&self.file_path)?;
            let json = serde_json::to_string(&self.state)?;
            file.write_all(json.as_bytes())?;
            file.flush()?;
        }
        Ok(())
    }

    pub fn load(file_path: &Path) -> Result<Self, Error> {
        let mut file = match fs::File::open(&file_path) {
            Ok(file) => file,
            Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
                return Err(ErrorKind::UnknownFile(format!("{:?}", file_path)).into())
            }
            Err(err) => return Err(err).chain_err(|| "Cannot open storage"),
        };

        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        let state: State = serde_json::from_str(&buffer).unwrap_or(State::default());
        Ok(Self {
            file_path: file_path.to_path_buf(),
            state: state,
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use super::*;
    use tempdir::TempDir;

    #[test]
    fn should_load_save() {
        // TODO
        let tmp_dir = TempDir::new("example").unwrap();
        let file_path = tmp_dir.path().join("test.json");

        let mut tmp_file = File::create(file_path).unwrap();
        let mut ss = StateStorage::load(file_path).unwrap();
        println!("{:?}", ss.state);
        let state = State {
            ingress: 10,
            egress: 10,
            deposit: 10,
            withdraw: 10,
            authority: 10,
        };
        ss.save(state).unwrap();
        drop(tmp_file);
        tmp_dir.close().unwrap();
    }
}
