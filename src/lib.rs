use std::{fs::File, path::Path};

use anyhow::{anyhow, Result};
use network::Network;
use serde::{de::DeserializeOwned, Serialize};

#[warn(clippy::pedantic, clippy::nursery)]
#[allow(clippy::module_name_repetitions)]
pub mod network;
pub mod rand;
pub mod trainers;

pub trait Config: Serialize + DeserializeOwned {
    fn to_json_file(&self, path: &Path) -> Result<()>;

    fn from_json_file(path: &Path) -> Result<Self>;
}

impl<T> Config for T
where
    T: Serialize + DeserializeOwned,
{
    fn to_json_file(&self, path: &Path) -> Result<()> {
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            return Err(anyhow!("Tried to write config to non-json file!"));
        }
        let mut file = File::create(path)?;
        Ok(serde_json::to_writer_pretty(&mut file, self)?)
    }

    fn from_json_file(path: &Path) -> Result<Self> {
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            return Err(anyhow!("Tried to read config from non-json file!"));
        }
        let file = File::open(path)?;
        Ok(serde_json::from_reader(file)?)
    }
}

pub trait Dna {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(buf: &[u8]) -> Self;
}

pub trait ProgressHandler<D: Dna> {
    fn update_progress(&mut self, d: &D);
}

impl<F: FnMut(&D), D: Dna> ProgressHandler<D> for F {
    fn update_progress(&mut self, d: &D) {
        self(d)
    }
}

pub trait Trainer {
    type DNA: Dna;
    type Config: Config;

    fn new(config: &Self::Config) -> Self;

    fn train<H: ProgressHandler<Self::DNA>>(
        &self,
        networks: &[Network],
        progress_handler: &mut H,
    ) -> Self::DNA;
}
