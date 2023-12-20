use std::convert::Into;

use anyhow::Result;
use protobuf::{Message, MessageField};
use serde::{Deserialize, Serialize};

use crate::{
    flow::UtilityFunction, network::config::NetworkConfig, rand::Rng, time::Float, Dna,
    ProgressHandler, Trainer,
};

use self::autogen::remy_dna::{Memory, MemoryRange, Whisker, WhiskerTree};

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[derive(Serialize, Deserialize, Default)]
pub struct RemyConfig {}

#[derive(Debug, Clone, PartialEq)]
struct Point {
    ack_ewma: Float,
    send_ewma: Float,
    rtt_ratio: Float,
}

impl Point {
    const MIN: Point = Point {
        ack_ewma: 0.,
        send_ewma: 0.,
        rtt_ratio: 0.,
    };
    // TODO
    const MAX: Point = Point {
        ack_ewma: 163_840.,
        send_ewma: 163_840.,
        rtt_ratio: 163_840.,
    };
}

impl From<Point> for Memory {
    fn from(value: Point) -> Self {
        let mut memory = Memory::new();
        memory.set_rec_rec_ewma(value.ack_ewma);
        memory.set_rec_send_ewma(value.send_ewma);
        memory.set_rtt_ratio(value.rtt_ratio);
        memory
    }
}

impl From<MessageField<Memory>> for Point {
    fn from(value: MessageField<Memory>) -> Self {
        Point {
            ack_ewma: value.rec_rec_ewma(),
            send_ewma: value.rec_send_ewma(),
            rtt_ratio: value.rtt_ratio(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Action {
    window_multiplier: Float,
    window_increment: i32,
    intersend_ms: Float,
    num_accesses: u64,
    epoch: u64,
}

impl Whisker {
    fn create(value: &Action, min: Point, max: Point) -> Self {
        let mut memory_range = MemoryRange::new();
        memory_range.lower = MessageField::some(min.into());
        memory_range.upper = MessageField::some(max.into());
        let mut whisker = Whisker::new();
        whisker.set_intersend(value.intersend_ms);
        whisker.set_window_increment(value.window_increment);
        whisker.set_window_multiple(value.window_multiplier);
        whisker.domain = MessageField::some(memory_range);
        whisker
    }
}

impl From<MessageField<Whisker>> for Action {
    fn from(value: MessageField<Whisker>) -> Self {
        Action {
            window_multiplier: value.window_multiple(),
            window_increment: value.window_increment(),
            intersend_ms: value.intersend(),
            num_accesses: 0,
            epoch: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RuleTree {
    Node {
        min: Point,
        max: Point,
        children: Box<[RuleTree; 8]>,
    },
    Leaf {
        min: Point,
        max: Point,
        action: Action,
    },
}

impl Default for RuleTree {
    fn default() -> Self {
        RuleTree::Leaf {
            action: Action {
                window_multiplier: 1.,
                window_increment: 1,
                intersend_ms: 0.01,
                num_accesses: 0,
                epoch: 0,
            },
            min: Point::MIN,
            max: Point::MAX,
        }
    }
}

impl From<RuleTree> for WhiskerTree {
    fn from(value: RuleTree) -> Self {
        match value {
            RuleTree::Node { min, max, children } => {
                let mut tree = WhiskerTree::new();
                tree.children = children.into_iter().map(Into::into).collect();
                let domain = tree.domain.mut_or_insert_default();
                domain.lower = MessageField::some(min.into());
                domain.upper = MessageField::some(max.into());
                tree
            }
            RuleTree::Leaf { action, min, max } => {
                let mut tree = WhiskerTree::new();

                tree.leaf = MessageField::some(Whisker::create(&action, min.clone(), max.clone()));
                let domain = tree.domain.mut_or_insert_default();
                domain.lower = MessageField::some(min.into());
                domain.upper = MessageField::some(max.into());
                tree
            }
        }
    }
}

impl From<WhiskerTree> for RuleTree {
    fn from(value: WhiskerTree) -> Self {
        if value.leaf.is_some() {
            RuleTree::Leaf {
                min: value.domain.lower.clone().into(),
                max: value.domain.upper.clone().into(),
                action: value.leaf.into(),
            }
        } else {
            RuleTree::Node {
                min: value.domain.lower.clone().into(),
                max: value.domain.upper.clone().into(),
                children: Box::new(
                    value
                        .children
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<_>>()
                        .try_into()
                        .expect("vector of length 8"),
                ),
            }
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct RemyDna {
    tree: RuleTree,
}

impl Dna for RemyDna {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(WhiskerTree::from(self.tree.clone()).write_to_bytes()?)
    }

    fn deserialize(buf: &[u8]) -> Result<RemyDna> {
        Ok(RemyDna {
            tree: WhiskerTree::parse_from_bytes(buf)?.into(),
        })
    }
}

pub struct RemyTrainer {}

impl Trainer<RemyDna> for RemyTrainer {
    type Config = RemyConfig;

    fn new(config: &RemyConfig) -> RemyTrainer {
        RemyTrainer {}
    }

    fn train<H: ProgressHandler<RemyDna>>(
        &self,
        network_config: &NetworkConfig,
        utility_function: &dyn UtilityFunction,
        progress_handler: &mut H,
        rng: &mut Rng,
    ) -> RemyDna {
        let result = RemyDna::default();
        progress_handler.update_progress(1., Some(&result));
        result
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::{
        fs::{read_dir, File},
        io::Read,
        path::Path,
    };

    use anyhow::Result;
    use protobuf::Message;
    use tempfile::tempdir;

    use crate::{trainers::remy::RemyDna, Config};

    use super::{autogen::remy_dna::WhiskerTree, RuleTree};

    fn same_file(p1: &Path, p2: &Path) -> Result<bool> {
        let mut f1 = File::open(p1)?;
        let mut f2 = File::open(p2)?;

        let mut b1 = Vec::new();
        let mut b2 = Vec::new();

        f1.read_to_end(&mut b1)?;
        f2.read_to_end(&mut b2)?;

        Ok(b1 == b2)
    }

    fn check_to_pb(dna: &RemyDna) {
        let cycled = RuleTree::from(WhiskerTree::from(dna.tree.clone()));
        assert_eq!(dna.tree, cycled);
    }

    fn check_to_dna(pb: &WhiskerTree) {
        let cycled = WhiskerTree::from(RuleTree::from(pb.clone()));
        assert_eq!(pb, &cycled);
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
            check_to_pb(&original);
            let mut file = File::open(original_file.clone())?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            let raw_pb = WhiskerTree::parse_from_bytes(&buf)?;
            check_to_dna(&raw_pb);
            assert!(same_file(&original_file, &tmp_file).unwrap());
        }

        Ok(())
    }
}
