use std::fmt::Debug;

use itertools::Itertools;

use crate::{
    logging::Logger,
    network::toggler::Toggle,
    quantities::Time,
    simulation::{Component, EffectContext, Message, MessageDestination},
};

use super::{ControllerEffect, LossyWindowBehavior, SenderEffect, SettingsUpdate};

#[derive(Debug)]
enum LossyWindowControllerState<B> {
    Enabled(B),
    Disabled { wait_for_enable: bool },
}

pub struct LossyWindowController<'sim, 'a, B, E, L> {
    sender: MessageDestination<'sim, SenderEffect<'sim, E>, E>,
    new_behavior: Box<dyn (Fn() -> B) + 'a>,
    state: LossyWindowControllerState<B>,
    logger: L,
}

impl<'sim, 'a, B: Debug, E, L: Debug> Debug for LossyWindowController<'sim, 'a, B, E, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LossyWindowController")
            .field("sender", &self.sender)
            .field("state", &self.state)
            .field("logger", &self.logger)
            .finish()
    }
}

impl<'sim, 'a, B, E, L> LossyWindowController<'sim, 'a, B, E, L> {
    pub fn new(
        sender: MessageDestination<'sim, SenderEffect<'sim, E>, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> LossyWindowController<'sim, 'a, B, E, L> {
        LossyWindowController {
            sender,
            new_behavior,
            state: LossyWindowControllerState::Disabled { wait_for_enable },
            logger,
        }
    }
}

impl<'sim, 'a, B, E, L> Component<'sim, E> for LossyWindowController<'sim, 'a, B, E, L>
where
    B: LossyWindowBehavior,
    L: Logger,
{
    type Receive = ControllerEffect;

    fn next_tick(&self, time: Time) -> Option<Time> {
        if matches!(
            self.state,
            LossyWindowControllerState::Disabled {
                wait_for_enable: false,
            }
        ) {
            Some(time)
        } else {
            None
        }
    }

    fn tick(&mut self, context: EffectContext) -> Vec<Message<'sim, E>> {
        if matches!(
            self.state,
            LossyWindowControllerState::Disabled {
                wait_for_enable: false,
            }
        ) {
            self.receive(ControllerEffect::Toggle(Toggle::Enable), context)
        } else {
            panic!()
        }
    }

    fn receive(&mut self, e: Self::Receive, _context: EffectContext) -> Vec<Message<'sim, E>> {
        (match (&mut self.state, e) {
            (
                LossyWindowControllerState::Disabled { .. },
                ControllerEffect::Toggle(Toggle::Enable),
            ) => {
                let behavior = (self.new_behavior)();
                let initial_settings = behavior.initial_settings();
                self.state = LossyWindowControllerState::Enabled(behavior);
                Some(SettingsUpdate::Enable(initial_settings))
            }
            (LossyWindowControllerState::Enabled(_), ControllerEffect::Toggle(Toggle::Disable)) => {
                self.state = LossyWindowControllerState::Disabled {
                    wait_for_enable: true,
                };
                Some(SettingsUpdate::Disable)
            }
            (
                LossyWindowControllerState::Enabled(behavior),
                ControllerEffect::AckReceived(context),
            ) => behavior
                .ack_received(context, &mut self.logger)
                .map(SettingsUpdate::Enable),
            (LossyWindowControllerState::Disabled { .. }, ControllerEffect::AckReceived(_)) => None,
            _ => {
                panic!("Unexpected toggle!")
            }
        })
        .map(|x| self.sender.create_message(SenderEffect::SettingsUpdate(x)))
        .into_iter()
        .collect_vec()
    }
}
