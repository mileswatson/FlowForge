use rand::SeedableRng;
use rand_distr::{num_traits::PrimInt, Distribution, Exp, Normal, Uniform};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

use crate::quantities::Float;

pub trait Wrapper {
    type Underlying;
    fn from_underlying(value: Self::Underlying) -> Self;
    fn to_underlying(self) -> Self::Underlying;
}

impl Wrapper for Float {
    type Underlying = Float;

    fn from_underlying(value: Self::Underlying) -> Self {
        value
    }

    fn to_underlying(self) -> Self::Underlying {
        self
    }
}

impl Wrapper for u32 {
    type Underlying = u32;

    fn from_underlying(value: Self::Underlying) -> Self {
        value
    }

    fn to_underlying(self) -> Self::Underlying {
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContinuousDistribution<T> {
    Always { value: T },
    Uniform { min: T, max: T },
    Normal { mean: T, std_dev: T },
    Exponential { mean: T },
}

impl<T> Distribution<T> for ContinuousDistribution<T>
where
    T: Copy + Wrapper<Underlying = Float>,
{
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> T {
        T::from_underlying(match self {
            ContinuousDistribution::Uniform { min, max } => {
                rng.sample(Uniform::new((*min).to_underlying(), (*max).to_underlying()))
            }
            ContinuousDistribution::Normal { mean, std_dev } => rng
                .sample(Normal::new((*mean).to_underlying(), (*std_dev).to_underlying()).unwrap()),
            ContinuousDistribution::Exponential { mean } => {
                rng.sample(Exp::new(1. / (*mean).to_underlying()).unwrap())
            }
            ContinuousDistribution::Always { value } => (*value).to_underlying(),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscreteDistribution<T> {
    /// A max-exclusive uniform distribution in the range [min, max].
    Uniform {
        min: T,
        max: T,
    },
    Always {
        value: T,
    },
}

impl<T> Distribution<T> for DiscreteDistribution<T>
where
    T: Copy + Wrapper,
    T::Underlying: PrimInt + rand_distr::uniform::SampleUniform + From<u16>,
{
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> T {
        T::from_underlying(match self {
            DiscreteDistribution::Uniform { min, max } => rng.sample(Uniform::new(
                min.to_underlying(),
                max.to_underlying() + <T::Underlying as From<u16>>::from(1),
            )),

            DiscreteDistribution::Always { value } => value.to_underlying(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProbabilityDistribution(pub ContinuousDistribution<Float>);

impl Distribution<Float> for ProbabilityDistribution {
    fn sample<R: rand::prelude::Rng + ?Sized>(&self, rng: &mut R) -> Float {
        let mut i = 0;
        loop {
            if i == 100 {
                println!("WARNING: a probability distribution is overwhelmingly returning numbers outside [0, 1]. If the program is hanging, this is a likely cause.");
            }
            let v = rng.sample(&self.0);
            if (0. ..=1.).contains(&v) {
                return v;
            }
            i += 1;
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PositiveContinuousDistribution<T>(pub ContinuousDistribution<T>);

impl<T> Distribution<T> for PositiveContinuousDistribution<T>
where
    T: Copy + Wrapper<Underlying = Float>,
{
    fn sample<R: rand::prelude::Rng + ?Sized>(&self, rng: &mut R) -> T {
        let mut i = 0;
        loop {
            if i == 100 {
                println!("WARNING: a probability distribution is overwhelmingly returning numbers outside [0, 1]. If the program is hanging, this is a likely cause.");
            }
            let v = rng.sample(&self.0);
            if <T::Underlying as From<f32>>::from(0.) < v.to_underlying() {
                return v;
            }
            i += 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rng {
    rng: Xoshiro256PlusPlus,
}

impl Rng {
    #[must_use]
    pub fn from_seed(seed: u64) -> Rng {
        Rng {
            rng: Xoshiro256PlusPlus::seed_from_u64(seed),
        }
    }

    #[must_use]
    // Xoshiro256PlusPlus::from_rng is infallible when called with Xoshiro256PlusPlus
    #[allow(clippy::missing_panics_doc)]
    pub fn create_child(&mut self) -> Rng {
        Rng {
            rng: Xoshiro256PlusPlus::from_rng(&mut self.rng).unwrap(),
        }
    }

    pub fn sample<R>(&mut self, dist: &impl Distribution<R>) -> R {
        dist.sample(&mut self.rng)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiscreteDistribution, Rng};

    #[test]
    fn rng_determinism() {
        let seed = 123_497_239_457;

        let mut rng = Rng::from_seed(seed);
        let dist = DiscreteDistribution::Uniform {
            min: 0,
            max: 1_000_000,
        };
        let mut v1 = Vec::new();
        v1.push(rng.sample(&dist));
        let mut child1 = rng.create_child();
        let mut child2 = rng.create_child();
        let sample1 = child1.sample(&dist);
        v1.push(rng.sample(&dist));
        let sample2 = child2.sample(&dist);
        v1.push(sample1);
        v1.push(sample2);

        let mut rng = Rng::from_seed(seed);
        let mut v2 = Vec::new();
        v2.push(rng.sample(&dist));
        let mut child1 = rng.create_child();
        let mut child2 = rng.create_child();
        let sample2 = child2.sample(&dist);
        let sample1 = child1.sample(&dist);
        v2.push(rng.sample(&dist));
        v2.push(sample1);
        v2.push(sample2);

        assert_eq!(v1, vec![959_040, 834_209, 999_497, 723_315]);
        assert_eq!(v1, v2);
    }
}
