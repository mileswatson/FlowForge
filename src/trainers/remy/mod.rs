use anyhow::Result;
use prost::Message;
use serde::{Deserialize, Serialize};

use crate::{Dna, ProgressHandler, Trainer};

use self::remy_buffers::WhiskerTree;

mod remy_buffers {
    include!(concat!(env!("OUT_DIR"), "/remy_buffers.rs"));
}

#[derive(Serialize, Deserialize, Default)]
pub struct RemyConfig {}

#[derive(Default)]
pub struct RemyDna {
    tree: WhiskerTree,
}

impl Dna for RemyDna {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.tree.encode(&mut buf)?;
        Ok(buf)
    }

    fn deserialize(buf: &[u8]) -> Result<Self> {
        Ok(RemyDna {
            tree: WhiskerTree::decode(buf)?,
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
