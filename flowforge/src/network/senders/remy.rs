use std::fmt::Debug;

use crate::{
    core::{
        logging::Logger,
        meters::{FlowMeter, EWMA},
    },
    network::PacketAddress,
    protocols::remy::{action::Action, point::Point, rule_tree::RuleTree},
    quantities::{Time, TimeSpan},
    simulation::{HasSubEffect, SimulatorBuilder},
};

use super::window::{
    AckReceived, LossyInternalControllerEffect, LossyInternalSenderEffect, LossySenderAddress,
    LossySenderSlot, LossyWindowBehavior, LossyWindowSender, LossyWindowSettings,
};

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
            ack_ewma: self.ack_ewma.value().unwrap_or(TimeSpan::ZERO),
            send_ewma: self.send_ewma.value().unwrap_or(TimeSpan::ZERO),
            rtt_ratio: self.rtt.as_ref().map_or(0., |rtt| rtt.current / rtt.min),
        }
    }

    fn action(&self) -> T::Action<'_> {
        self.rule_tree
            .action(&self.point())
            .unwrap_or_else(|| panic!("Expected {} to map to an action", self.point()))
    }
}

impl<'a, T> LossyWindowBehavior for Behavior<'a, T>
where
    T: RuleTree,
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
        let action = self.action();
        let Action {
            window_multiplier,
            window_increment,
            intersend_delay: intersend_ms,
            ..
        } = action.as_ref();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let window = ((f64::from(current_settings.window) * window_multiplier) as i32
            + *window_increment)
            .clamp(0, 1_000_000) as u32;
        let intersend_delay = *intersend_ms;
        Some(LossyWindowSettings {
            window,
            intersend_delay,
        })
    }
}

pub struct LossyRemySender;

pub struct LossyRemySenderSlot<'sim, 'a, 'b, E>(LossySenderSlot<'sim, 'a, 'b, E>);

impl<'sim, 'a, 'b, E> LossyRemySenderSlot<'sim, 'a, 'b, E>
where
    E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
        + HasSubEffect<LossyInternalControllerEffect>
        + 'sim,
    'sim: 'a,
{
    #[must_use]
    pub fn address(&self) -> LossySenderAddress<'sim, E> {
        self.0.address()
    }

    pub fn set<T, F>(
        self,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        rule_tree: &'a T,
        wait_for_enable: bool,
        flow_meter: F,
        logger: impl Logger + Clone + 'a,
    ) -> LossySenderAddress<'sim, E>
    where
        T: RuleTree,
        F: FlowMeter + 'a,
    {
        self.0.set(
            id,
            link,
            destination,
            Box::new(move || Behavior::<'a>::new(rule_tree)),
            wait_for_enable,
            flow_meter,
            logger,
        )
    }
}

impl LossyRemySender {
    pub fn reserve_slot<'sim, 'a, 'b, E, L>(
        builder: &'b SimulatorBuilder<'sim, 'a, E>,
    ) -> LossyRemySenderSlot<'sim, 'a, 'b, E>
    where
        L: Logger + Clone + 'a,
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
        'sim: 'a,
    {
        LossyRemySenderSlot(LossyWindowSender::reserve_slot(builder))
    }

    pub fn insert<'sim, 'a, 'b, T, F, E, L>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        rule_tree: &'a T,
        wait_for_enable: bool,
        flow_meter: F,
        logger: L,
    ) -> LossySenderAddress<'sim, E>
    where
        T: RuleTree,
        L: Logger + Clone + 'a,
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
        F: FlowMeter + 'a,
        'sim: 'a,
    {
        let slot = Self::reserve_slot::<E, L>(builder);
        slot.set(
            id,
            link,
            destination,
            rule_tree,
            wait_for_enable,
            flow_meter,
            logger,
        )
    }
}
