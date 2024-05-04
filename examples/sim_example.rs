use derive_more::{From, TryInto};
use derive_where::derive_where;
use flowforge::{
    quantities::{seconds, Time},
    simulation::{Address, Component, Message, SimulatorBuilder},
    util::{logging::NothingLogger, never::Never},
};
use generativity::make_guard;

struct Ping<'sim, E> {
    source: Address<'sim, Pong, E>,
}
struct Pong {
    name: String,
}

#[derive(Debug)]
struct Printer;

impl<'sim, E> Component<'sim, E> for Printer {
    type Receive = Ping<'sim, E>;

    fn receive(&mut self, p: Ping<'sim, E>, _: Time) -> Vec<Message<'sim, E>> {
        vec![p.source.create_message(Pong {
            name: "Printer".into(),
        })]
    }
}

#[derive(Debug)]
struct Toaster;

impl<'sim, E> Component<'sim, E> for Toaster {
    type Receive = Ping<'sim, E>;

    fn receive(&mut self, p: Ping<'sim, E>, _: Time) -> Vec<Message<'sim, E>> {
        vec![p.source.create_message(Pong {
            name: "Toaster".into(),
        })]
    }
}

#[derive_where(Debug)]
struct Server<'sim, E> {
    address: Address<'sim, ServerMessage, E>,
    devices: Vec<Address<'sim, Ping<'sim, E>, E>>,
}

struct PingDevices;

#[derive(From, TryInto)]
enum ServerMessage {
    SendPing(PingDevices),
    Pong(Pong),
}

impl<'sim, E> Component<'sim, E> for Server<'sim, E>
where
    E: 'sim,
{
    type Receive = ServerMessage;

    fn receive(&mut self, e: ServerMessage, time: Time) -> Vec<Message<'sim, E>> {
        match e {
            ServerMessage::SendPing(PingDevices) => {
                println!("Sent pings at time {time}");
                self.devices
                    .iter()
                    .map(|address| {
                        address.create_message(Ping {
                            source: self.address.clone().cast(),
                        })
                    })
                    .collect()
            }
            ServerMessage::Pong(Pong { name }) => {
                println!("Received pong from {name} at time {time}");
                vec![]
            }
        }
    }
}

#[derive_where(Debug)]
struct User<'sim, E> {
    next_send: Time,
    server: Address<'sim, PingDevices, E>,
}

impl<'sim, E> Component<'sim, E> for User<'sim, E> {
    type Receive = Never;

    fn next_tick(&self, _: Time) -> Option<Time> {
        Some(self.next_send)
    }

    fn tick(&mut self, _: Time) -> Vec<Message<'sim, E>> {
        self.next_send = self.next_send + seconds(1.);
        vec![self.server.create_message(PingDevices)]
    }
}

#[derive(From, TryInto)]
enum GlobalMessage<'sim> {
    Server(ServerMessage),
    Ping(Ping<'sim, GlobalMessage<'sim>>),
    Never(Never),
}

#[allow(unused_variables)]
fn main() {
    make_guard!(guard);
    let builder = SimulatorBuilder::<GlobalMessage>::new(guard);

    let printer_address = builder.insert(Printer);
    let toaster_address = builder.insert(Toaster);

    let server_slot = builder.reserve_slot();
    let server = Server {
        address: server_slot.address(),
        devices: vec![printer_address, toaster_address],
    };
    let server_address = server_slot.fill(server);

    // builder.insert(User {
    //     next_send: Time::SIM_START,
    //     server: printer_address.cast(),
    // });
    // COMPILE ERROR - expected Address<PING_PRINTER, _>, found Address<PING, _>

    builder.insert(User {
        next_send: Time::SIM_START,
        server: server_address.cast(),
    });

    // let builder = SimulatorBuilder::<GlobalMessage>::new(guard);
    // COMPILE ERROR - guard moved

    // let builder = SimulatorBuilder::<GlobalMessage>::new(Guard::new( ... ));
    // COMPILE ERROR - unsafe

    make_guard!(guard);
    let builder2 = SimulatorBuilder::<GlobalMessage>::new(guard);

    // builder2.insert(User {
    //     next_send: Time::SIM_START,
    //     server: server_address.cast()
    // }); COMPILE ERROR - invariant lifetime clash due to mixing simulations

    let mut sim = builder.build(NothingLogger).unwrap();
    while sim.time() < Time::from_sim_start(seconds(3.)) && sim.tick() {}

    // Sent pings at time 0.00st
    // Received pong from Printer at time 0.00st
    // Received pong from Toaster at time 0.00st
    // Sent pings at time 1.00st
    // Received pong from Printer at time 1.00st
    // Received pong from Toaster at time 1.00st
    // Sent pings at time 2.00st
    // Received pong from Printer at time 2.00st
    // Received pong from Toaster at time 2.00st
}
