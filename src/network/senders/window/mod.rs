use std::{cell::RefCell, fmt::Debug, rc::Rc};

use derive_more::{From, TryInto};

use crate::{
    core::{logging::Logger, meters::FlowMeter, rand::Rng}, network::{toggler::Toggle, Packet, PacketAddress}, simulation::{Address, Component, ComponentSlot, DynComponent, HasSubEffect, SimulatorBuilder}, AckReceived, Cca, CwndSettings
};

use self::{controller::LossyWindowController, sender::Sender};

mod controller;
mod sender;

enum SettingsUpdate {
    Enable(CwndSettings),
    Disable,
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

    pub fn set<C>(
        self,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        dest: PacketAddress<'sim, E>,
        cca_generator: impl Fn() -> C + 'a,
        wait_for_enable: bool,
        flow_meter: impl FlowMeter + Debug + 'a,
        rng: Rng,
        logger: impl Logger + Clone + 'a,
    ) -> LossySenderAddress<'sim, E>
    where
        C: Cca + 'a,
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
            flow_meter,
            logger.clone(),
        )));
        let sender_address = sender_slot.set(DynComponent::shared(sender.clone()
            as Rc<
                RefCell<dyn Component<'sim, E, Receive = LossyInternalSenderEffect<'sim, E>> + 'a>,
            >));
        controller_slot.set(DynComponent::new(LossyWindowController::new(
            sender_address,
            cca_generator,
            wait_for_enable,
            rng,
            logger,
        )));
        address
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

    pub fn insert<'sim, 'a, 'b, C, F, E, L>(
        builder: &SimulatorBuilder<'sim, 'a, E>,
        id: PacketAddress<'sim, E>,
        link: PacketAddress<'sim, E>,
        destination: PacketAddress<'sim, E>,
        cca_generator: impl Fn() -> C + 'a,
        wait_for_enable: bool,
        flow_meter: impl FlowMeter + Debug + 'a,
        rng: Rng,
        logger: impl Logger + Clone + 'a,
    ) -> LossySenderAddress<'sim, E>
    where
        E: HasSubEffect<LossyInternalSenderEffect<'sim, E>>
            + HasSubEffect<LossyInternalControllerEffect>
            + 'sim,
        C: Cca + 'a,
        'sim: 'a,
    {
        let slot = Self::reserve_slot(builder);
        slot.set(
            id,
            link,
            destination,
            cca_generator,
            wait_for_enable,
            flow_meter,
            rng,
            logger,
        )
    }
}
