use std::{cmp::Reverse, iter::repeat, marker::PhantomData, sync::Mutex};

use anyhow::Result;
use itertools::Itertools;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{
    evaluator::{EvaluationConfig, PopulateComponents},
    flow::{FlowProperties, NoActiveFlows, UtilityFunction},
    network::config::NetworkConfig,
    quantities::Float,
    rand::Rng,
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

pub struct GeneticTrainer<D> {
    iters: u32,
    population_size: u32,
    evaluation_config: EvaluationConfig,
    dna: PhantomData<D>,
}

pub trait GeneticDna: Dna + PopulateComponents {
    fn new_random(rng: &mut Rng) -> Self;

    #[must_use]
    fn spawn_child(&self, rng: &mut Rng) -> Self;
}

impl<D> Trainer for GeneticTrainer<D>
where
    D: GeneticDna,
{
    type Config = GeneticConfig;
    type Dna = D;

    fn new(config: &Self::Config) -> Self {
        GeneticTrainer {
            iters: config.iters,
            population_size: config.population_size,
            evaluation_config: config.evaluation_config.clone(),
            dna: PhantomData,
        }
    }

    fn train<H>(
        &self,
        starting_point: Option<D>,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> D
    where
        H: ProgressHandler<D>,
        D: GeneticDna,
    {
        assert!(
            starting_point.is_none(),
            "Starting point not supported for genetic trainer!"
        );
        let mut population = (0..self.population_size)
            .map(|_| D::new_random(rng))
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
        let update_best = |best: &D| {
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
                    let score = self.evaluation_config.evaluate(
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
        d: &D,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        rng: &mut Rng,
    ) -> Result<(Float, FlowProperties), NoActiveFlows> {
        self.evaluation_config
            .evaluate(network_config, d, utility_function, rng)
    }
}
