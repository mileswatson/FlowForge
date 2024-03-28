use std::{cmp::Reverse, iter::repeat, marker::PhantomData, sync::Mutex};

use anyhow::Result;
use itertools::Itertools;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{
    core::never::Never,
    core::rand::Rng,
    evaluator::EvaluationConfig,
    flow::{FlowProperties, NoActiveFlows, UtilityFunction},
    network::{config::NetworkConfig, toggler::Toggle, AddFlows, EffectTypeGenerator, Packet},
    quantities::Float,
    simulation::HasSubEffect,
    Dna, ProgressHandler, Trainer,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneticConfig {
    iters: u32,
    population_size: u32,
    evaluation_config: EvaluationConfig,
}

impl Default for GeneticConfig {
    fn default() -> Self {
        Self {
            iters: 100,
            population_size: 1000,
            evaluation_config: EvaluationConfig::default(),
        }
    }
}

pub struct GeneticTrainer<A, G> {
    iters: u32,
    population_size: u32,
    evaluation_config: EvaluationConfig,
    effect: PhantomData<G>,
    flow_adder: PhantomData<A>,
}

pub trait GeneticDna<G>: Dna + Sync {
    fn new_random(rng: &mut Rng) -> Self;

    #[must_use]
    fn spawn_child(&self, rng: &mut Rng) -> Self;
}

impl<A, G> Trainer for GeneticTrainer<A, G>
where
    G: EffectTypeGenerator,
    A: AddFlows<G>,
    A::Dna: GeneticDna<G>,
    for<'sim> G::Type<'sim>: HasSubEffect<Packet<'sim, G::Type<'sim>>>
        + HasSubEffect<Toggle>
        + HasSubEffect<Never>
        + 'sim,
{
    type Config = GeneticConfig;
    type Dna = A::Dna;
    type DefaultEffectGenerator = G;
    type DefaultFlowAdder = A;

    fn new(config: &Self::Config) -> Self {
        GeneticTrainer {
            iters: config.iters,
            population_size: config.population_size,
            evaluation_config: config.evaluation_config.clone(),
            flow_adder: PhantomData,
            effect: PhantomData,
        }
    }

    fn train<H>(
        &self,
        starting_point: Option<A::Dna>,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> A::Dna
    where
        H: ProgressHandler<A::Dna>,
    {
        assert!(
            starting_point.is_none(),
            "Starting point not supported for genetic trainer!"
        );
        let mut population = (0..self.population_size)
            .map(|_| A::Dna::new_random(rng))
            .collect_vec();
        let progress = Mutex::new((0, progress_handler));
        let increment_progress = || {
            let mut handle = progress.lock().unwrap();
            handle.0 += 1;
            #[allow(clippy::cast_precision_loss)]
            let progress =
                f64::from(handle.0) / (f64::from(self.population_size) * f64::from(self.iters));
            handle.1.update_progress(progress, None);
        };
        let update_best = |best: &A::Dna| {
            let mut handle = progress.lock().unwrap();
            #[allow(clippy::cast_precision_loss)]
            let progress =
                f64::from(handle.0) / (f64::from(self.population_size) * f64::from(self.iters));
            handle.1.update_progress(progress, Some(best));
        };
        let update_progress = &increment_progress;
        for _ in 0..self.iters {
            let mut scores = population
                .into_iter()
                .map(|d| (d, rng.create_child()))
                .filter_map(|(d, mut rng)| {
                    let score = self.evaluation_config.evaluate::<A, G>(
                        network_config,
                        &d,
                        utility_function,
                        &mut rng,
                    );
                    update_progress();
                    score.map(|(s, p)| (d, s, p)).ok()
                })
                .collect_vec();
            scores.sort_by_key(|x| Reverse(NotNan::new(x.1).unwrap()));

            println!("Score: {}", scores.first().unwrap().1);
            update_best(&scores.first().unwrap().0);
            scores.truncate(self.population_size as usize / 2);
            population = scores
                .iter()
                .flat_map(|x| repeat(&x.0).take(2))
                .map(|x| x.spawn_child(rng))
                .collect();
        }
        population.into_iter().next().unwrap()
    }

    fn evaluate(
        &self,
        d: &A::Dna,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        self.evaluation_config
            .evaluate::<A, G>(network_config, d, utility_function, rng)
    }
}
