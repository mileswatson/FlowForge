#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::module_name_repetitions,
    clippy::use_self,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::suboptimal_flops,
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::too_many_arguments
)]

use std::{
    fmt::Debug,
    fs::File,
    io::{Read, Write},
    path::Path,
};

use anyhow::{anyhow, Result};
use rand_distr::Distribution;
use serde::{de::DeserializeOwned, Serialize};

use flow::UtilityFunction;
use quantities::{Float, Time};
use simulation::{Simulator, SimulatorBuilder};
use util::{
    logging::{Logger, NothingLogger},
    meters::FlowMeter,
    rand::Rng,
    WithLifetime,
};

#[macro_use]
pub mod util;
pub mod ccas;
pub mod components;
pub mod eval;
pub mod flow;
pub mod networks;
pub mod quantities;
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

pub trait Dna: Sized + Sync + Send + 'static {
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
    fn update_progress(&mut self, frac_complete: Float, current: &D);
}

impl<D: Dna, F: FnMut(Float, &D) + Send> ProgressHandler<D> for F {
    fn update_progress(&mut self, frac_complete: Float, current: &D) {
        self(frac_complete, current);
    }
}

pub trait NetworkBuilder<G>: Clone + Send
where
    G: WithLifetime,
{
    fn populate_sim<'sim, 'a, C, F>(
        &self,
        builder: SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        new_cca: impl Fn() -> C + Clone + 'a,
        rng: &'a mut Rng,
        new_flow_meter: impl FnMut() -> F,
    ) -> Simulator<'sim, 'a, G::Type<'sim>, NothingLogger>
    where
        C: Cca + 'a,
        F: FlowMeter + 'a,
        'sim: 'a;
}

pub trait NetworkConfig<G>:
    Serialize + DeserializeOwned + Distribution<Self::NetworkBuilder> + Sync
where
    G: WithLifetime,
{
    type NetworkBuilder: NetworkBuilder<G>;
}

pub trait Cca: Debug {
    #[must_use]
    fn initial_cwnd(&self, time: Time) -> u32;
    fn next_tick(&self, time: Time) -> Option<Time>;
    #[must_use]
    fn tick<L: Logger>(&mut self, rng: &mut Rng, logger: &mut L) -> u32;
    #[must_use]
    fn packet_sent<L: Logger>(&mut self, packet: PacketSent, rng: &mut Rng, logger: &mut L) -> u32;
    #[must_use]
    fn ack_received<L: Logger>(&mut self, ack: AckReceived, rng: &mut Rng, logger: &mut L) -> u32;
}

pub trait CcaTemplate<'a>: Default + Debug {
    type Policy: 'a + ?Sized;
    type CCA: Cca + 'a;
    fn with(&self, policy: Self::Policy) -> impl Fn() -> Self::CCA + Sync;
}

pub struct AckReceived {
    pub sent_time: Time,
    pub received_time: Time,
}

pub struct PacketSent {
    pub sent_time: Time,
}

pub trait Trainer {
    type Config: Config<Json>;
    type Dna: Dna;
    type CcaTemplate<'a>: CcaTemplate<'a, Policy = &'a Self::Dna>;
    type DefaultEffectGenerator: WithLifetime;

    fn new(config: &Self::Config) -> Self;

    fn train<G, H>(
        &self,
        starting_point: Option<Self::Dna>,
        network_config: &impl NetworkConfig<G>,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> Self::Dna
    where
        H: ProgressHandler<Self::Dna>,
        G: WithLifetime;
}
