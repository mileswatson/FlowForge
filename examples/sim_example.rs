use derive_more::{From, TryInto};
use derive_where::derive_where;
use flowforge::{
    quantities::{seconds, Time},
    simulation::{Address, Component, Message, SimulatorBuilder},
    util::{logging::NothingLogger, never::Never},
};
use generativity::make_guard;

struct PingPrinter;

struct Ping;
struct Pong;

#[derive_where(Debug)]
struct User<'sim, E> {
    next_send: Time,
    server: Address<'sim, PingPrinter, E>,
}

impl<'sim, E> Component<'sim, E> for User<'sim, E> {
    type Receive = Never;

    fn next_tick(&self, _: Time) -> Option<Time> {
        Some(self.next_send)
    }

    fn tick(&mut self, _: Time) -> Vec<Message<'sim, E>> {
        self.next_send = self.next_send + seconds(1.);
        vec![self.server.create_message(PingPrinter)]
    }
}

#[derive(From, TryInto)]
enum ServerMessage {
    PingPrinter(PingPrinter),
    Pong(Pong),
}

#[derive_where(Debug)]
struct Server<'sim, E> {
    printer: Address<'sim, Ping, E>,
}

impl<'sim, E> Component<'sim, E> for Server<'sim, E> {
    type Receive = ServerMessage;

    fn receive(&mut self, e: ServerMessage, time: Time) -> Vec<Message<'sim, E>> {
        match e {
            ServerMessage::PingPrinter(PingPrinter) => {
                println!("Sent Pong at time {time}");
                vec![self.printer.create_message(Ping)]
            }
            ServerMessage::Pong(Pong) => {
                println!("Received Pong at time {time}");
                vec![]
            }
        }
    }
}

#[derive_where(Debug)]
struct Printer<'sim, E> {
    server: Address<'sim, Pong, E>,
}

impl<'sim, E> Component<'sim, E> for Printer<'sim, E> {
    type Receive = Ping;

    fn receive(&mut self, _: Ping, _: Time) -> Vec<Message<'sim, E>> {
        vec![self.server.create_message(Pong)]
    }
}

#[derive(From, TryInto)]
enum GlobalMessage {
    Server(ServerMessage),
    Ping(Ping),
    Pong(Pong),
    Never(Never),
}

fn main() {
    make_guard!(guard);
    let builder = SimulatorBuilder::<GlobalMessage>::new(guard);

    let printer_slot = builder.reserve_slot();
    let server_slot = builder.reserve_slot();

    let printer_address = printer_slot.fill(Printer {
        server: server_slot.address().cast(),
    });

    let server_address = server_slot.fill(Server {
        printer: printer_address,
    });

    builder.insert(User {
        next_send: Time::SIM_START,
        server: server_address.cast(),
    });

    builder
        .build(NothingLogger)
        .unwrap()
        .run_while(|t| t < Time::from_sim_start(seconds(10.)));
}
