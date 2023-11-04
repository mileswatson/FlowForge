use flowforge::{
    logging::LogTable,
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
    let table = LogTable::new(5);
    let sim = Simulator::<Msg, _>::new(
        vec![
            Sender::create(1, table.logger(1)),
            Link::create(2, 1.5, 0.1, table.logger(2)),
            Receiver::create(3, table.logger(3)),
            Link::create(0, 1.5, 0.1, table.logger(4)),
        ],
        Rng::from_seed(1_234_987_348),
        table.logger(0),
    );
    sim.run_until(100.);
    println!("{}", table.build());
}
