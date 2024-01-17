use std::{cell::RefCell, rc::Rc};

use serde::{Deserialize, Serialize};

use crate::{
    evaluator::PopulateComponents,
    flow::{Flow, FlowProperties, NoActiveFlows, UtilityFunction},
    logging::NothingLogger,
    network::{config::NetworkConfig, protocols::delay_multiplier::LossySender, NetworkSlots},
    quantities::Float,
    rand::{ContinuousDistribution, Rng},
    simulation::DynComponent,
    Dna, Trainer,
};

use super::genetic::{GeneticConfig, GeneticDna, GeneticTrainer};

#[derive(Serialize, Deserialize, Default)]
pub struct DelayMultiplierConfig {
    genetic_config: GeneticConfig,
}

pub struct DelayMultiplierTrainer {
    genetic_trainer: GeneticTrainer<DelayMultiplierDna>,
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

impl PopulateComponents for DelayMultiplierDna {
    fn populate_components<'sim>(
        &self,
        network_slots: NetworkSlots<'sim, '_, '_>,
        _rng: &mut Rng,
    ) -> Vec<Rc<dyn Flow + 'sim>> {
        network_slots
            .sender_slots
            .into_iter()
            .map(|slot| {
                let sender = Rc::new(RefCell::new(LossySender::<'sim>::new(
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

impl GeneticDna for DelayMultiplierDna {
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
}

impl Trainer for DelayMultiplierTrainer {
    type Config = DelayMultiplierConfig;
    type Dna = DelayMultiplierDna;

    fn new(config: &Self::Config) -> Self {
        DelayMultiplierTrainer {
            genetic_trainer: GeneticTrainer::new(&config.genetic_config),
        }
    }

    fn train<H>(
        &self,
        starting_point: Option<DelayMultiplierDna>,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> DelayMultiplierDna
    where
        H: crate::ProgressHandler<DelayMultiplierDna>,
    {
        self.genetic_trainer.train(
            starting_point,
            network_config,
            utility_function,
            progress_handler,
            rng,
        )
    }

    fn evaluate(
        &self,
        d: &DelayMultiplierDna,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        rng: &mut Rng,
    ) -> anyhow::Result<(Float, FlowProperties), NoActiveFlows> {
        self.genetic_trainer
            .evaluate(d, network_config, utility_function, rng)
    }
}