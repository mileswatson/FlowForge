use flowforge::{
    logging::NothingLogger,
    network::{
        link::Link,
        protocols::one_at_time::{Ack, Packet, Receiver, Sender},
    },
    rand::Rng,
    simulation::{HasVariant, Simulator},
};

#[derive(Debug)]
enum Msg {
    Packet(Packet),
    Ack(Ack),
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
            Msg::Ack(_) => Err(self),
        }
    }
}

impl From<Ack> for Msg {
    fn from(ack: Ack) -> Self {
        Msg::Ack(ack)
    }
}

impl HasVariant<Ack> for Msg {
    fn try_into(self) -> Result<Ack, Self> {
        match self {
            Msg::Ack(ack) => Ok(ack),
            Msg::Packet(_) => Err(self),
        }
    }
}

fn main() {
    let mut logger = NothingLogger {};
    let sim = Simulator::<Msg, _>::new(
        vec![
            Sender::create(1, NothingLogger {}),
            Link::create(2, 1.5, 0.1, NothingLogger {}),
            Receiver::create(3, NothingLogger {}),
            Link::create(0, 1.5, 0.1, NothingLogger {}),
        ],
        Rng::from_seed(1_234_987_348),
        &mut logger,
    );

    sim.run_until(10000000.);
}
