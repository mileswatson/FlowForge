use crate::{
    rand::{ContinuousDistribution, Rng},
    simulation::{Component, ComponentId, EffectResult, HasVariant, Message},
    time::{Float, Time, TimeSpan},
};

pub enum Toggle {
    Enable,
    Disable,
}

pub struct Toggler {
    target: ComponentId,
    enabled: bool,
    on_distribution: ContinuousDistribution<Float>,
    off_distribution: ContinuousDistribution<Float>,
    next_toggle: Time,
}

impl Toggler {
    pub fn new(
        target: ComponentId,
        on_distribution: ContinuousDistribution<Float>,
        off_distribution: ContinuousDistribution<Float>,
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
    fn tick(
        &mut self,
        context: crate::simulation::EffectContext,
    ) -> crate::simulation::EffectResult<E> {
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
            self.next_toggle = context.time + TimeSpan::new(context.rng.sample(&dist));
        }
        EffectResult {
            next_tick: Some(self.next_toggle),
            effects,
        }
    }

    fn receive(
        &mut self,
        _e: E,
        _context: crate::simulation::EffectContext,
    ) -> crate::simulation::EffectResult<E> {
        EffectResult {
            next_tick: Some(self.next_toggle),
            effects: Vec::new(),
        }
    }
}
