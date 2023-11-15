use std::{cell::RefCell, rc::Rc};

use serde::{Deserialize, Serialize};

use crate::{
    logging::NothingLogger,
    network::protocols::delay_multiplier::{Packet, Receiver, Sender},
    rand::{ContinuousDistribution, Rng},
    simulation::DynComponent,
    time::TimeSpan,
    Dna, Trainer,
};

use super::genetic::{self, GeneticConfig, GeneticDna, GeneticTrainer};

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

    fn generate_components(
        &self,
        sim_properties: crate::network::SimProperties,
        rng: &mut Rng,
    ) -> genetic::Components<Packet> {
        let senders: Vec<Rc<_>> = sim_properties
            .sender_ids
            .into_iter()
            .map(|id| {
                Rc::new(RefCell::new(Sender::new::<Packet>(
                    id,
                    sim_properties.sender_link_id,
                    sim_properties.receiver_id,
                    self.multiplier,
                    TimeSpan::new(
                        rng.sample(&ContinuousDistribution::Uniform { min: 0., max: 10. }),
                    ),
                    NothingLogger,
                )))
            })
            .collect();
        #[allow(clippy::cast_precision_loss)]
        genetic::Components {
            senders: senders
                .iter()
                .map(|x| DynComponent::shared(x.clone()))
                .collect(),
            receiver: DynComponent::owned(Box::new(Receiver::new::<Packet>(
                sim_properties.receiver_link_id,
                NothingLogger,
            ))),
            get_score: Box::new(move || {
                /*senders
                .iter()
                .map(|s| s.borrow().packets() as f64)
                .sum::<f64>()
                / senders.len() as f64*/
                senders.iter().map(|s| s.borrow().packets()).min().unwrap() as f64
            }),
        }
    }

    fn spawn_child(&self, rng: &mut Rng) -> Self {
        DelayMultiplierDna {
            multiplier: self.multiplier
                * rng.sample(&ContinuousDistribution::Uniform { min: 0.9, max: 1.1 }),
        }
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
