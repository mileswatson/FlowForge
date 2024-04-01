use std::fmt::Debug;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        logging::NothingLogger,
        meters::FlowMeter,
        rand::{ContinuousDistribution, Rng},
    },
    flow::UtilityFunction,
    network::{
        config::NetworkConfig,
        senders::{
            delay_multiplier::LossyDelayMultiplierSender,
            window::{LossyInternalControllerEffect, LossyInternalSenderEffect, LossySenderEffect},
        },
        toggler::Toggle,
        AddFlows, EffectTypeGenerator, Packet,
    },
    simulation::{Address, HasSubEffect, SimulatorBuilder},
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

pub struct DelayMultiplierTrainer {
    genetic_trainer:
        GeneticTrainer<DelayMultiplierFlowAdder, DelayMultiplierDna, DefaultEffect<'static>>,
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

#[derive(Default)]
pub struct DelayMultiplierFlowAdder;

impl<'d, G> AddFlows<&'d DelayMultiplierDna, G> for DelayMultiplierFlowAdder
where
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasSubEffect<LossySenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalSenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalControllerEffect>,
{
    fn add_flows<'sim, 'a, F>(
        &self,
        dna: &DelayMultiplierDna,
        flows: impl IntoIterator<Item = F>,
        simulator_builder: &mut SimulatorBuilder<'sim, 'a, G::Type<'sim>>,
        sender_link_id: Address<'sim, Packet<'sim, G::Type<'sim>>, G::Type<'sim>>,
        _rng: &mut Rng,
    ) -> Vec<Address<'sim, Toggle, G::Type<'sim>>>
    where
        F: FlowMeter + 'a,
        G::Type<'sim>: 'sim,
        'sim: 'a,
    {
        let senders = flows
            .into_iter()
            .map(|flow| {
                let slot =
                    LossyDelayMultiplierSender::reserve_slot::<_, NothingLogger>(simulator_builder);
                let address = slot.address();
                let packet_address = address.clone().cast();
                let _ = slot.set(
                    packet_address.clone(),
                    sender_link_id.clone(),
                    packet_address,
                    dna.multiplier,
                    true,
                    flow,
                    NothingLogger,
                );
                address.cast()
            })
            .collect_vec();
        senders
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
    type DefaultEffectGenerator = DefaultEffect<'static>;
    type DefaultFlowAdder<'a> = DelayMultiplierFlowAdder;

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
