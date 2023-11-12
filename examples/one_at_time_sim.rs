use flowforge::{
    logging::LogTable,
    network::{
        link::Link,
        protocols::one_at_time::{Packet, Receiver, Sender},
    },
    rand::Rng,
    simulation::{HasVariant, Simulator},
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
    let sim = Simulator::<Msg, _>::new(
        vec![
            Sender::create(0, 1, 2, table.logger(1)),
            Link::create(1.5, 0.2, 0.01, Some(1), table.logger(2)),
            Receiver::create(3, table.logger(3)),
            Link::create(1.5, 0.2, 0.01, Some(1), table.logger(4)),
        ],
        Rng::from_seed(1_234_987_348),
        table.logger(0),
    );
    sim.run_until(1000.);
    println!("{}", table.build());
}
