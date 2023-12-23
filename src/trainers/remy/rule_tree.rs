use std::{
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

use protobuf::MessageField;

use crate::time::Float;

use super::autogen::remy_dna::{Memory, MemoryRange, Whisker, WhiskerTree};

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub ack_ewma: Float,
    pub send_ewma: Float,
    pub rtt_ratio: Float,
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

#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    min: Point,
    max: Point,
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            min: Point::MIN,
            max: Point::MAX,
        }
    }
}

fn within(min: Float, x: Float, max: Float) -> bool {
    min <= x && x < max
}

impl Cube {
    #[must_use]
    pub fn contains(&self, point: &Point) -> bool {
        within(self.min.rtt_ratio, point.rtt_ratio, self.max.rtt_ratio)
            && within(self.min.ack_ewma, point.ack_ewma, self.max.ack_ewma)
            && within(self.min.send_ewma, point.send_ewma, self.max.send_ewma)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub window_multiplier: Float,
    pub window_increment: i32,
    pub intersend_ms: Float,
}

impl Default for Action {
    fn default() -> Self {
        Self {
            window_multiplier: 1.,
            window_increment: 1,
            intersend_ms: 0.01,
        }
    }
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
        }
    }
}

#[derive(Debug)]
enum RuleTreeVariant {
    Node(Box<[RuleTree; 8]>),
    Leaf {
        epoch: u64,
        access_tracker: AtomicU64,
        action: Action,
    },
}

impl PartialEq for RuleTreeVariant {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Node(l0), Self::Node(r0)) => l0 == r0,
            (
                Self::Leaf {
                    epoch: l_epoch,
                    access_tracker: _,
                    action: l_action,
                },
                Self::Leaf {
                    epoch: r_epoch,
                    access_tracker: _,
                    action: r_action,
                },
            ) => l_epoch == r_epoch && l_action == r_action,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct RuleTree {
    domain: Cube,
    variant: RuleTreeVariant,
}

impl RuleTree {
    pub fn new_with_same_rules(RuleTree { domain, variant }: &RuleTree) -> RuleTree {
        RuleTree {
            domain: domain.clone(),
            variant: match variant {
                RuleTreeVariant::Node(children) => RuleTreeVariant::Node(Box::new(
                    children
                        .iter()
                        .map(RuleTree::new_with_same_rules)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                )),
                RuleTreeVariant::Leaf { action, .. } => RuleTreeVariant::Leaf {
                    epoch: 0,
                    access_tracker: AtomicU64::new(0),
                    action: action.clone(),
                },
            },
        }
    }

    pub fn action<const COUNT: bool>(&self, point: &Point) -> Option<&Action> {
        if !self.domain.contains(point) {
            return None;
        }
        match &self.variant {
            RuleTreeVariant::Node(children) => {
                children.iter().find_map(|x| x.action::<COUNT>(point))
            }
            RuleTreeVariant::Leaf {
                epoch: _,
                access_tracker,
                action,
            } => {
                if COUNT {
                    access_tracker.fetch_add(1, Ordering::Relaxed);
                }
                Some(action)
            }
        }
    }
}

impl Default for RuleTree {
    fn default() -> Self {
        RuleTree {
            domain: Cube::default(),
            variant: RuleTreeVariant::Leaf {
                epoch: 0,
                access_tracker: AtomicU64::new(0),
                action: Action::default(),
            },
        }
    }
}

impl From<RuleTree> for WhiskerTree {
    fn from(value: RuleTree) -> Self {
        let mut tree = WhiskerTree::new();
        let domain = tree.domain.mut_or_insert_default();
        domain.lower = MessageField::some(value.domain.min.clone().into());
        domain.upper = MessageField::some(value.domain.max.clone().into());
        match value.variant {
            RuleTreeVariant::Node(children) => {
                tree.children = children.into_iter().map(Into::into).collect();
            }
            RuleTreeVariant::Leaf { action, .. } => {
                tree.leaf = MessageField::some(Whisker::create(
                    &action,
                    value.domain.min,
                    value.domain.max,
                ));
            }
        };
        tree
    }
}

impl From<WhiskerTree> for RuleTree {
    fn from(value: WhiskerTree) -> Self {
        let domain = Cube {
            min: value.domain.lower.clone().into(),
            max: value.domain.upper.clone().into(),
        };
        let variant = if value.leaf.is_some() {
            RuleTreeVariant::Leaf {
                action: value.leaf.into(),
                epoch: 0,
                access_tracker: AtomicU64::new(0),
            }
        } else {
            RuleTreeVariant::Node(Box::new(
                value
                    .children
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("vector of length 8"),
            ))
        };
        RuleTree { domain, variant }
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

    use super::{super::autogen::remy_dna::WhiskerTree, RuleTree};

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
        let cycled: RuleTree =
            RuleTree::from(WhiskerTree::from(RuleTree::new_with_same_rules(&dna.tree)));
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
