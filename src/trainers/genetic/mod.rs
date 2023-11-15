use std::{iter::repeat, marker::PhantomData, sync::Mutex};

use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    network::{link::Routable, Network, SimProperties},
    rand::Rng,
    simulation::{DynComponent, HasVariant},
    time::{Float, TimeSpan},
    Dna, ProgressHandler, Trainer,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct GeneticConfig {
    iters: usize,
    population_size: usize,
    run_for: Float,
}

impl Default for GeneticConfig {
    fn default() -> Self {
        Self {
            iters: 100,
            population_size: 1000,
            run_for: 1000.,
        }
    }
}

pub struct GeneticTrainer<E, P> {
    iters: usize,
    population_size: usize,
    run_for: TimeSpan,
    event: PhantomData<E>,
    packet: PhantomData<P>,
}

pub struct Components<'a, E: 'static> {
    pub senders: Vec<DynComponent<'a, E>>,
    pub receiver: DynComponent<'a, E>,
    pub get_score: Box<dyn FnOnce() -> Float>,
}

pub trait GeneticDna<E>: Dna {
    fn new_random(rng: &mut Rng) -> Self;

    fn generate_components(&self, sim_properties: SimProperties, rng: &mut Rng) -> Components<E>;

    #[must_use]
    fn spawn_child(&self, rng: &mut Rng) -> Self;
}

impl<D, E, P> Trainer<D> for GeneticTrainer<E, P>
where
    D: GeneticDna<E>,
    E: HasVariant<P>,
    P: Routable,
{
    type Config = GeneticConfig;

    fn new(config: &Self::Config) -> Self {
        GeneticTrainer {
            iters: config.iters,
            population_size: config.population_size,
            run_for: TimeSpan::new(config.run_for),
            event: PhantomData,
            packet: PhantomData,
        }
    }

    fn train<H>(
        &self,
        networks: &[crate::network::Network],
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
            let progress = handle.0 as f32
                / (self.population_size as f32 * self.iters as f32 * networks.len() as f32);
            handle.1.update_progress(progress, None);
        };
        let update_best = |best: &D| {
            let mut handle = progress.lock().unwrap();
            #[allow(clippy::cast_precision_loss)]
            let progress = handle.0 as f32
                / (self.population_size as f32 * self.iters as f32 * networks.len() as f32);
            handle.1.update_progress(progress, Some(best));
        };
        let update_progress = &increment_progress;
        for _ in 0..self.iters {
            let mut scores: Vec<_> = population
                .into_iter()
                .map(|d| (d, rng.create_child()))
                .par_bridge()
                .map(|(d, mut rng)| {
                    let score_network = |n: &Network| {
                        let Components {
                            senders,
                            receiver,
                            get_score,
                        } = d.generate_components(n.sim_properties(), &mut rng);
                        update_progress();
                        let sim = n.to_sim::<E, P>(&mut rng, senders, receiver);
                        sim.run_for(self.run_for);
                        get_score()
                    };
                    #[allow(clippy::cast_precision_loss)]
                    let score =
                        networks.iter().map(score_network).sum::<Float>() / networks.len() as Float;
                    (d, score)
                })
                .collect();
            scores.sort_by(|a, b| a.1.total_cmp(&b.1).reverse());

            println!("Score: {}", scores.first().unwrap().1);
            update_best(&scores.first().unwrap().0);
            scores.truncate(self.population_size / 2);
            population = scores
                .iter()
                .flat_map(|x| repeat(&x.0).take(2))
                .map(|x| x.spawn_child(rng))
                .collect();
        }
        population.into_iter().next().unwrap()
    }
}
