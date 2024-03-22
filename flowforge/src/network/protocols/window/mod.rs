use std::{cell::RefCell, fmt::Debug, rc::Rc};

use derive_more::{From, TryInto};

use crate::{
    flow::Flow,
    logging::Logger,
    network::{toggler::Toggle, Packet, PacketDestination},
    quantities::{Time, TimeSpan},
    simulation::{
        Component, ComponentSlot, DynComponent, HasSubEffect, MessageDestination, SimulatorBuilder,
    },
};

use self::{controller::LossyWindowController, sender::Sender};

mod controller;
mod sender;

enum SettingsUpdate {
    Enable(LossyWindowSettings),
    Disable,
}

#[derive(Debug, Clone)]
pub struct LossyWindowSettings {
    pub window: u32,
    pub intersend_delay: TimeSpan,
}

pub trait LossyWindowBehavior: Debug {
    fn initial_settings(&self) -> LossyWindowSettings;
    fn ack_received<L: Logger>(
        &mut self,
        context: AckReceived,
        logger: &mut L,
    ) -> Option<LossyWindowSettings>;
}

pub struct AckReceived {
    pub current_settings: LossyWindowSettings,
    pub sent_time: Time,
    pub received_time: Time,
}

#[derive(From, TryInto)]
pub enum SenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    #[allow(private_interfaces)]
    SettingsUpdate(SettingsUpdate),
}

#[derive(From, TryInto)]
pub enum ControllerEffect {
    Toggle(Toggle),
    AckReceived(AckReceived),
}

#[derive(From, TryInto)]
pub enum LossySenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    Toggle(Toggle),
}

pub type LossySenderMessageDestination<'sim, E> =
    MessageDestination<'sim, LossySenderEffect<'sim, E>, E>;

pub struct LossySenderSlot<'sim, 'a, 'b, E> {
    sender_slot: ComponentSlot<'sim, 'a, 'b, SenderEffect<'sim, E>, E>,
    controller_slot: ComponentSlot<'sim, 'a, 'b, ControllerEffect, E>,
    destination: LossySenderMessageDestination<'sim, E>,
}

impl<'sim, 'a, 'b, E> LossySenderSlot<'sim, 'a, 'b, E>
where
    E: HasSubEffect<ControllerEffect> + HasSubEffect<SenderEffect<'sim, E>> + 'sim,
{
    #[must_use]
    pub fn destination(&self) -> LossySenderMessageDestination<'sim, E> {
        self.destination.clone()
    }

    pub fn set<B>(
        self,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        dest: PacketDestination<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: impl Logger + Clone + 'a,
    ) -> (LossySenderMessageDestination<'sim, E>, Rc<dyn Flow + 'a>)
    where
        B: LossyWindowBehavior + 'a,
        'sim: 'a,
    {
        let LossySenderSlot {
            sender_slot,
            controller_slot,
            destination,
        } = self;
        let sender = Rc::new(RefCell::new(Sender::new(
            controller_slot.destination(),
            id,
            link,
            dest,
            logger.clone(),
        )));
        let sender_destination = sender_slot.set(DynComponent::shared(sender.clone()
            as Rc<RefCell<dyn Component<'sim, E, Receive = SenderEffect<'sim, E>> + 'a>>));
        controller_slot.set(DynComponent::new(LossyWindowController::new(
            sender_destination,
            new_behavior,
            wait_for_enable,
            logger,
        )));
        (destination, sender)
    }
}

pub struct LossyWindowSender;

impl LossyWindowSender {
    pub fn reserve_slot<'sim, 'a, 'b, E>(
        builder: &'b SimulatorBuilder<'sim, 'a, E>,
    ) -> LossySenderSlot<'sim, 'a, 'b, E>
    where
        E: HasSubEffect<SenderEffect<'sim, E>> + HasSubEffect<ControllerEffect> + 'sim,
    {
        let sender_slot = builder.reserve_slot();
        let controller_slot = builder.reserve_slot();
        let packet_destination = sender_slot.destination().cast();
        let toggle_destination = controller_slot.destination().cast();
        LossySenderSlot {
            destination: MessageDestination::custom(move |x| match x {
                LossySenderEffect::Packet(packet) => {
                    packet_destination.create_message(SenderEffect::Packet(packet))
                }
                LossySenderEffect::Toggle(toggle) => {
                    toggle_destination.create_message(ControllerEffect::Toggle(toggle))
                }
            }),
            sender_slot,
            controller_slot,
        }
    }

    pub fn insert<'sim, 'a, 'b, B, E, L>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketDestination<'sim, E>,
        link: PacketDestination<'sim, E>,
        destination: PacketDestination<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> (LossySenderMessageDestination<'sim, E>, Rc<dyn Flow + 'a>)
    where
        E: HasSubEffect<SenderEffect<'sim, E>> + HasSubEffect<ControllerEffect> + 'sim,
        L: Logger + Clone + 'a,
        B: LossyWindowBehavior + 'a,
        'sim: 'a,
    {
        let slot = Self::reserve_slot(builder);
        slot.set(id, link, destination, new_behavior, wait_for_enable, logger)
    }
}
