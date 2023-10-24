use anyhow::Result;
use protobuf::Message;
use serde::{Deserialize, Serialize};

use crate::{Dna, ProgressHandler, Trainer};

use self::remy_dna::WhiskerTree;

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

#[derive(Serialize, Deserialize, Default)]
pub struct RemyConfig {}

#[derive(Default)]
pub struct RemyDna {
    tree: WhiskerTree,
}

impl Dna for RemyDna {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(self.tree.write_to_bytes()?)
    }

    fn deserialize(buf: &[u8]) -> Result<Self> {
        Ok(RemyDna {
            tree: WhiskerTree::parse_from_bytes(buf)?,
        })
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
        let result = RemyDna::default();
        progress_handler.update_progress(&result);
        result
    }
}
