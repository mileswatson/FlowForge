use serde::{Deserialize, Serialize};

use crate::{
    flow::{FlowProperties, NoActiveFlows, UtilityFunction},
    logging::NothingLogger,
    network::{
        config::NetworkConfig,
        protocols::{
            delay_multiplier::LossyDelayMultiplierSender,
            window::{LossyInternalControllerEffect, LossyInternalSenderEffect, LossySenderEffect},
        },
        EffectTypeGenerator, Packet, PopulateComponents, PopulateComponentsResult,
    },
    quantities::Float,
    rand::{ContinuousDistribution, Rng},
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
    genetic_trainer: GeneticTrainer<DefaultEffect<'static>, DelayMultiplierDna>,
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

impl<G> PopulateComponents<G> for DelayMultiplierDna
where
    G: EffectTypeGenerator,
    for<'sim> G::Type<'sim>: HasSubEffect<LossySenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalSenderEffect<'sim, G::Type<'sim>>>
        + HasSubEffect<LossyInternalControllerEffect>,
{
    fn populate_components<'sim>(
        &self,
        num_senders: u32,
        simulator_builder: &mut SimulatorBuilder<'sim, 'sim, G::Type<'sim>>,
        sender_link_id: Address<'sim, Packet<'sim, G::Type<'sim>>, G::Type<'sim>>,
        _rng: &mut Rng,
    ) -> PopulateComponentsResult<'sim, 'sim, G::Type<'sim>>
    where
        G::Type<'sim>: 'sim,
    {
        let (senders, flows) = (0..num_senders)
            .map(|_| {
                let slot =
                    LossyDelayMultiplierSender::reserve_slot::<_, NothingLogger>(simulator_builder);
                let address = slot.address();
                let packet_address = address.clone().cast();
                let (_, flow) = slot.set(
                    packet_address.clone(),
                    sender_link_id.clone(),
                    packet_address,
                    self.multiplier,
                    true,
                    NothingLogger,
                );
                (address.cast(), flow)
            })
            .unzip();
        PopulateComponentsResult { senders, flows }
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
