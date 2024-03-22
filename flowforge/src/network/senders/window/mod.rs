use std::{cell::RefCell, fmt::Debug, rc::Rc};

use derive_more::{From, TryInto};

use crate::{
    core::logging::Logger,
    flow::Flow,
    network::{toggler::Toggle, Packet, PacketAddress},
    quantities::{Time, TimeSpan},
    simulation::{Address, Component, ComponentSlot, DynComponent, HasSubEffect, SimulatorBuilder},
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
pub enum LossyInternalSenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    #[allow(private_interfaces)]
    SettingsUpdate(SettingsUpdate),
}

#[derive(From, TryInto)]
pub enum LossyInternalControllerEffect {
    Toggle(Toggle),
    AckReceived(AckReceived),
}

#[derive(From, TryInto)]
pub enum LossySenderEffect<'sim, E> {
    Packet(Packet<'sim, E>),
    Toggle(Toggle),
}

pub type LossySenderAddress<'sim, E> = Address<'sim, LossySenderEffect<'sim, E>, E>;

pub struct LossySenderSlot<'sim, 'a, 'b, E> {
    sender_slot: ComponentSlot<'sim, 'a, 'b, LossyInternalSenderEffect<'sim, E>, E>,
    controller_slot: ComponentSlot<'sim, 'a, 'b, LossyInternalControllerEffect, E>,
    address: LossySenderAddress<'sim, E>,
}

impl<'sim, 'a, 'b, E> LossySenderSlot<'sim, 'a, 'b, E>
where
    E: HasSubEffect<LossyInternalControllerEffect>
        + HasSubEffect<LossyInternalSenderEffect<'sim, E>>
        + 'sim,
{
    #[must_use]
    pub fn address(&self) -> LossySenderAddress<'sim, E> {
        self.address.clone()
    }

    pub fn set<B>(
        self,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        dest: PacketAddress<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: impl Logger + Clone + 'a,
    ) -> (LossySenderAddress<'sim, E>, Rc<dyn Flow + 'a>)
    where
        B: LossyWindowBehavior + 'a,
        'sim: 'a,
    {
        let LossySenderSlot {
            sender_slot,
            controller_slot,
            address,
        } = self;
        let sender = Rc::new(RefCell::new(Sender::new(
            controller_slot.address(),
            id,
            link,
            dest,
            logger.clone(),
        )));
        let sender_address = sender_slot.set(DynComponent::shared(sender.clone()
            as Rc<
                RefCell<dyn Component<'sim, E, Receive = LossyInternalSenderEffect<'sim, E>> + 'a>,
            >));
        controller_slot.set(DynComponent::new(LossyWindowController::new(
            sender_address,
            new_behavior,
            wait_for_enable,
            logger,
        )));
        (address, sender)
    }
}

pub struct LossyWindowSender;

impl LossyWindowSender {
    pub fn reserve_slot<'sim, 'a, 'b, E>(
        builder: &'b SimulatorBuilder<'sim, 'a, E>,
    ) -> LossySenderSlot<'sim, 'a, 'b, E>
    where
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
    {
        let sender_slot = builder.reserve_slot();
        let controller_slot = builder.reserve_slot();
        let packet_address = sender_slot.address().cast();
        let toggle_address = controller_slot.address().cast();
        LossySenderSlot {
            address: Address::custom(move |x| match x {
                LossySenderEffect::Packet(packet) => {
                    packet_address.create_message(LossyInternalSenderEffect::Packet(packet))
                }
                LossySenderEffect::Toggle(toggle) => {
                    toggle_address.create_message(LossyInternalControllerEffect::Toggle(toggle))
                }
            }),
            sender_slot,
            controller_slot,
        }
    }

    pub fn insert<'sim, 'a, 'b, B, E, L>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        new_behavior: Box<dyn (Fn() -> B) + 'a>,
        wait_for_enable: bool,
        logger: L,
    ) -> (LossySenderAddress<'sim, E>, Rc<dyn Flow + 'a>)
    where
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
        L: Logger + Clone + 'a,
        B: LossyWindowBehavior + 'a,
        'sim: 'a,
    {
        let slot = Self::reserve_slot(builder);
        slot.set(id, link, destination, new_behavior, wait_for_enable, logger)
    }
}
