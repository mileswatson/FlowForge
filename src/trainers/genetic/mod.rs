use std::{iter::repeat, marker::PhantomData, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::{
    evaluator::{EvaluationConfig, PopulateComponents},
    flow::UtilityFunction,
    network::{config::NetworkConfig, link::Routable, toggler::Toggle},
    rand::Rng,
    simulation::HasVariant,
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

pub struct GeneticTrainer<E, P> {
    iters: u32,
    population_size: u32,
    evaluation_config: EvaluationConfig,
    event: PhantomData<E>,
    packet: PhantomData<P>,
}

pub trait GeneticDna<E>: Dna + PopulateComponents<E> {
    fn new_random(rng: &mut Rng) -> Self;

    #[must_use]
    fn spawn_child(&self, rng: &mut Rng) -> Self;
}

impl<D, E, P> Trainer<D> for GeneticTrainer<E, P>
where
    D: GeneticDna<E>,
    E: HasVariant<P> + HasVariant<Toggle>,
    P: Routable,
{
    type Config = GeneticConfig;

    fn new(config: &Self::Config) -> Self {
        GeneticTrainer {
            iters: config.iters,
            population_size: config.population_size,
            evaluation_config: config.evaluation_config.clone(),
            event: PhantomData,
            packet: PhantomData,
        }
    }

    fn train<H>(
        &self,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> D
    where
        H: ProgressHandler<D>,
        D: GeneticDna<E>,
    {
        let mut population: Vec<_> = (0..self.population_size)
            .map(|_| D::new_random(rng))
            .collect();
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
            let mut scores: Vec<_> = population
                .into_iter()
                .map(|d| (d, rng.create_child()))
                //.par_bridge()
                .map(|(d, mut rng)| {
                    let score = self.evaluation_config.evaluate::<_, P>(
                        network_config,
                        &d,
                        utility_function,
                        &mut rng,
                    );
                    update_progress();
                    (d, score)
                })
                .collect();
            scores.sort_by(|a, b| a.1.total_cmp(&b.1).reverse());

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
}
