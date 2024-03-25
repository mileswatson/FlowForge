use std::{mem::ManuallyDrop, ops::Deref};

use flowforge::{
    core::{
        logging::{LogTable, NothingLogger},
        meters::{AverageFlowMeter, CurrentFlowMeter},
        rand::Rng,
    },
    network::{
        bouncer::LossyBouncer, link::Link, senders::delay_multiplier::LossyDelayMultiplierSender,
    },
    quantities::{packets, packets_per_second, seconds, Time},
    simulation::{DynComponent, SimulatorBuilder},
    trainers::DefaultEffect,
};
use generativity::make_guard;

fn main() {
    let mut rng = Rng::from_seed(1_234_987_348);
    let table = LogTable::new(5);
    // ManuallyDrop is used to re-order drop(builder) to before drop(sender),
    // as it can contain a ref to sender
    make_guard!(guard);
    let builder = ManuallyDrop::new(SimulatorBuilder::<DefaultEffect>::new(guard));

    let sender_slot = LossyDelayMultiplierSender::reserve_slot::<_, NothingLogger>(builder.deref());
    let link1_slot = builder.reserve_slot();
    let receiver_slot = builder.reserve_slot();
    let link2_slot = builder.reserve_slot();

    let sender_address = sender_slot.address().cast();

    let mut flow_meter = (
        AverageFlowMeter::new_disabled(),
        CurrentFlowMeter::new_disabled(Time::SIM_START, seconds(10.)),
    );

    sender_slot.set(
        sender_address,
        link1_slot.address().cast(),
        receiver_slot.address().cast(),
        2.0,
        false,
        &mut flow_meter,
        table.logger(1),
    );
    let mut link1 = Link::create(
        seconds(1.5),
        packets_per_second(0.2),
        0.,
        Some(packets(1)),
        rng.create_child(),
        table.logger(2),
    );
    let mut receiver = LossyBouncer::new(link2_slot.address().cast(), table.logger(3));
    let mut link2 = Link::create(
        seconds(1.5),
        packets_per_second(0.2),
        0.,
        Some(packets(1)),
        rng.create_child(),
        table.logger(4),
    );

    link1_slot.set(DynComponent::Ref(&mut link1));
    receiver_slot.set(DynComponent::Ref(&mut receiver));
    link2_slot.set(DynComponent::Ref(&mut link2));

    let sim = ManuallyDrop::into_inner(builder).build(table.logger(0));
    let length = seconds(1000.);
    sim.run_for(length);

    drop(link1);

    println!("{}", table.build());
    println!("{:?}", flow_meter.1);
    println!(
        "{:?} {:?}",
        flow_meter
            .0
            .average_properties(Time::from_sim_start(length)),
        flow_meter
            .1
            .current_properties(Time::from_sim_start(length))
    );
}
