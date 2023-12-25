use std::fmt::Debug;

use crate::{
    average::EWMA,
    flow::{Flow, FlowNeverActive, FlowProperties},
    logging::Logger,
    network::toggler::Toggle,
    simulation::{Component, ComponentId, EffectContext, HasVariant, MaybeHasVariant, Message},
    time::{Time, TimeSpan},
    trainers::remy::{
        rule_tree::{Action, Point, RuleOverride},
        RemyDna,
    },
};

use super::window::lossy_window::{
    LossyWindowBehavior, LossyWindowSender, LossyWindowSettings, Packet,
};

#[derive(Debug, Clone)]
struct Rtt {
    min: TimeSpan,
    current: TimeSpan,
}

#[derive(Debug)]
struct Behavior<'a, O, const COUNT: bool>
where
    O: 'a,
{
    dna: &'a RemyDna,
    last_ack: Option<Time>,
    last_send: Option<Time>,
    ack_ewma: EWMA<TimeSpan>,
    send_ewma: EWMA<TimeSpan>,
    rtt: Option<Rtt>,
    rule_override: O,
}

impl<O, const COUNT: bool> Behavior<'_, O, COUNT>
where
    O: RuleOverride,
{
    fn new(dna: &RemyDna, rule_override: O) -> Behavior<'_, O, COUNT> {
        Behavior {
            dna,
            ack_ewma: EWMA::new(1. / 8.),
            send_ewma: EWMA::new(1. / 8.),
            last_ack: None,
            last_send: None,
            rtt: None,
            rule_override,
        }
    }

    fn point(&self) -> Point {
        Point {
            ack_ewma: self.ack_ewma.value().map_or(0., |t| t.value()),
            send_ewma: self.send_ewma.value().map_or(0., |t| t.value()),
            rtt_ratio: self.rtt.as_ref().map_or(0., |rtt| rtt.current / rtt.min),
        }
    }

    fn action(&self) -> &Action {
        self.dna
            .action::<O, COUNT>(&self.point(), &self.rule_override)
    }
}

impl<'a, L, O, const COUNT: bool> LossyWindowBehavior<'a, L> for Behavior<'a, O, COUNT>
where
    L: Logger,
    O: RuleOverride,
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
        self.last_send = Some(received_time);
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
            intersend_ms,
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
        current.intersend_delay = TimeSpan::new(*intersend_ms);
        log!(logger, "Action is {:?}", current);
    }
}

#[derive(Debug)]
pub struct LossySender<'a, L, O, const COUNT: bool>(
    LossyWindowSender<'a, Behavior<'a, O, COUNT>, L>,
)
where
    L: Logger,
    O: RuleOverride;

impl<'a, L, O, const COUNT: bool> LossySender<'a, L, O, COUNT>
where
    L: Logger,
    O: RuleOverride + 'a,
{
    pub fn new(
        id: ComponentId,
        link: ComponentId,
        destination: ComponentId,
        dna: &'a RemyDna,
        wait_for_enable: bool,
        rule_override: O,
        logger: L,
    ) -> LossySender<'a, L, O, COUNT> {
        LossySender(LossyWindowSender::<'a, _, _>::new(
            id,
            link,
            destination,
            Box::new(move || Behavior::<'a>::new(dna, rule_override.clone())),
            wait_for_enable,
            logger,
        ))
    }
}

impl<E, L, O, const COUNT: bool> Component<E> for LossySender<'_, L, O, COUNT>
where
    E: HasVariant<Packet> + MaybeHasVariant<Toggle>,
    L: Logger,
    O: RuleOverride,
{
    fn tick(&mut self, context: EffectContext) -> Vec<Message<E>> {
        self.0.tick(context)
    }

    fn receive(&mut self, e: E, context: EffectContext) -> Vec<Message<E>> {
        self.0.receive(e, context)
    }

    fn next_tick(&self, time: Time) -> Option<Time> {
        Component::<E>::next_tick(&self.0, time)
    }
}

impl<L, O, const COUNT: bool> Flow for LossySender<'_, L, O, COUNT>
where
    L: Logger,
    O: RuleOverride,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.0.properties(current_time)
    }
}
