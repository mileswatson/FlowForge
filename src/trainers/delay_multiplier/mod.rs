use std::{cell::RefCell, rc::Rc};

use serde::{Deserialize, Serialize};

use crate::{
    flow::{Flow, UtilityFunction},
    logging::NothingLogger,
    network::{
        config::NetworkConfig,
        protocols::{delay_multiplier::LossySender, window::lossy_window::Packet},
        toggler::Toggle,
        NetworkSlots,
    },
    rand::{ContinuousDistribution, Rng},
    simulation::{DynComponent, MaybeHasVariant},
    Dna, Trainer,
};

use super::genetic::{GeneticConfig, GeneticDna, GeneticTrainer};

#[derive(Debug)]
pub enum DelayMultiplierPacket {
    Packet(Packet),
    Toggle(Toggle),
}

impl MaybeHasVariant<Toggle> for DelayMultiplierPacket {
    fn try_into(self) -> Result<Toggle, Self> {
        match self {
            DelayMultiplierPacket::Packet(_) => Err(self),
            DelayMultiplierPacket::Toggle(t) => Ok(t),
        }
    }
}

impl From<Toggle> for DelayMultiplierPacket {
    fn from(value: Toggle) -> Self {
        DelayMultiplierPacket::Toggle(value)
    }
}

impl MaybeHasVariant<Packet> for DelayMultiplierPacket {
    fn try_into(self) -> Result<Packet, Self> {
        match self {
            DelayMultiplierPacket::Packet(p) => Ok(p),
            DelayMultiplierPacket::Toggle(_) => Err(self),
        }
    }
}

impl From<Packet> for DelayMultiplierPacket {
    fn from(value: Packet) -> Self {
        DelayMultiplierPacket::Packet(value)
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct DelayMultiplierConfig {
    genetic_config: GeneticConfig,
}

pub struct DelayMultiplierTrainer {
    genetic_trainer: GeneticTrainer<DelayMultiplierPacket, Packet>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DelayMultiplierDna {
    multiplier: f64,
}

impl Dna for DelayMultiplierDna {
    const NAME: &'static str = "delaymultiplier";

    fn serialize(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    fn deserialize(buf: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(buf)?)
    }
}

impl GeneticDna<DelayMultiplierPacket> for DelayMultiplierDna {
    fn new_random(rng: &mut Rng) -> Self {
        DelayMultiplierDna {
            multiplier: rng.sample(&ContinuousDistribution::Uniform { min: 0.0, max: 5.0 }),
        }
    }

    fn spawn_child(&self, rng: &mut Rng) -> Self {
        DelayMultiplierDna {
            multiplier: self.multiplier
                * rng.sample(&ContinuousDistribution::Uniform { min: 0.9, max: 1.1 }),
        }
    }

    fn populate_components(
        &self,
        network_slots: NetworkSlots<DelayMultiplierPacket>,
        _rng: &mut Rng,
    ) -> Vec<Rc<dyn Flow>> {
        network_slots
            .sender_slots
            .into_iter()
            .map(|slot| {
                let sender = Rc::new(RefCell::new(LossySender::new(
                    slot.id(),
                    network_slots.sender_link_id,
                    slot.id(),
                    self.multiplier,
                    true,
                    NothingLogger,
                )));
                slot.set(DynComponent::shared(sender.clone()));
                sender as Rc<dyn Flow>
            })
            .collect()
    }
}

impl Trainer<DelayMultiplierDna> for DelayMultiplierTrainer {
    type Config = DelayMultiplierConfig;

    fn new(config: &Self::Config) -> Self {
        DelayMultiplierTrainer {
            genetic_trainer: Trainer::<DelayMultiplierDna>::new(&config.genetic_config),
        }
    }

    fn train<H>(
        &self,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> DelayMultiplierDna
    where
        H: crate::ProgressHandler<DelayMultiplierDna>,
    {
        self.genetic_trainer
            .train(network_config, utility_function, progress_handler, rng)
    }
}
