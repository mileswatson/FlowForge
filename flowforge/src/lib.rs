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
use flow::{FlowProperties, NoActiveFlows, UtilityFunction};
use network::config::NetworkConfig;
use quantities::Float;
use rand::Rng;
use serde::{de::DeserializeOwned, Serialize};

#[macro_use]
pub mod logging;
pub mod average;
pub mod evaluator;
pub mod flow;
pub mod meters;
pub mod network;
pub mod quantities;
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

pub trait Dna: Sized + Send + Sync {
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

pub trait ProgressHandler<D: Dna>: Send {
    fn update_progress(&mut self, fraction_completed: Float, top_scorer: Option<&D>);
}

impl<D: Dna, F: FnMut(Float, Option<&D>) + Send> ProgressHandler<D> for F {
    fn update_progress(&mut self, fraction_completed: Float, top_scorer: Option<&D>) {
        self(fraction_completed, top_scorer);
    }
}

pub trait Trainer {
    type Config: Config<Json>;
    type Dna: Dna;

    fn new(config: &Self::Config) -> Self;

    fn train<H>(
        &self,
        starting_point: Option<Self::Dna>,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> Self::Dna
    where
        H: ProgressHandler<Self::Dna>;

    fn evaluate(
        &self,
        d: &Self::Dna,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows>;
}