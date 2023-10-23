use std::marker::PhantomData;

use network::Network;

#[warn(clippy::pedantic, clippy::nursery)]
#[allow(clippy::module_name_repetitions)]
pub mod network;
pub mod rand;
pub mod trainers;

pub trait Dna {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(buf: &[u8]) -> Self;
}

impl Dna for () {
    fn serialize(&self) -> Vec<u8> {
        todo!()
    }

    fn deserialize(buf: &[u8]) -> Self {
        todo!()
    }
}

pub trait Trainer {
    type Output;

    fn train(&self, networks: &[Network]) -> Self::Output;
}

pub struct IgnoreResultTrainer<T>
where
    T: Trainer,
{
    pub trainer: T,
}

impl<T: Trainer> Trainer for IgnoreResultTrainer<T> {
    type Output = ();

    fn train(&self, networks: &[Network]) {
        self.trainer.train(networks);
    }
}
