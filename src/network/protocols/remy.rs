use crate::{
    average::EWMA,
    flow::{Flow, FlowNeverActive, FlowProperties},
    logging::Logger,
    network::toggler::Toggle,
    simulation::{Component, ComponentId, EffectContext, HasVariant, MaybeHasVariant, Message},
    time::{Time, TimeSpan},
    trainers::remy::{
        rule_tree::{Action, Point},
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
struct Behavior<'a> {
    dna: &'a RemyDna,
    last_ack: Option<Time>,
    last_send: Option<Time>,
    ack_ewma: EWMA<TimeSpan>,
    send_ewma: EWMA<TimeSpan>,
    rtt: Option<Rtt>,
}

impl Behavior<'_> {
    fn new(dna: &RemyDna) -> Behavior {
        Behavior {
            dna,
            ack_ewma: EWMA::new(1. / 8.),
            send_ewma: EWMA::new(1. / 8.),
            last_ack: None,
            last_send: None,
            rtt: None,
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
        self.dna.action(&self.point())
    }
}

impl<'a, L> LossyWindowBehavior<'a, L> for Behavior<'a>
where
    L: Logger,
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
pub struct LossySender<'a, L>(LossyWindowSender<'a, Behavior<'a>, L>)
where
    L: Logger;

impl<'a, L> LossySender<'a, L>
where
    L: Logger,
{
    pub fn new(
        id: ComponentId,
        link: ComponentId,
        destination: ComponentId,
        dna: &'a RemyDna,
        wait_for_enable: bool,
        logger: L,
    ) -> LossySender<'a, L> {
        LossySender(LossyWindowSender::<'a, _, _>::new(
            id,
            link,
            destination,
            Box::new(|| Behavior::<'a>::new(dna)),
            wait_for_enable,
            logger,
        ))
    }
}

impl<E, L> Component<E> for LossySender<'_, L>
where
    E: HasVariant<Packet> + MaybeHasVariant<Toggle>,
    L: Logger,
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

impl<L> Flow for LossySender<'_, L>
where
    L: Logger,
{
    fn properties(&self, current_time: Time) -> Result<FlowProperties, FlowNeverActive> {
        self.0.properties(current_time)
    }
}
