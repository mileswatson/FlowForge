use std::{
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering},
};

use protobuf::MessageField;

use crate::time::Float;

use super::{
    action::Action,
    autogen::remy_dna::{Whisker, WhiskerTree},
    cube::Cube,
    point::Point,
    RemyConfig,
};

pub trait RuleTree<const TESTING: bool = false>: Debug {
    fn action(&self, point: &Point) -> Option<&Action<TESTING>>;
}

#[derive(Debug)]
pub struct AugmentedRuleTree<'a> {
    tree: &'a BaseRuleTree,
    rule_override: (usize, Action),
}

impl<'a> RuleTree for AugmentedRuleTree<'a> {
    fn action(&self, point: &Point) -> Option<&Action> {
        self.tree._action(self.tree.root, point, &|idx| {
            if idx == self.rule_override.0 {
                Some(&self.rule_override.1)
            } else {
                None
            }
        })
    }
}

#[derive(Debug)]
pub struct CountingRuleTree<'a> {
    tree: &'a mut BaseRuleTree,
    counts: Vec<AtomicU64>,
}

impl<'a> RuleTree for CountingRuleTree<'a> {
    fn action(&self, point: &Point) -> Option<&Action> {
        self.tree._action(self.tree.root, point, &|idx| {
            self.counts[idx].fetch_add(1, Ordering::Relaxed);
            None
        })
    }
}

impl<'a> CountingRuleTree<'a> {
    pub fn new(tree: &'a mut BaseRuleTree) -> CountingRuleTree<'a> {
        CountingRuleTree {
            counts: tree.nodes.iter().map(|_| AtomicU64::new(0)).collect(),
            tree,
        }
    }

    fn _most_used_rule<const ONLY_UNOPTIMIZED: bool>(mut self) -> Option<(Float, LeafHandle<'a>)> {
        self.tree
            .greatest_leaf_node(|idx, optimized| {
                if ONLY_UNOPTIMIZED && optimized {
                    None
                } else {
                    Some(*self.counts[idx].get_mut())
                }
            })
            .map(|handle| {
                #[allow(clippy::cast_precision_loss)]
                return (
                    *self.counts[handle.rule].get_mut() as Float
                        / self.counts.iter_mut().map(|c| *c.get_mut()).sum::<u64>() as Float,
                    handle,
                );
            })
    }

    #[must_use]
    pub fn most_used_rule(self) -> (Float, LeafHandle<'a>) {
        self._most_used_rule::<false>().unwrap()
    }

    #[must_use]
    pub fn most_used_unoptimized_rule(self) -> Option<(Float, LeafHandle<'a>)> {
        self._most_used_rule::<true>()
    }
}

pub struct LeafHandle<'a> {
    tree: &'a mut BaseRuleTree,
    rule: usize,
}

impl<'a> LeafHandle<'a> {
    #[must_use]
    pub fn augmented_tree(&'a self, new_action: Action) -> AugmentedRuleTree<'a> {
        AugmentedRuleTree {
            tree: self.tree,
            rule_override: (self.rule, new_action),
        }
    }

    pub fn action(&mut self) -> &mut Action {
        match &mut self.tree.nodes[self.rule] {
            RuleTreeNode::Node { .. } => panic!(),
            RuleTreeNode::Leaf { action, .. } => action,
        }
    }

    pub fn mark_optimized(self) {
        match &mut self.tree.nodes[self.rule] {
            RuleTreeNode::Node { .. } => panic!(),
            RuleTreeNode::Leaf { optimized, .. } => *optimized = true,
        }
    }

    #[must_use]
    pub fn domain(&self) -> &Cube {
        match &self.tree.nodes[self.rule] {
            RuleTreeNode::Node { .. } => panic!(),
            RuleTreeNode::Leaf { domain, .. } => domain,
        }
    }

    pub fn split(self) {
        let children: Vec<_> = match &self.tree.nodes[self.rule] {
            RuleTreeNode::Node { .. } => panic!(),
            RuleTreeNode::Leaf { domain, action, .. } => domain
                .split()
                .into_iter()
                .map(|domain| RuleTreeNode::Leaf {
                    domain,
                    action: action.clone(),
                    optimized: false,
                })
                .collect(),
        };
        self.tree.nodes[self.rule] = RuleTreeNode::Node {
            domain: self.tree.nodes[self.rule].domain().clone(),
            children: children
                .into_iter()
                .map(|node| {
                    self.tree.nodes.push(node);
                    self.tree.nodes.len() - 1
                })
                .collect(),
        };
    }
}

#[derive(Debug)]
pub enum RuleTreeNode<const TESTING: bool = false> {
    Node {
        domain: Cube<TESTING>,
        children: Vec<usize>,
    },
    Leaf {
        domain: Cube<TESTING>,
        action: Action<TESTING>,
        optimized: bool,
    },
}

impl<const TESTING: bool> RuleTreeNode<TESTING> {
    fn equals(
        lhs: &RuleTreeNode<TESTING>,
        lhs_tree: &BaseRuleTree<TESTING>,
        rhs: &RuleTreeNode<TESTING>,
        rhs_tree: &BaseRuleTree<TESTING>,
    ) -> bool {
        match (lhs, rhs) {
            (
                Self::Node {
                    domain: l_domain,
                    children: l_children,
                },
                Self::Node {
                    domain: r_domain,
                    children: r_children,
                },
            ) => {
                l_domain == r_domain
                    && l_children.iter().zip(r_children).all(|(x, y)| {
                        RuleTreeNode::<TESTING>::equals(
                            &lhs_tree.nodes[*x],
                            lhs_tree,
                            &rhs_tree.nodes[*y],
                            rhs_tree,
                        )
                    })
            }
            (
                Self::Leaf {
                    domain: l_domain,
                    action: l_action,
                    ..
                },
                Self::Leaf {
                    domain: r_domain,
                    action: r_action,
                    ..
                },
            ) => l_domain == r_domain && l_action == r_action,
            _ => false,
        }
    }

    const fn domain(&self) -> &Cube<TESTING> {
        match self {
            RuleTreeNode::Node { domain, .. } | RuleTreeNode::Leaf { domain, .. } => domain,
        }
    }
}

#[derive(Debug)]
pub struct BaseRuleTree<const TESTING: bool = false> {
    root: usize,
    nodes: Vec<RuleTreeNode<TESTING>>,
}

fn _push_whisker_tree<const TESTING: bool>(
    nodes: &mut Vec<RuleTreeNode<TESTING>>,
    value: &WhiskerTree,
) -> usize {
    let domain = Cube {
        min: Point::from_memory(&value.domain.lower),
        max: Point::from_memory(&value.domain.upper),
    };
    let new_node = if value.leaf.is_some() {
        RuleTreeNode::<TESTING>::Leaf {
            domain,
            action: Action::<TESTING>::from_whisker(&value.leaf),
            optimized: false,
        }
    } else {
        RuleTreeNode::Node {
            domain,
            children: value
                .children
                .iter()
                .map(|child| _push_whisker_tree::<TESTING>(nodes, child))
                .collect(),
        }
    };
    nodes.push(new_node);
    nodes.len() - 1
}

fn push_tree<const TESTING: bool>(
    nodes: &mut Vec<RuleTreeNode<TESTING>>,
    root: usize,
    tree: &BaseRuleTree<TESTING>,
) -> usize {
    let new_node = match &tree.nodes[root] {
        RuleTreeNode::Node { domain, children } => RuleTreeNode::Node {
            children: children
                .iter()
                .map(|child| push_tree(nodes, *child, tree))
                .collect(),
            domain: domain.clone(),
        },
        RuleTreeNode::Leaf { domain, action, .. } => RuleTreeNode::Leaf {
            domain: domain.clone(),
            action: action.clone(),
            optimized: false,
        },
    };
    nodes.push(new_node);
    nodes.len() - 1
}

impl<const TESTING: bool> BaseRuleTree<TESTING> {
    #[must_use]
    pub fn from_tree(self: &BaseRuleTree<TESTING>) -> BaseRuleTree<TESTING> {
        let mut nodes = Vec::new();
        let root = push_tree(&mut nodes, self.root, self);
        BaseRuleTree { root, nodes }
    }

    fn _action<'a, F>(
        &'a self,
        current_idx: usize,
        point: &Point,
        leaf_override: &F,
    ) -> Option<&Action<TESTING>>
    where
        F: Fn(usize) -> Option<&'a Action<TESTING>>,
    {
        let current = &self.nodes[current_idx];
        if !current.domain().contains(point) {
            return None;
        }
        match current {
            RuleTreeNode::Node { children, .. } => children
                .iter()
                .find_map(|x| self._action(*x, point, leaf_override)),
            RuleTreeNode::Leaf { action, .. } => {
                if let Some(a) = leaf_override(current_idx) {
                    return Some(a);
                }
                Some(action)
            }
        }
    }

    fn _to_whisker_tree(&self, root: usize) -> WhiskerTree {
        let value = &self.nodes[root];
        let mut tree = WhiskerTree::new();
        let cube = value.domain().clone();
        let domain = tree.domain.mut_or_insert_default();
        domain.lower = MessageField::some(cube.min.to_memory());
        domain.upper = MessageField::some(cube.max.to_memory());
        match value {
            RuleTreeNode::Node { children, .. } => {
                tree.children = children.iter().map(|i| self._to_whisker_tree(*i)).collect();
            }
            RuleTreeNode::Leaf { action, .. } => {
                tree.leaf = MessageField::some(Whisker::create(action, &cube.min, &cube.max));
            }
        };
        tree
    }

    #[must_use]
    pub fn to_whisker_tree(&self) -> WhiskerTree {
        self._to_whisker_tree(self.root)
    }

    pub fn from_whisker_tree(value: &WhiskerTree) -> BaseRuleTree<TESTING> {
        let mut nodes = Vec::new();
        let root = _push_whisker_tree::<TESTING>(&mut nodes, value);
        BaseRuleTree { root, nodes }
    }
}

impl BaseRuleTree {
    fn greatest_leaf_node<F>(&mut self, mut score: F) -> Option<LeafHandle<'_>>
    where
        F: FnMut(usize, bool) -> Option<u64>,
    {
        self.nodes
            .iter_mut()
            .enumerate()
            .filter_map(|(i, n)| match n {
                RuleTreeNode::Node { .. } => None,
                RuleTreeNode::Leaf { optimized, .. } => score(i, *optimized).map(|s| (s, i)),
            })
            .max_by_key(|x| x.0)
            .map(|x| LeafHandle {
                tree: self,
                rule: x.1,
            })
    }

    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        BaseRuleTree {
            root: 0,
            nodes: vec![RuleTreeNode::Leaf {
                domain: Cube::default(),
                action: dna.default_action.clone(),
                optimized: false,
            }],
        }
    }

    pub fn mark_all_unoptimized(&mut self) {
        self.nodes.iter_mut().for_each(|n| {
            if let RuleTreeNode::Leaf { optimized, .. } = n {
                *optimized = false;
            }
        });
    }
}

impl RuleTree for BaseRuleTree {
    fn action(&self, point: &Point) -> Option<&Action> {
        self._action(self.root, point, &|_| None)
    }
}

impl<const TESTING: bool> PartialEq for BaseRuleTree<TESTING> {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
            && RuleTreeNode::equals(
                &self.nodes[self.root],
                self,
                &other.nodes[other.root],
                other,
            )
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

    use crate::{
        trainers::remy::{rule_tree::BaseRuleTree, RemyDna},
        Config,
    };

    use super::super::autogen::remy_dna::WhiskerTree;

    fn same_file(p1: &Path, p2: &Path) -> Result<bool> {
        let mut f1 = File::open(p1)?;
        let mut f2 = File::open(p2)?;

        let mut b1 = Vec::new();
        let mut b2 = Vec::new();

        f1.read_to_end(&mut b1)?;
        f2.read_to_end(&mut b2)?;

        Ok(b1 == b2)
    }

    fn check_to_pb(dna: &RemyDna<true>) {
        let cycled = BaseRuleTree::<true>::from_whisker_tree(&dna.tree.to_whisker_tree());
        assert_eq!(dna.tree, cycled);
    }

    fn check_to_dna(pb: &WhiskerTree) {
        let cycled = BaseRuleTree::<true>::from_whisker_tree(pb).to_whisker_tree();
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
            let original = RemyDna::<true>::load(&original_file)?;
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
