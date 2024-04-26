use rand_distr::Distribution;
use serde::{Deserialize, Serialize};

use crate::{
    components::{
        link::Link,
        packet::Packet,
        senders::lossy::{LossySender, LossySenderEffect},
        toggler::{Toggle, Toggler},
    },
    quantities::{
        bits_per_second, milliseconds, seconds, Float, Information, InformationRate, TimeSpan,
    },
    simulation::{DynComponent, HasSubEffect, SimulatorBuilder},
    util::{
        logging::NothingLogger,
        meters::FlowMeter,
        never::Never,
        rand::{
            ContinuousDistribution, DiscreteDistribution, PositiveContinuousDistribution,
            ProbabilityDistribution, Rng,
        },
        WithLifetime,
    },
    Cca, Network, NetworkDistribution,
};

#[derive(Debug, Clone, Serialize)]
pub struct RemyNetworkBuilder {
    pub rtt: TimeSpan,
    pub packet_rate: InformationRate,
    pub loss_rate: Float,
    pub buffer_size: Option<Information>,
    pub num_senders: u32,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

pub trait HasRemyNetworkSubEffects<'sim, E>:
    HasSubEffect<LossySenderEffect<'sim, E>>
    + HasSubEffect<Packet<'sim, E>>
    + HasSubEffect<Toggle>
    + HasSubEffect<Never>
    + 'sim
{
}

impl<'sim, E, T> HasRemyNetworkSubEffects<'sim, E> for T where
    T: HasSubEffect<LossySenderEffect<'sim, E>>
        + HasSubEffect<Packet<'sim, E>>
        + HasSubEffect<Toggle>
        + HasSubEffect<Never>
        + 'sim
{
}

impl<G> Network<G> for RemyNetworkBuilder
where
    G: WithLifetime,
    for<'sim> G::Type<'sim>: HasRemyNetworkSubEffects<'sim, G::Type<'sim>>,
{
    fn populate_sim<'sim, 'a, C, F>(
        &self,
        builder: &SimulatorBuilder<'sim, 'a, <G>::Type<'sim>>,
        new_cca: impl Fn() -> C + Clone + 'a,
        rng: &'a mut Rng,
        mut new_flow_meter: impl FnMut() -> F,
    ) where
        C: Cca + 'a,
        F: FlowMeter + 'a,
        'sim: 'a,
    {
        let sender_link_id = builder.insert(DynComponent::Owned(Link::create(
            self.rtt,
            self.packet_rate,
            self.loss_rate,
            self.buffer_size,
            rng.create_child(),
            NothingLogger,
        )));
        for _ in 0..self.num_senders {
            let slot = builder.reserve_slot();
            let address = slot.address();
            let packet_address = address.clone().cast();
            slot.set(DynComponent::Owned(LossySender::new(
                packet_address.clone(),
                sender_link_id.clone(),
                packet_address,
                new_flow_meter(),
                new_cca.clone(),
                true,
                rng.create_child(),
                NothingLogger,
            )));
            builder.insert(DynComponent::Owned(Toggler::new(
                address.cast(),
                self.on_time.clone(),
                self.off_time.clone(),
                rng.create_child(),
            )));
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RemyNetworkConfig {
    pub rtt: PositiveContinuousDistribution<TimeSpan>,
    pub bandwidth: PositiveContinuousDistribution<InformationRate>,
    pub loss_rate: ProbabilityDistribution,
    pub buffer_size: Option<DiscreteDistribution<Information>>,
    pub num_senders: DiscreteDistribution<u32>,
    pub off_time: PositiveContinuousDistribution<TimeSpan>,
    pub on_time: PositiveContinuousDistribution<TimeSpan>,
}

impl Default for RemyNetworkConfig {
    fn default() -> RemyNetworkConfig {
        RemyNetworkConfig {
            rtt: PositiveContinuousDistribution(ContinuousDistribution::Uniform {
                min: milliseconds(100.),
                max: milliseconds(200.),
            }),
            bandwidth: PositiveContinuousDistribution(ContinuousDistribution::Uniform {
                min: bits_per_second(10_000_000.),
                max: bits_per_second(20_000_000.),
            }),
            loss_rate: ProbabilityDistribution(ContinuousDistribution::Always { value: 0. }),
            buffer_size: None,
            num_senders: DiscreteDistribution::Uniform { min: 1, max: 16 },
            off_time: PositiveContinuousDistribution(ContinuousDistribution::Exponential {
                mean: seconds(5.),
            }),
            on_time: PositiveContinuousDistribution(ContinuousDistribution::Exponential {
                mean: seconds(5.),
            }),
        }
    }
}

impl Distribution<RemyNetworkBuilder> for RemyNetworkConfig {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> RemyNetworkBuilder {
        RemyNetworkBuilder {
            rtt: rng.sample(&self.rtt),
            packet_rate: rng.sample(&self.bandwidth),
            loss_rate: rng.sample(&self.loss_rate),
            buffer_size: self.buffer_size.as_ref().map(|d| rng.sample(d)),
            num_senders: rng.sample(&self.num_senders),
            off_time: self.off_time.clone(),
            on_time: self.on_time.clone(),
        }
    }
}

impl<G> NetworkDistribution<G> for RemyNetworkConfig
where
    G: WithLifetime,
    for<'sim> G::Type<'sim>: HasRemyNetworkSubEffects<'sim, G::Type<'sim>>,
{
    type Network = RemyNetworkBuilder;
}
