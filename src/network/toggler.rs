use crate::{
    quantities::{Time, TimeSpan},
    rand::{PositiveContinuousDistribution, Rng},
    simulation::{Component, ComponentId, EffectContext, HasVariant, Message},
};

#[derive(PartialEq, Eq, Debug)]
pub enum Toggle {
    Enable,
    Disable,
}

#[derive(Debug)]
pub struct Toggler<'sim> {
    target: ComponentId<'sim>,
    enabled: bool,
    on_distribution: PositiveContinuousDistribution<TimeSpan>,
    off_distribution: PositiveContinuousDistribution<TimeSpan>,
    next_toggle: Time,
    rng: Rng,
}

impl<'sim> Toggler<'sim> {
    #[must_use]
    pub fn new(
        target: ComponentId<'sim>,
        on_distribution: PositiveContinuousDistribution<TimeSpan>,
        off_distribution: PositiveContinuousDistribution<TimeSpan>,
        mut rng: Rng,
    ) -> Toggler<'sim> {
        Toggler {
            target,
            enabled: false,
            next_toggle: Time::from_sim_start(rng.sample(&off_distribution)),
            on_distribution,
            off_distribution,
            rng,
        }
    }
}

impl<'sim, E> Component<'sim, E> for Toggler<'sim>
where
    E: HasVariant<'sim, Toggle>,
{
    fn tick(&mut self, context: EffectContext<'sim>) -> Vec<Message<'sim, E>> {
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
            self.next_toggle = context.time + self.rng.sample(dist);
        }
        effects
    }

    fn receive(&mut self, _e: E, _context: EffectContext) -> Vec<Message<'sim, E>> {
        panic!()
    }

    fn next_tick(&self, _time: Time) -> Option<Time> {
        Some(self.next_toggle)
    }
}
