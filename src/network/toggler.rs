use crate::{
    rand::{PositiveContinuousDistribution, Rng},
    simulation::{Component, ComponentId, EffectContext, HasVariant, Message},
    quantities::{seconds, Float, Time},
};

#[derive(PartialEq, Eq, Debug)]
pub enum Toggle {
    Enable,
    Disable,
}

#[derive(Debug)]
pub struct Toggler {
    target: ComponentId,
    enabled: bool,
    on_distribution: PositiveContinuousDistribution<Float>,
    off_distribution: PositiveContinuousDistribution<Float>,
    next_toggle: Time,
}

impl Toggler {
    pub fn new(
        target: ComponentId,
        on_distribution: PositiveContinuousDistribution<Float>,
        off_distribution: PositiveContinuousDistribution<Float>,
        rng: &mut Rng,
    ) -> Toggler {
        Toggler {
            target,
            enabled: false,
            next_toggle: Time::from_sim_start(rng.sample(&off_distribution)),
            on_distribution,
            off_distribution,
        }
    }
}

impl<E> Component<E> for Toggler
where
    E: HasVariant<Toggle>,
{
    fn tick(&mut self, context: EffectContext) -> Vec<Message<E>> {
        assert_eq!(
            Some(context.time),
            Component::<E>::next_tick(self, context.time)
        );
        let mut effects = Vec::new();
        if context.time == self.next_toggle {
            self.enabled = !self.enabled;
            let dist = if self.enabled {
                effects.push(Message::new(self.target, Toggle::Enable));
                &self.on_distribution
            } else {
                effects.push(Message::new(self.target, Toggle::Disable));
                &self.off_distribution
            };
            self.next_toggle = context.time + seconds(context.rng.sample(dist));
        }
        effects
    }

    fn receive(&mut self, _e: E, _context: EffectContext) -> Vec<Message<E>> {
        panic!()
    }

    fn next_tick(&self, _time: Time) -> Option<Time> {
        Some(self.next_toggle)
    }
}
