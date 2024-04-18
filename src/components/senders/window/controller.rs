use std::fmt::Debug;

use itertools::Itertools;

use crate::{
    util::{logging::Logger, rand::Rng},
    components::toggler::Toggle,
    quantities::Time,
    simulation::{Address, Component, EffectContext, Message},
};

use super::{Cca, LossyInternalControllerEffect, LossyInternalSenderEffect, SettingsUpdate};

#[derive(Debug)]
enum LossyWindowControllerState<C> {
    Enabled(C),
    Disabled { wait_for_enable: bool },
}

pub struct LossyWindowController<'sim, C, G, E, L>
where
    G: Fn() -> C,
{
    sender: Address<'sim, LossyInternalSenderEffect<'sim, E>, E>,
    cca_generator: G,
    state: LossyWindowControllerState<C>,
    rng: Rng,
    logger: L,
}

impl<'sim, C, G, E, L: Debug> Debug for LossyWindowController<'sim, C, G, E, L>
where
    G: Fn() -> C,
    C: Cca,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LossyWindowController")
            .field("sender", &self.sender)
            .field("state", &self.state)
            .field("rng", &self.rng)
            .field("logger", &self.logger)
            .finish()
    }
}

impl<'sim, C, G, E, L> LossyWindowController<'sim, C, G, E, L>
where
    G: Fn() -> C,
{
    pub const fn new(
        sender: Address<'sim, LossyInternalSenderEffect<'sim, E>, E>,
        cca_generator: G,
        wait_for_enable: bool,
        rng: Rng,
        logger: L,
    ) -> Self {
        LossyWindowController {
            sender,
            cca_generator,
            state: LossyWindowControllerState::Disabled { wait_for_enable },
            rng,
            logger,
        }
    }
}

impl<'sim, C, G, E, L> Component<'sim, E> for LossyWindowController<'sim, C, G, E, L>
where
    G: Fn() -> C,
    C: Cca,
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
                let cca = (self.cca_generator)();
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
