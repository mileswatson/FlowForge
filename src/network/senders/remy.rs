use std::fmt::Debug;

use crate::{
    core::{
        logging::Logger,
        meters::EWMA,
        rand::{DiscreteDistribution, Rng},
    },
    protocols::remy::{action::Action, point::Point, rule_tree::DynRuleTree},
    quantities::{Time, TimeSpan},
};

use super::window::{AckReceived, Cca, LossyWindowSettings};

#[derive(Debug, Clone)]
struct Rtt {
    min: TimeSpan,
    current: TimeSpan,
}

pub struct RemyCca<T> {
    rule_tree: T,
    last_ack: Option<Time>,
    last_send: Option<Time>,
    ack_ewma: EWMA<TimeSpan>,
    send_ewma: EWMA<TimeSpan>,
    rtt: Option<Rtt>,
    next_change: Option<(u32, Action)>,
    repeat_actions: Option<DiscreteDistribution<u32>>,
}

impl<T> Debug for RemyCca<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemyCca")
            .field("rule_tree", &"")
            .field("last_ack", &self.last_ack)
            .field("last_send", &self.last_send)
            .field("ack_ewma", &self.ack_ewma)
            .field("send_ewma", &self.send_ewma)
            .field("rtt", &self.rtt)
            .field("next_change", &self.next_change)
            .field("repeat_updates", &self.repeat_actions)
            .finish()
    }
}

impl<T> RemyCca<T>
where
    T: DynRuleTree,
{
    pub fn new(rule_tree: T, repeat_actions: Option<DiscreteDistribution<u32>>) -> RemyCca<T> {
        RemyCca {
            rule_tree,
            ack_ewma: EWMA::new(1. / 8.),
            send_ewma: EWMA::new(1. / 8.),
            last_ack: None,
            last_send: None,
            rtt: None,
            next_change: None,
            repeat_actions,
        }
    }

    fn point(&self) -> Point {
        Point {
            ack_ewma: self.ack_ewma.value().unwrap_or(TimeSpan::ZERO),
            send_ewma: self.send_ewma.value().unwrap_or(TimeSpan::ZERO),
            rtt_ratio: self.rtt.as_ref().map_or(0., |rtt| rtt.current / rtt.min),
        }
    }

    fn action(&self, time: Time) -> Action {
        self.rule_tree
            .as_ref()
            .action(&self.point(), time)
            .unwrap_or_else(|| panic!("Expected {} to map to an action", self.point()))
    }
}

impl<T> Cca for RemyCca<T>
where
    T: DynRuleTree,
{
    fn initial_settings(&self) -> LossyWindowSettings {
        LossyWindowSettings {
            window: 1,
            intersend_delay: TimeSpan::ZERO,
        }
    }

    fn ack_received<L>(
        &mut self,
        AckReceived {
            current_settings,
            sent_time,
            received_time,
        }: AckReceived,
        rng: &mut Rng,
        logger: &mut L,
    ) -> Option<LossyWindowSettings>
    where
        L: Logger,
    {
        if let Some(last_ack) = self.last_ack {
            self.ack_ewma.update(received_time - last_ack);
        }
        if let Some(last_send) = self.last_send {
            self.send_ewma.update(sent_time - last_send);
        }
        self.last_ack = Some(received_time);
        self.last_send = Some(sent_time);
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
                let action = self.action(received_time);
                let a = action.as_ref().clone();
                self.next_change = self
                    .repeat_actions
                    .as_ref()
                    .map(|dist| (rng.sample(dist), a.clone()));
                a
            }
        };
        let window = action.apply_to(current_settings.window);
        Some(LossyWindowSettings {
            window,
            intersend_delay: action.intersend_delay,
        })
    }
}
