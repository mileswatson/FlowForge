use network::Network;
use serde::{de::DeserializeOwned, Serialize};

#[warn(clippy::pedantic, clippy::nursery)]
#[allow(clippy::module_name_repetitions)]
pub mod network;
pub mod rand;
pub mod trainers;

pub trait Dna {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(buf: &[u8]) -> Self;
}

pub trait ProgressHandler<D: Dna> {
    fn update_progress(&mut self, d: &D);
}

impl<F: FnMut(&D), D: Dna> ProgressHandler<D> for F {
    fn update_progress(&mut self, d: &D) {
        self(d)
    }
}

pub trait Trainer {
    type DNA: Dna;
    type Config: Serialize + DeserializeOwned;

    fn new(config: &Self::Config) -> Self;

    fn train<H: ProgressHandler<Self::DNA>>(
        &self,
        networks: &[Network],
        progress_handler: &mut H,
    ) -> Self::DNA;
}
