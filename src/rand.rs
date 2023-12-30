use rand::SeedableRng;
use rand_distr::{
    num_traits::{Float, PrimInt},
    Distribution, Exp, Normal, Uniform,
};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContinuousDistribution<F> {
    Always { value: F },
    Uniform { min: F, max: F },
    Normal { mean: F, std_dev: F },
    Exponential { mean: F },
}

impl<F> Distribution<F> for ContinuousDistribution<F>
where
    F: Float + rand_distr::uniform::SampleUniform,
    rand_distr::Exp1: rand_distr::Distribution<F>,
    rand_distr::StandardNormal: rand_distr::Distribution<F>,
{
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> F {
        match self {
            ContinuousDistribution::Uniform { min, max } => rng.sample(Uniform::new(min, max)),
            ContinuousDistribution::Normal { mean, std_dev } => {
                rng.sample(Normal::new(*mean, *std_dev).unwrap())
            }
            ContinuousDistribution::Exponential { mean } => {
                rng.sample(Exp::new(F::one() / *mean).unwrap())
            }
            ContinuousDistribution::Always { value } => *value,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscreteDistribution<I: PrimInt> {
    /// A max-exclusive uniform distribution in the range [min, max].
    Uniform {
        min: I,
        max: I,
    },
    Always {
        value: I,
    },
}

impl<I> Distribution<I> for DiscreteDistribution<I>
where
    I: PrimInt + rand_distr::uniform::SampleUniform + From<u16>,
{
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> I {
        match self {
            DiscreteDistribution::Uniform { min, max } => {
                rng.sample(Uniform::new(min, *max + <I as From<u16>>::from(1)))
            }

            DiscreteDistribution::Always { value } => *value,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProbabilityDistribution<F>(pub ContinuousDistribution<F>);

impl<F> Distribution<F> for ProbabilityDistribution<F>
where
    F: Float + rand_distr::uniform::SampleUniform + From<f32>,
    rand_distr::Exp1: rand_distr::Distribution<F>,
    rand_distr::StandardNormal: rand_distr::Distribution<F>,
{
    fn sample<R: rand::prelude::Rng + ?Sized>(&self, rng: &mut R) -> F {
        let mut i = 0;
        loop {
            if i == 100 {
                println!("WARNING: a probability distribution is overwhelmingly returning numbers outside [0, 1]. If the program is hanging, this is a likely cause.");
            }
            let v = rng.sample(&self.0);
            if <F as From<f32>>::from(0.) <= v && v <= From::from(1.) {
                return v;
            }
            i += 1;
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PositiveContinuousDistribution<F>(pub ContinuousDistribution<F>);

impl<F> Distribution<F> for PositiveContinuousDistribution<F>
where
    F: Float + rand_distr::uniform::SampleUniform + From<f32>,
    rand_distr::Exp1: rand_distr::Distribution<F>,
    rand_distr::StandardNormal: rand_distr::Distribution<F>,
{
    fn sample<R: rand::prelude::Rng + ?Sized>(&self, rng: &mut R) -> F {
        let mut i = 0;
        loop {
            if i == 100 {
                println!("WARNING: a probability distribution is overwhelmingly returning numbers outside [0, 1]. If the program is hanging, this is a likely cause.");
            }
            let v = rng.sample(&self.0);
            if <F as From<f32>>::from(0.) < v {
                return v;
            }
            i += 1;
        }
    }
}

#[derive(Debug)]
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
