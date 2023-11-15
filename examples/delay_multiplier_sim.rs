use flowforge::{
    logging::LogTable,
    network::{
        link::Link,
        protocols::delay_multiplier::{Packet, Receiver, Sender},
    },
    rand::Rng,
    simulation::{ComponentId, DynComponent, HasVariant, Simulator},
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

impl HasVariant<Packet> for Msg {
    fn try_into(self) -> Result<Packet, Self> {
        match self {
            Msg::Packet(p) => Ok(p),
        }
    }
}

fn main() {
    let table = LogTable::new(5);
    let mut rng = Rng::from_seed(1_234_987_348);
    let mut sender = Sender::new::<Msg>(
        ComponentId::new(0),
        ComponentId::new(1),
        ComponentId::new(2),
        2.0,
        TimeSpan::new(0.),
        table.logger(1),
    );
    let mut link1 = Link::<Packet, _>::create(
        TimeSpan::new(1.5),
        Rate::new(0.2),
        0.01,
        Some(1),
        table.logger(2),
    );
    let mut receiver = Receiver::new::<Msg>(ComponentId::new(3), table.logger(3));
    let mut link2 = Link::<Packet, _>::create(
        TimeSpan::new(1.5),
        Rate::new(0.2),
        0.01,
        Some(1),
        table.logger(4),
    );
    let sim = Simulator::<Msg, _>::new(
        vec![
            DynComponent::reference(&mut sender),
            DynComponent::reference(&mut link1),
            DynComponent::reference(&mut receiver),
            DynComponent::reference(&mut link2),
        ],
        &mut rng,
        table.logger(0),
    );
    sim.run_for(TimeSpan::new(100.));
    println!("{}", table.build());
}
