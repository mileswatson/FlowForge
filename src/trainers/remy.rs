use serde::{Deserialize, Serialize};

use crate::{Dna, ProgressHandler, Trainer};

#[derive(Serialize, Deserialize)]
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
    type DNA = RemyDna;
    type Config = RemyConfig;

    fn new(config: &RemyConfig) -> Self {
        RemyTrainer {}
    }

    fn train<H: ProgressHandler<Self::DNA>>(
        &self,
        networks: &[crate::network::Network],
        progress_handler: &mut H,
    ) -> Self::DNA {
        let result = RemyDna {};
        progress_handler.update_progress(&result);
        result
    }
}
