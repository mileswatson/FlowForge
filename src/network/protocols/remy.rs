use std::fmt::Debug;

use crate::{
    flow::{Flow, FlowNeverActive, FlowProperties},
    logging::Logger,
    meters::EWMA,
    network::{NetworkEffect, NetworkMessage},
    simulation::{Component, ComponentId, EffectContext},
    time::{Time, TimeSpan},
    trainers::remy::{action::Action, point::Point, rule_tree::RuleTree},
};

use super::window::lossy_window::{LossyWindowBehavior, LossyWindowSender, LossyWindowSettings};

#[derive(Debug, Clone)]
struct Rtt {
    min: TimeSpan,
    current: TimeSpan,
}

#[derive(Debug)]
struct Behavior<'a, T> {
    rule_tree: &'a T,
    last_ack: Option<Time>,
    last_send: Option<Time>,
    ack_ewma: EWMA<TimeSpan>,
    send_ewma: EWMA<TimeSpan>,
    rtt: Option<Rtt>,
}

impl<T> Behavior<'_, T>
where
    T: RuleTree,
{
    fn new(rule_tree: &T) -> Behavior<T> {
        Behavior {
            rule_tree,
            ack_ewma: EWMA::new(1. / 8.),
            send_ewma: EWMA::new(1. / 8.),
            last_ack: None,
            last_send: None,
            rtt: None,
        }
    }

    fn point(&self) -> Point {
        Point {
            ack_ewma: self.ack_ewma.value().unwrap_or(TimeSpan::new(0.)),
            send_ewma: self.send_ewma.value().unwrap_or(TimeSpan::new(0.)),
            rtt_ratio: self.rtt.as_ref().map_or(0., |rtt| rtt.current / rtt.min),
        }
    }

    fn action(&self) -> &Action {
        self.rule_tree
            .action(&self.point())
            .expect("point to map to an action")
    }
}

impl<'a, L, T> LossyWindowBehavior<'a, L> for Behavior<'a, T>
where
    L: Logger,
    T: RuleTree,
{
    fn initial_settings(&self) -> LossyWindowSettings {
        LossyWindowSettings {
            window: 1,
            intersend_delay: TimeSpan::new(0.),
        }
    }

    fn ack_received(
        &mut self,
        current: &mut LossyWindowSettings,
        sent_time: Time,
        received_time: Time,
        logger: &mut L,
    ) {
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
        let Action {
            window_multiplier,
            window_increment,
            intersend_delay: intersend_ms,
            ..
        } = self.action();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            current.window = u32::checked_add_signed(
                (f64::from(current.window) * window_multiplier) as u32,
                *window_increment,
            )
            .unwrap();
        }
        current.intersend_delay = *intersend_ms;
        log!(logger, "Action is {:?}", current);
    }
}

#[derive(Debug)]
pub struct LossySender<'a, L, T>(LossyWindowSender<'a, Behavior<'a, T>, L>)
where
    L: Logger,
    T: RuleTree;

impl<'a, L, T> LossySender<'a, L, T>
where
    L: Logger,
    T: RuleTree,
{
    pub fn new(
        id: ComponentId,
        link: ComponentId,
        destination: ComponentId,
        rule_tree: &'a T,
        wait_for_enable: bool,
        logger: L,
    ) -> LossySender<'a, L, T> {
        LossySender(LossyWindowSender::<'a, _, _>::new(
            id,
            link,
            destination,
            Box::new(move || Behavior::<'a>::new(rule_tree)),
            wait_for_enable,
            logger,
        ))
    }
}

impl<L, T> Component<NetworkEffect> for LossySender<'_, L, T>
where
    L: Logger,
    T: RuleTree,
{
    fn tick(&mut self, context: EffectContext) -> Vec<NetworkMessage> {
        self.0.tick(context)
    }

    fn receive(&mut self, e: NetworkEffect, context: EffectContext) -> Vec<NetworkMessage> {
        self.0.receive(e, context)
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        Component::next_tick(&self.0, time)
    }
}

impl<L, T> Flow for LossySender<'_, L, T>
where
    L: Logger,
    T: RuleTree,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.0.properties(current_time)
    }
}
