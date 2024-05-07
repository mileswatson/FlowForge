use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::{
    ccas::delay_multiplier::DelayMultiplierCca,
    util::{
        rand::{ContinuousDistribution, Rng},
        OfLifetime,
    },
    CcaTemplate, Dna,
};

use super::genetic::{GeneticConfig, GeneticPolicy, GeneticTrainer};

#[derive(Serialize, Deserialize, Default)]
pub struct DelayMultiplierTrainer {
    genetic_config: GeneticConfig,
}

#[derive(Debug, Default)]
pub struct DelayMultiplierCcaTemplate;

impl<'a> CcaTemplate<'a> for DelayMultiplierCcaTemplate {
    type Policy = &'a DelayMultiplierDna;
    type Cca = DelayMultiplierCca;

    fn with(&self, policy: &'a DelayMultiplierDna) -> impl Fn() -> DelayMultiplierCca + Sync {
        || DelayMultiplierCca::new(policy.multiplier, 1. / 8.)
    }
}

impl OfLifetime for DelayMultiplierCcaTemplate {
    type Of<'a> = DelayMultiplierCcaTemplate;
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

impl GeneticPolicy for DelayMultiplierDna {
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

impl GeneticTrainer for DelayMultiplierTrainer {
    type Policy = DelayMultiplierDna;
    type CcaTemplate<'a> = DelayMultiplierCcaTemplate;

    fn genetic_config(&self) -> GeneticConfig {
        todo!()
    }
}
