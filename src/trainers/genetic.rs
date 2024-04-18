use std::{cmp::Reverse, iter::repeat, marker::PhantomData};

use itertools::Itertools;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{
    util::{rand::Rng, WithLifetime},
    evaluator::EvaluationConfig,
    flow::UtilityFunction,
    networks::{remy::HasNetworkSubEffects, NetworkConfig},
    CcaTemplate, Dna, ProgressHandler, Trainer,
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

pub struct GeneticTrainer<T> {
    iters: u32,
    population_size: u32,
    child_eval_config: EvaluationConfig,
    trainer: PhantomData<T>,
}

pub trait GeneticDna<G>: Dna + Sync {
    fn new_random(rng: &mut Rng) -> Self;

    #[must_use]
    fn spawn_child(&self, rng: &mut Rng) -> Self;
}

impl<T> Trainer for GeneticTrainer<T>
where
    T: Trainer,
    T::Dna: GeneticDna<T::DefaultEffectGenerator>,
    for<'sim> <T::DefaultEffectGenerator as WithLifetime>::Type<'sim>:
        HasNetworkSubEffects<'sim, <T::DefaultEffectGenerator as WithLifetime>::Type<'sim>>,
{
    type Config = GeneticConfig;
    type Dna = T::Dna;
    type CcaTemplate<'a> = T::CcaTemplate<'a>;
    type DefaultEffectGenerator = T::DefaultEffectGenerator;

    fn new(config: &Self::Config) -> Self {
        GeneticTrainer {
            iters: config.iters,
            population_size: config.population_size,
            child_eval_config: config.evaluation_config.clone(),
            trainer: PhantomData,
        }
    }

    fn train<H>(
        &self,
        starting_point: Option<T::Dna>,
        network_config: &impl NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> T::Dna
    where
        H: ProgressHandler<T::Dna>,
    {
        assert!(
            starting_point.is_none(),
            "Starting point not supported for genetic trainer!"
        );
        let mut population = (0..self.population_size)
            .map(|_| T::Dna::new_random(rng))
            .collect_vec();
        for i in 0..self.iters {
            let frac = f64::from(i) / f64::from(self.iters);
            let mut scores = population
                .into_iter()
                .map(|d| (d, rng.create_child()))
                .filter_map(|(d, mut rng)| {
                    let score = self
                        .child_eval_config
                        .evaluate::<_, T::DefaultEffectGenerator, _>(
                            T::CcaTemplate::default().with(&d),
                            network_config,
                            utility_function,
                            &mut rng,
                        );
                    score.map(|(s, p)| (d, s, p)).ok()
                })
                .collect_vec();
            scores.sort_by_key(|x| Reverse(NotNan::new(x.1).unwrap()));

            println!("Score: {}", scores.first().unwrap().1);
            progress_handler.update_progress(frac, &scores.first().unwrap().0);
            scores.truncate(self.population_size as usize / 2);
            population = scores
                .iter()
                .flat_map(|x| repeat(&x.0).take(2))
                .map(|x| x.spawn_child(rng))
                .collect();
        }
        population.into_iter().next().unwrap()
    }
}
