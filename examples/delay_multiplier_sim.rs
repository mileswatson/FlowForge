use std::mem::ManuallyDrop;

use flowforge::{
    components::{bouncer::LossyBouncer, link::Link, senders::lossy::LossySender},
    quantities::{packets, packets_per_second, seconds, Time},
    simulation::SimulatorBuilder,
    trainers::{
        delay_multiplier::{DelayMultiplierCcaTemplate, DelayMultiplierDna},
        DefaultEffect,
    },
    util::{
        logging::LogTable,
        meters::{AverageFlowMeter, CurrentFlowMeter},
        rand::Rng,
    },
    CcaTemplate,
};
use generativity::make_guard;

fn main() {
    let dna = DelayMultiplierDna { multiplier: 2.0 };
    let cca_template = DelayMultiplierCcaTemplate;
    let mut rng = Rng::from_seed(1_234_987_348);
    let table = LogTable::new(5);
    // ManuallyDrop is used to re-order drop(builder) to before drop(sender),
    // as it can contain a ref to sender
    make_guard!(guard);
    let builder = ManuallyDrop::new(SimulatorBuilder::<DefaultEffect>::new(guard));

    let sender_slot = builder.reserve_slot();
    let link1_slot = builder.reserve_slot::<Link<_, _>>();
    let receiver_slot = builder.reserve_slot::<LossyBouncer<_, _>>();
    let link2_slot = builder.reserve_slot::<Link<_, _>>();

    let sender_address = sender_slot.address().cast();

    let mut flow_meter = (
        AverageFlowMeter::new_disabled(),
        CurrentFlowMeter::new_disabled(Time::SIM_START, seconds(10.)),
    );

    sender_slot.fill(LossySender::new(
        sender_address,
        link1_slot.address().cast(),
        receiver_slot.address().cast(),
        &mut flow_meter,
        cca_template.with(&dna),
        false,
        rng.create_child(),
        table.logger(1),
    ));
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

    link1_slot.fill(&mut link1);
    receiver_slot.fill(&mut receiver);
    link2_slot.fill(&mut link2);

    let sim = ManuallyDrop::into_inner(builder)
        .build(table.logger(0))
        .unwrap();
    let sim_end = Time::from_sim_start(seconds(100.));
    sim.run_while(|t| t < sim_end);

    drop(link1);

    println!("{}", table.build());
    println!("{:?}", flow_meter.1);
    println!(
        "{:?} {:?}",
        flow_meter.0.average_properties(sim_end),
        flow_meter.1.current_properties(sim_end)
    );
}
