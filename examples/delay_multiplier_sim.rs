use flowforge::{
    logging::LogTable,
    network::{
        link::Link,
        protocols::{
            delay_multiplier::LossySender,
            window::lossy_window::{LossyBouncer, Packet},
        },
        toggler::Toggle,
    },
    rand::Rng,
    simulation::{DynComponent, MaybeHasVariant, SimulatorBuilder},
    time::{Rate, TimeSpan},
};

#[derive(Debug)]
enum Msg {
    Packet(Packet),
}

impl From<Packet> for Msg {
    fn from(value: Packet) -> Self {
        Msg::Packet(value)
    }
}

impl MaybeHasVariant<Packet> for Msg {
    fn try_into(self) -> Result<Packet, Self> {
        match self {
            Msg::Packet(p) => Ok(p),
        }
    }
}

impl MaybeHasVariant<Toggle> for Msg {
    fn try_into(self) -> Result<Toggle, Self> {
        Err(self)
    }
}

fn main() {
    let table = LogTable::new(5);
    let builder = SimulatorBuilder::<Msg>::new();

    let sender_slot = builder.reserve_slot();
    let link1_slot = builder.reserve_slot();
    let receiver_slot = builder.reserve_slot();
    let link2_slot = builder.reserve_slot();

    let mut sender = LossySender::new(
        sender_slot.id(),
        link1_slot.id(),
        receiver_slot.id(),
        2.0,
        false,
        table.logger(1),
    );
    let mut link1 = Link::<Packet, _>::create(
        TimeSpan::new(1.5),
        Rate::new(0.2),
        0.1,
        Some(1),
        table.logger(2),
    );
    let mut receiver = LossyBouncer::new(link2_slot.id(), table.logger(3));
    let mut link2 = Link::<Packet, _>::create(
        TimeSpan::new(1.5),
        Rate::new(0.2),
        0.1,
        Some(1),
        table.logger(4),
    );

    sender_slot.set(DynComponent::Ref(&mut sender));
    link1_slot.set(DynComponent::Ref(&mut link1));
    receiver_slot.set(DynComponent::Ref(&mut receiver));
    link2_slot.set(DynComponent::Ref(&mut link2));

    let mut rng = Rng::from_seed(1_234_987_348);
    let sim = builder.build(&mut rng, table.logger(0));
    sim.run_for(TimeSpan::new(100.));
    println!("{}", table.build());
}
