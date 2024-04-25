use std::{cmp::Reverse, iter::repeat};

use itertools::Itertools;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{
    eval::EvaluationConfig,
    flow::UtilityFunction,
    util::{rand::Rng, WithLifetime},
    CcaTemplate, Dna, NetworkConfig, ProgressHandler, Trainer,
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

pub trait GeneticPolicy: Dna + Sync {
    fn new_random(rng: &mut Rng) -> Self;

    #[must_use]
    fn spawn_child(&self, rng: &mut Rng) -> Self;
}

pub trait GeneticTrainer {
    type Policy: GeneticPolicy;
    type CcaTemplate<'a>: CcaTemplate<'a, Policy = &'a Self::Policy>;

    fn genetic_config(&self) -> GeneticConfig;
}

impl<T> Trainer for T
where
    T: GeneticTrainer,
{
    type Policy = T::Policy;
    type CcaTemplate<'a> = T::CcaTemplate<'a>;

    fn train<G, H>(
        &self,
        starting_point: Option<T::Policy>,
        network_config: &impl NetworkConfig<G>,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> T::Policy
    where
        H: ProgressHandler<T::Policy>,
        G: WithLifetime,
    {
        assert!(
            starting_point.is_none(),
            "Starting point not supported for genetic trainer!"
        );
        let config = self.genetic_config();
        let mut population = (0..config.population_size)
            .map(|_| T::Policy::new_random(rng))
            .collect_vec();
        for i in 0..config.iters {
            let frac = f64::from(i) / f64::from(config.iters);
            let mut scores = population
                .into_iter()
                .map(|d| (d, rng.create_child()))
                .filter_map(|(d, mut rng)| {
                    let score = config.evaluation_config.evaluate::<_, G, _>(
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
            scores.truncate(config.population_size as usize / 2);
            population = scores
                .iter()
                .flat_map(|x| repeat(&x.0).take(2))
                .map(|x| x.spawn_child(rng))
                .collect();
        }
        population.into_iter().next().unwrap()
    }
}
