use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::{
    core::{
        meters::EWMA,
        rand::{ContinuousDistribution, Rng},
    },
    flow::UtilityFunction,
    network::{
        config::NetworkConfig,
        senders::{
            delay_multiplier::DelayMultiplierCca,
            window::{
                CcaTemplate, LossyInternalControllerEffect, LossyInternalSenderEffect,
                LossySenderEffect,
            },
        },
        EffectTypeGenerator,
    },
    simulation::HasSubEffect,
    Dna, Trainer,
};

use super::{
    genetic::{GeneticConfig, GeneticDna, GeneticTrainer},
    DefaultEffect,
};

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
        || DelayMultiplierCca {
            multiplier: policy.multiplier,
            rtt: EWMA::new(1. / 8.),
        }
    }
}

impl EffectTypeGenerator for DelayMultiplierCcaTemplate {
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

impl<G> GeneticDna<G> for DelayMultiplierDna
where
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasSubEffect<LossySenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalSenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalControllerEffect>,
{
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
    type DefaultEffectGenerator = DefaultEffect<'static>;

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
}
