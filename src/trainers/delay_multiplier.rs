use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::{
    ccas::delay_multiplier::DelayMultiplierCca,
    flow::UtilityFunction,
    util::{
        rand::{ContinuousDistribution, Rng},
        WithLifetime,
    },
    CcaTemplate, Dna, NetworkConfig, Trainer,
};

use super::genetic::{GeneticConfig, GeneticDna, GeneticTrainer};

#[derive(Serialize, Deserialize, Default)]
pub struct DelayMultiplierConfig {
    genetic_config: GeneticConfig,
}

#[derive(Debug, Default)]
pub struct DelayMultiplierCcaTemplate;

impl<'a> CcaTemplate<'a> for DelayMultiplierCcaTemplate {
    type Policy = &'a DelayMultiplierDna;
    type CCA = DelayMultiplierCca;

    fn with(&self, policy: &'a DelayMultiplierDna) -> impl Fn() -> DelayMultiplierCca + Sync {
        || DelayMultiplierCca::new(policy.multiplier, 1. / 8.)
    }
}

impl WithLifetime for DelayMultiplierCcaTemplate {
    type Type<'a> = DelayMultiplierCcaTemplate;
}

pub struct DelayMultiplierTrainer {
    genetic_trainer: GeneticTrainer<DelayMultiplierTrainer>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DelayMultiplierDna {
    pub multiplier: f64,
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
    type CcaTemplate<'a> = DelayMultiplierCcaTemplate;

    fn new(config: &Self::Config) -> Self {
        DelayMultiplierTrainer {
            genetic_trainer: GeneticTrainer::new(&config.genetic_config),
        }
    }

    fn train<G, H>(
        &self,
        starting_point: Option<DelayMultiplierDna>,
        network_config: &impl NetworkConfig<G>,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> DelayMultiplierDna
    where
        H: crate::ProgressHandler<DelayMultiplierDna>,
        G: WithLifetime,
    {
        self.genetic_trainer.train(
            starting_point,
            network_config,
            utility_function,
            progress_handler,
            rng,
        )
    }
}
