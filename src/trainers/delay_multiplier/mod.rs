use std::{cell::RefCell, rc::Rc};

use serde::{Deserialize, Serialize};

use crate::{
    logging::NothingLogger,
    network::{
        protocols::delay_multiplier::{Packet, Receiver, Sender},
        NetworkSlots,
    },
    rand::{ContinuousDistribution, Rng},
    simulation::DynComponent,
    time::TimeSpan,
    Dna, Trainer,
};

use super::genetic::{GeneticConfig, GeneticDna, GeneticTrainer};

#[derive(Serialize, Deserialize, Default)]
pub struct DelayMultiplierConfig {
    genetic_config: GeneticConfig,
}

pub struct DelayMultiplierTrainer {
    genetic_trainer: GeneticTrainer<Packet, Packet>,
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

impl GeneticDna<Packet> for DelayMultiplierDna {
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
        network_slots: NetworkSlots<Packet>,
        rng: &mut Rng,
    ) -> Box<dyn FnOnce() -> crate::time::Float> {
        let senders: Vec<Rc<_>> = network_slots
            .sender_slots
            .into_iter()
            .map(|slot| {
                let sender = Rc::new(RefCell::new(Sender::new::<Packet>(
                    slot.id(),
                    network_slots.sender_link_id,
                    network_slots.receiver_slot.id(),
                    self.multiplier,
                    TimeSpan::new(
                        rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 10. }),
                    ),
                    NothingLogger,
                )));
                slot.set(DynComponent::shared(sender.clone()));
                sender
            })
            .collect();
        network_slots
            .receiver_slot
            .set(DynComponent::owned(Box::new(Receiver::new::<Packet>(
                network_slots.receiver_link_id,
                NothingLogger,
            ))));
        #[allow(clippy::cast_precision_loss)]
        Box::new(move || {
            /*senders
            .iter()
            .map(|s| s.borrow().packets() as f64)
            .sum::<f64>()
            / senders.len() as f64*/
            senders.iter().map(|s| s.borrow().packets()).min().unwrap() as f64
        })
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
        networks: &[crate::network::Network],
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> DelayMultiplierDna
    where
        H: crate::ProgressHandler<DelayMultiplierDna>,
    {
        self.genetic_trainer.train(networks, progress_handler, rng)
    }
}
