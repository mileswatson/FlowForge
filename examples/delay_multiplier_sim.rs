use std::mem::ManuallyDrop;

use flowforge::{
    logging::LogTable,
    network::{
        link::Link,
        protocols::{delay_multiplier::LossySender, window::lossy_window::LossyBouncer},
        NetworkEffect,
    },
    rand::Rng,
    simulation::{DynComponent, SimulatorBuilder},
    time::{Rate, TimeSpan},
};

fn main() {
    let table = LogTable::new(5);
    // ManuallyDrop is used to re-order drop(builder) to before drop(sender),
    // as it can contain a ref to sender
    let builder = ManuallyDrop::new(SimulatorBuilder::<NetworkEffect>::new());

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
    let mut link1 = Link::create(
        TimeSpan::new(1.5),
        Rate::new(0.2),
        0.1,
        Some(1),
        table.logger(2),
    );
    let mut receiver = LossyBouncer::new(link2_slot.id(), table.logger(3));
    let mut link2 = Link::create(
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
    let sim = ManuallyDrop::into_inner(builder).build(&mut rng, table.logger(0));
    sim.run_for(TimeSpan::new(100.));

    println!("{}", table.build());
}
