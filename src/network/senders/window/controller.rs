use std::fmt::Debug;

use itertools::Itertools;

use crate::{
    core::{logging::Logger, rand::Rng},
    network::toggler::Toggle,
    quantities::Time,
    simulation::{Address, Component, EffectContext, Message},
};

use super::{LossyInternalControllerEffect, LossyInternalSenderEffect, SettingsUpdate, CCA};

#[derive(Debug)]
enum LossyWindowControllerState<C> {
    Enabled(C),
    Disabled { wait_for_enable: bool },
}

pub struct LossyWindowController<'sim, 'a, C, E, L> {
    sender: Address<'sim, LossyInternalSenderEffect<'sim, E>, E>,
    new_cca: Box<dyn (Fn() -> C) + 'a>,
    state: LossyWindowControllerState<C>,
    rng: Rng,
    logger: L,
}

impl<'sim, 'a, C: Debug, E, L: Debug> Debug for LossyWindowController<'sim, 'a, C, E, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LossyWindowController")
            .field("sender", &self.sender)
            .field("state", &self.state)
            .field("logger", &self.logger)
            .finish()
    }
}

impl<'sim, 'a, C, E, L> LossyWindowController<'sim, 'a, C, E, L> {
    pub fn new(
        sender: Address<'sim, LossyInternalSenderEffect<'sim, E>, E>,
        new_cca: Box<dyn (Fn() -> C) + 'a>,
        wait_for_enable: bool,
        rng: Rng,
        logger: L,
    ) -> LossyWindowController<'sim, 'a, C, E, L> {
        LossyWindowController {
            sender,
            new_cca,
            state: LossyWindowControllerState::Disabled { wait_for_enable },
            rng,
            logger,
        }
    }
}

impl<'sim, 'a, C, E, L> Component<'sim, E> for LossyWindowController<'sim, 'a, C, E, L>
where
    C: CCA,
    L: Logger,
{
    type Receive = LossyInternalControllerEffect;

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
            self.receive(
                LossyInternalControllerEffect::Toggle(Toggle::Enable),
                context,
            )
        } else {
            panic!()
        }
    }

    fn receive(&mut self, e: Self::Receive, _context: EffectContext) -> Vec<Message<'sim, E>> {
        (match (&mut self.state, e) {
            (
                LossyWindowControllerState::Disabled { .. },
                LossyInternalControllerEffect::Toggle(Toggle::Enable),
            ) => {
                let cca = (self.new_cca)();
                let initial_settings = cca.initial_settings();
                self.state = LossyWindowControllerState::Enabled(cca);
                Some(SettingsUpdate::Enable(initial_settings))
            }
            (
                LossyWindowControllerState::Enabled(_),
                LossyInternalControllerEffect::Toggle(Toggle::Disable),
            ) => {
                self.state = LossyWindowControllerState::Disabled {
                    wait_for_enable: true,
                };
                Some(SettingsUpdate::Disable)
            }
            (
                LossyWindowControllerState::Enabled(cca),
                LossyInternalControllerEffect::AckReceived(context),
            ) => cca
                .ack_received(context, &mut self.rng, &mut self.logger)
                .map(SettingsUpdate::Enable),
            (
                LossyWindowControllerState::Disabled { .. },
                LossyInternalControllerEffect::AckReceived(_),
            ) => None,
            _ => {
                panic!("Unexpected toggle!")
            }
        })
        .map(|x| {
            self.sender
                .create_message(LossyInternalSenderEffect::SettingsUpdate(x))
        })
        .into_iter()
        .collect_vec()
    }
}
