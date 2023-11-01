#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::module_name_repetitions,
    clippy::use_self,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::suboptimal_flops
)]

use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use anyhow::{anyhow, Result};
use network::Network;
use serde::{de::DeserializeOwned, Serialize};

pub mod network;
pub mod rand;
pub mod simulation;
pub mod trainers;

pub struct Json;
pub struct Custom;

pub trait Config<T>: Sized {
    fn valid_path(path: &Path) -> bool;
    fn save(&self, path: &Path) -> Result<()>;
    fn load(path: &Path) -> Result<Self>;
}

impl<T> Config<Json> for T
where
    T: Serialize + DeserializeOwned,
{
    fn valid_path(path: &Path) -> bool {
        path.extension().is_some_and(|x| x.to_str() == Some("json"))
    }

    fn save(&self, path: &Path) -> Result<()> {
        if !Self::valid_path(path) {
            return Err(anyhow!("Tried to write config to non-json file!"));
        }
        let mut file = File::create(path)?;
        Ok(serde_json::to_writer_pretty(&mut file, self)?)
    }

    fn load(path: &Path) -> Result<Self> {
        if !Self::valid_path(path) {
            return Err(anyhow!("Tried to read config from non-json file!"));
        }
        let file = File::open(path)?;
        Ok(serde_json::from_reader(file)?)
    }
}

pub trait Dna: Sized {
    const NAME: &'static str;
    fn serialize(&self) -> Result<Vec<u8>>;
    fn deserialize(buf: &[u8]) -> Result<Self>;
}

impl<D: Dna> Config<Custom> for D {
    fn valid_path(path: &Path) -> bool {
        path.to_str()
            .is_some_and(|x| x.ends_with(&format!(".{}.dna", Self::NAME)))
    }

    fn save(&self, path: &Path) -> Result<()> {
        if !Self::valid_path(path) {
            return Err(anyhow!(
                "Tried to save DNA to file with non .{}.dna extension!",
                Self::NAME
            ));
        }
        let buf = self.serialize()?;
        let mut file = File::create(path)?;
        Ok(file.write_all(&buf)?)
    }

    fn load(path: &Path) -> Result<Self> {
        if !Self::valid_path(path) {
            return Err(anyhow!(
                "Tried to load DNA from file with non .{}.dna extension!",
                Self::NAME
            ));
        }
        let mut file = File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Self::deserialize(&buf)
    }
}

pub trait ProgressHandler<D: Dna> {
    fn update_progress(&mut self, d: &D);
}

impl<F: FnMut(&D), D: Dna> ProgressHandler<D> for F {
    fn update_progress(&mut self, d: &D) {
        self(d);
    }
}

pub trait Trainer {
    type DNA: Dna;
    type Config: Config<Json>;

    fn new(config: &Self::Config) -> Self;

    fn train<H: ProgressHandler<Self::DNA>>(
        &self,
        networks: &[Network],
        progress_handler: &mut H,
    ) -> Self::DNA;
}
