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

    fn deserialize(buf: &[u8]) -> Result<RemyDna> {
        Ok(RemyDna {
            tree: WhiskerTree::parse_from_bytes(buf)?,
        })
    }
}

pub struct RemyTrainer {}

impl Trainer for RemyTrainer {
    type DNA = RemyDna;
    type Config = RemyConfig;

    fn new(config: &RemyConfig) -> RemyTrainer {
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

#[cfg(test)]
mod tests {
    use std::{
        fs::{read_dir, File},
        io::Read,
        path::Path,
    };

    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{trainers::remy::RemyDna, Config};

    fn same_file(p1: &Path, p2: &Path) -> Result<bool> {
        let mut f1 = File::open(p1)?;
        let mut f2 = File::open(p2)?;

        let mut b1 = Vec::new();
        let mut b2 = Vec::new();

        f1.read_to_end(&mut b1)?;
        f2.read_to_end(&mut b2)?;

        Ok(b1 == b2)
    }

    #[test]
    fn original_remy_compatibility() -> Result<()> {
        let tmp_dir = tempdir()?;
        let test_data_dir = Path::new("./src/trainers/remy/test_dna");
        let dna_files: Vec<_> = read_dir(test_data_dir)?
            .map(Result::unwrap)
            .map(|x| x.path())
            .filter(|x| x.to_str().unwrap().ends_with(".remy.dna"))
            .collect();
        assert_eq!(dna_files.len(), 14);

        for original_file in dna_files {
            let tmp_file = tmp_dir.path().join(original_file.file_name().unwrap());
            let original = RemyDna::load(&original_file)?;
            original.save(&tmp_file)?;

            assert!(same_file(&original_file, &tmp_file).unwrap());
        }

        Ok(())
    }
}
