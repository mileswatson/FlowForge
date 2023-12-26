use std::{
    fmt::Debug,
    ptr,
    sync::atomic::{AtomicU64, Ordering},
};

use protobuf::MessageField;

use super::{
    action::Action,
    autogen::remy_dna::{Whisker, WhiskerTree},
    cube::Cube,
    point::Point,
    RemyConfig,
};

#[derive(Debug)]
pub struct Leaf {
    domain: Cube,
    access_tracker: AtomicU64,
    pub action: Action,
    pub optimized: bool,
}

#[derive(Debug)]
pub enum RuleTree {
    Node {
        domain: Cube,
        children: Box<[RuleTree; 8]>,
    },
    Leaf(Leaf),
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
                Self::Leaf(Leaf {
                    domain: l_domain,
                    action: l_action,
                    ..
                }),
                Self::Leaf(Leaf {
                    domain: r_domain,
                    action: r_action,
                    ..
                }),
            ) => l_domain == r_domain && l_action == r_action,
            _ => false,
        }
    }
}

pub trait RuleOverride: Clone + Debug {
    fn try_override(&self, rule: &Leaf) -> Option<&Action>;
}

#[derive(Debug, Clone)]
pub struct Override<'a>(&'a Leaf, Action);

#[derive(Debug, Clone)]
pub struct NoOverride;

impl<'a> RuleOverride for Override<'a> {
    fn try_override(&self, rule: &Leaf) -> Option<&Action> {
        if ptr::eq(rule, self.0) {
            Some(&self.1)
        } else {
            None
        }
    }
}

impl RuleOverride for NoOverride {
    fn try_override(&self, _rule: &Leaf) -> Option<&Action> {
        None
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
            RuleTree::Leaf(Leaf { domain, action, .. }) => RuleTree::Leaf(Leaf {
                domain: domain.clone(),
                access_tracker: AtomicU64::new(0),
                action: action.clone(),
                optimized: false,
            }),
        }
    }

    const fn domain(&self) -> &Cube {
        match self {
            RuleTree::Node { domain, .. } | RuleTree::Leaf(Leaf { domain, .. }) => domain,
        }
    }

    pub fn action<'a, O, const COUNT: bool>(
        &'a self,
        point: &Point,
        rule_override: &'a O,
    ) -> Option<&Action>
    where
        O: RuleOverride + 'a,
    {
        if !self.domain().contains(point) {
            return None;
        }

        match self {
            RuleTree::Node { children, .. } => children
                .iter()
                .find_map(|x| x.action::<O, COUNT>(point, rule_override)),
            RuleTree::Leaf(leaf) => {
                if let Some(a) = rule_override.try_override(leaf) {
                    return Some(a);
                }
                if COUNT {
                    leaf.access_tracker.fetch_add(1, Ordering::Relaxed);
                }
                Some(&leaf.action)
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
            RuleTree::Leaf(Leaf { access_tracker, .. }) => (*access_tracker.get_mut(), self),
        }
    }

    pub fn most_used_rule(&mut self) -> &mut RuleTree {
        self._most_used_rule().1
    }

    fn _most_used_unoptimized_rule(&mut self) -> Option<(u64, &mut Leaf)> {
        match self {
            RuleTree::Node { children, .. } => children
                .iter_mut()
                .filter_map(RuleTree::_most_used_unoptimized_rule)
                .max_by_key(|x| x.0),
            RuleTree::Leaf(leaf) => {
                if leaf.optimized {
                    Some((*leaf.access_tracker.get_mut(), leaf))
                } else {
                    None
                }
            }
        }
    }

    pub fn most_used_unoptimized_rule(&mut self) -> Option<&mut Leaf> {
        self._most_used_unoptimized_rule().map(|x| x.1)
    }

    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        RuleTree::Leaf(Leaf {
            domain: Cube::default(),
            access_tracker: AtomicU64::new(0),
            action: dna.default_action.clone(),
            optimized: false,
        })
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
            RuleTree::Leaf(Leaf { action, .. }) => {
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
            RuleTree::Leaf(Leaf {
                domain,
                action: value.leaf.into(),
                access_tracker: AtomicU64::new(0),
                optimized: false,
            })
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
