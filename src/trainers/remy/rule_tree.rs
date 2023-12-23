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
pub(super) enum RuleTree {
    Node {
        domain: Cube,
        children: Box<[RuleTree; 8]>,
    },
    Leaf {
        domain: Cube,
        access_tracker: AtomicU64,
        action: Action,
    },
}

impl PartialEq for RuleTree {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Node {
                    domain: l_domain,
                    children: l_children,
                },
                Self::Node {
                    domain: r_domain,
                    children: r_children,
                },
            ) => l_domain == r_domain && l_children == r_children,
            (
                Self::Leaf {
                    domain: l_domain,
                    access_tracker: _,
                    action: l_action,
                },
                Self::Leaf {
                    domain: r_domain,
                    access_tracker: _,
                    action: r_action,
                },
            ) => l_domain == r_domain && l_action == r_action,
            _ => false,
        }
    }
}

impl RuleTree {
    pub fn new_with_same_rules(self: &RuleTree) -> RuleTree {
        match self {
            RuleTree::Node { domain, children } => RuleTree::Node {
                domain: domain.clone(),
                children: Box::new(
                    children
                        .iter()
                        .map(RuleTree::new_with_same_rules)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                ),
            },
            RuleTree::Leaf { domain, action, .. } => RuleTree::Leaf {
                domain: domain.clone(),
                access_tracker: AtomicU64::new(0),
                action: action.clone(),
            },
        }
    }

    const fn domain(&self) -> &Cube {
        match self {
            RuleTree::Node { domain, .. } | RuleTree::Leaf { domain, .. } => domain,
        }
    }

    pub fn action<const COUNT: bool>(&self, point: &Point) -> Option<&Action> {
        if !self.domain().contains(point) {
            return None;
        }
        match self {
            RuleTree::Node { children, .. } => {
                children.iter().find_map(|x| x.action::<COUNT>(point))
            }
            RuleTree::Leaf {
                access_tracker,
                action,
                ..
            } => {
                if COUNT {
                    access_tracker.fetch_add(1, Ordering::Relaxed);
                }
                Some(action)
            }
        }
    }

    fn _most_used_rule(&mut self) -> (u64, &mut RuleTree) {
        match self {
            RuleTree::Node { children, .. } => children
                .iter_mut()
                .map(RuleTree::_most_used_rule)
                .max_by_key(|x| x.0)
                .unwrap(),
            RuleTree::Leaf { access_tracker, .. } => {
                let num = *access_tracker.get_mut();
                (num, self)
            }
        }
    }

    pub fn most_used_rule(&mut self) -> &mut RuleTree {
        self._most_used_rule().1
    }
}

impl Default for RuleTree {
    fn default() -> Self {
        RuleTree::Leaf {
            domain: Cube::default(),
            access_tracker: AtomicU64::new(0),
            action: Action::default(),
        }
    }
}

impl From<RuleTree> for WhiskerTree {
    fn from(value: RuleTree) -> Self {
        let mut tree = WhiskerTree::new();
        let cube = value.domain().clone();
        let domain = tree.domain.mut_or_insert_default();
        domain.lower = MessageField::some(cube.min.clone().into());
        domain.upper = MessageField::some(cube.max.clone().into());
        match value {
            RuleTree::Node { children, .. } => {
                tree.children = children.into_iter().map(Into::into).collect();
            }
            RuleTree::Leaf { action, .. } => {
                tree.leaf =
                    MessageField::some(Whisker::create(&action, cube.min.clone(), cube.max));
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
        if value.leaf.is_some() {
            RuleTree::Leaf {
                domain,
                action: value.leaf.into(),
                access_tracker: AtomicU64::new(0),
            }
        } else {
            RuleTree::Node {
                domain,
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
