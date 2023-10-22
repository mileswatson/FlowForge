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

pub trait Trainer<D: Dna> {
    fn train(&self, networks: &[Network]) -> D;
}

pub struct IgnoreResultTrainer<T, D: Dna>
where
    T: Trainer<D>,
{
    pub trainer: T,
    pub marker: PhantomData<D>,
}

impl<T: Trainer<D>, D: Dna> Trainer<()> for IgnoreResultTrainer<T, D> {
    fn train(&self, networks: &[Network]) {
        self.trainer.train(networks);
    }
}
