use rand::SeedableRng;
use rand_distr::{Distribution, Exp, Normal, Uniform};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContinuousDistribution {
    Uniform { min: f32, max: f32 },
    Normal { mean: f32, std_dev: f32 },
    Exponential { mean: f32 },
}

impl Distribution<f32> for ContinuousDistribution {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> f32 {
        match self {
            ContinuousDistribution::Uniform { min, max } => rng.sample(Uniform::new(min, max)),
            ContinuousDistribution::Normal { mean, std_dev } => {
                rng.sample(Normal::new(*mean, *std_dev).unwrap())
            }
            ContinuousDistribution::Exponential { mean } => {
                rng.sample(Exp::new(1. / mean).unwrap())
            }
        }
    }
}

pub struct Rng {
    rng: Xoshiro256PlusPlus,
}

impl Rng {
    pub fn from_seed(seed: u64) -> Rng {
        Rng {
            rng: Xoshiro256PlusPlus::seed_from_u64(seed),
        }
    }

    pub fn create_child(&mut self) -> Rng {
        Rng {
            rng: Xoshiro256PlusPlus::from_rng(&mut self.rng).unwrap(),
        }
    }

    pub fn sample<R>(&mut self, dist: &impl Distribution<R>) -> R {
        dist.sample(&mut self.rng)
    }
}
