use crate::{Dna, Trainer};

pub struct RemyConfig {}

pub struct RemyDna {}

impl Dna for RemyDna {
    fn serialize(&self) -> Vec<u8> {
        Vec::new()
    }

    fn deserialize(buf: &[u8]) -> Self {
        Self {}
    }
}

pub struct RemyTrainer {}

impl Trainer for RemyTrainer {
    type Output = RemyDna;

    fn train(&self, networks: &[crate::network::Network]) -> RemyDna {
        RemyDna {}
    }
}
