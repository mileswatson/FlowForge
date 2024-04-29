use std::{fmt::Debug, marker::PhantomData};

use derive_where::derive_where;

use crate::{
    quantities::{Time, TimeSpan},
    util::{
        logging::Logger,
        meters::EWMA,
        rand::{DiscreteDistribution, Rng},
    },
    AckReceived, Cca, CcaTemplate,
};

use self::{action::Action, point::Point};

pub mod action;
pub mod cube;
pub mod dna;
pub mod point;
pub mod rule_tree;

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Debug, Clone)]
struct Rtt {
    min: TimeSpan,
    current: TimeSpan,
}

pub struct RemyCca<T> {
    policy: T,
    last_ack: Option<Time>,
    last_ack_send: Option<Time>,
    ack_ewma: EWMA<TimeSpan>,
    send_ewma: EWMA<TimeSpan>,
    rtt: Option<Rtt>,
    next_change: Option<(u32, Action)>,
    repeat_actions: Option<DiscreteDistribution<u32>>,
    last_send: Option<Time>,
    current_settings: RemyCwndSettings,
}

impl<T> Debug for RemyCca<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemyCca")
            .field("policy", &"")
            .field("last_ack", &self.last_ack)
            .field("last_ack_send", &self.last_ack_send)
            .field("ack_ewma", &self.ack_ewma)
            .field("send_ewma", &self.send_ewma)
            .field("rtt", &self.rtt)
            .field("next_change", &self.next_change)
            .field("repeat_actions", &self.repeat_actions)
            .field("last_send", &self.last_send)
            .field("current_settings", &self.current_settings)
            .finish()
    }
}

impl<T> RemyCca<T>
where
    T: RemyPolicy,
{
    pub fn new(rule_tree: T, repeat_actions: Option<DiscreteDistribution<u32>>) -> RemyCca<T> {
        let settings = RemyCwndSettings::default();
        RemyCca {
            policy: rule_tree,
            ack_ewma: EWMA::new(1. / 8.),
            send_ewma: EWMA::new(1. / 8.),
            last_ack: None,
            last_ack_send: None,
            rtt: None,
            next_change: None,
            repeat_actions,
            last_send: None,
            current_settings: settings,
        }
    }

    fn point(&self) -> Point {
        Point {
            ack_ewma: self.ack_ewma.value().unwrap_or(TimeSpan::ZERO),
            send_ewma: self.send_ewma.value().unwrap_or(TimeSpan::ZERO),
            rtt_ratio: self.rtt.as_ref().map_or(0., |rtt| rtt.current / rtt.min),
        }
    }

    fn action(&self) -> Action {
        self.policy
            .action(&self.point())
            .unwrap_or_else(|| panic!("Expected {} to map to an action", self.point()))
    }

    const fn get_cwnd(&self) -> u32 {
        match self.last_send {
            Some(_) => 0,
            None => self.current_settings.cwnd,
        }
    }
}

#[derive(Debug)]
pub struct RemyCwndSettings {
    pub cwnd: u32,
    pub intersend_delay: TimeSpan,
}

impl Default for RemyCwndSettings {
    fn default() -> Self {
        Self {
            cwnd: 1,
            intersend_delay: TimeSpan::ZERO,
        }
    }
}

impl<T> Cca for RemyCca<T>
where
    T: RemyPolicy,
{
    fn initial_cwnd(&self, _time: Time) -> u32 {
        self.get_cwnd()
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        self.last_send
            .map(|t| time.max(t + self.current_settings.intersend_delay))
    }

    fn tick(&mut self, _rng: &mut Rng, _logger: &mut impl Logger) -> u32 {
        self.last_send = None;
        self.get_cwnd()
    }

    fn ack_received(
        &mut self,
        AckReceived {
            sent_time,
            received_time,
        }: AckReceived,
        rng: &mut Rng,
        logger: &mut impl Logger,
    ) -> u32 {
        if let Some(last_ack) = self.last_ack {
            self.ack_ewma.update(received_time - last_ack);
        }
        if let Some(last_ack_send) = self.last_ack_send {
            self.send_ewma.update(sent_time - last_ack_send);
        }
        self.last_ack = Some(received_time);
        self.last_ack_send = Some(sent_time);
        let current_rtt = received_time - sent_time;
        self.rtt = Some(Rtt {
            min: self.rtt.as_ref().map_or(current_rtt, |prev| {
                if prev.min < current_rtt {
                    prev.min
                } else {
                    current_rtt
                }
            }),
            current: current_rtt,
        });
        log!(logger, "Updated state to {:?}", self);

        let action = match &mut self.next_change {
            Some((remaining, a)) => {
                let a = a.clone();
                if *remaining == 0 {
                    self.next_change = None;
                } else {
                    *remaining -= 1;
                }
                a
            }
            None => {
                let action = self.action();
                let a = action.as_ref().clone();
                self.next_change = self
                    .repeat_actions
                    .as_ref()
                    .map(|dist| (rng.sample(dist), a.clone()));
                a
            }
        };
        self.current_settings = RemyCwndSettings {
            cwnd: action.apply_to(self.current_settings.cwnd),
            intersend_delay: action.intersend_delay,
        };
        self.get_cwnd()
    }

    fn packet_sent(
        &mut self,
        packet: crate::PacketSent,
        _rng: &mut Rng,
        _logger: &mut impl Logger,
    ) -> u32 {
        self.last_send = Some(packet.sent_time);
        self.get_cwnd()
    }
}

#[derive_where(Default, Debug)]
pub struct RemyCcaTemplate<T> {
    repeat_actions: Option<DiscreteDistribution<u32>>,
    rule_tree: PhantomData<T>,
}

impl<T> RemyCcaTemplate<T> {
    #[must_use]
    pub const fn new(repeat_actions: Option<DiscreteDistribution<u32>>) -> RemyCcaTemplate<T> {
        RemyCcaTemplate {
            repeat_actions,
            rule_tree: PhantomData,
        }
    }
}

impl<T> RemyCcaTemplate<T>
where
    T: RemyPolicy + Clone,
{
    pub fn with_not_sync(&self, policy: T) -> impl Fn() -> RemyCca<T> {
        let repeat_actions = self.repeat_actions.clone();
        move || RemyCca::new(policy.clone(), repeat_actions.clone())
    }
}

impl<'a, T> CcaTemplate<'a> for RemyCcaTemplate<T>
where
    T: RemyPolicy + Clone + Sync + 'a,
{
    type Policy = T;

    type Cca = RemyCca<T>;

    fn with(&self, policy: T) -> impl Fn() -> RemyCca<T> + Sync {
        let repeat_actions = self.repeat_actions.clone();
        move || RemyCca::new(policy.clone(), repeat_actions.clone())
    }
}

pub trait RemyPolicy: Debug {
    fn action(&self, point: &Point) -> Option<Action>;
}

impl<T> RemyPolicy for &T
where
    T: RemyPolicy,
{
    fn action(&self, point: &Point) -> Option<Action> {
        T::action(self, point)
    }
}
