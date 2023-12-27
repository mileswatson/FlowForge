use std::{
    fmt::Debug,
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

pub trait RuleTree: Debug {
    fn action(&self, point: &Point) -> Option<&Action>;
}

#[derive(Debug)]
pub struct AugmentedRuleTree<'a> {
    tree: &'a CountingRuleTree,
    rule_override: Override,
}

impl<'a> RuleTree for AugmentedRuleTree<'a> {
    fn action(&self, point: &Point) -> Option<&Action> {
        self.tree
            ._action::<_, false>(self.tree.root, point, &self.rule_override)
    }
}

pub struct LeafHandle<'a> {
    tree: &'a mut CountingRuleTree,
    rule: usize,
}

impl<'a> LeafHandle<'a> {
    #[must_use]
    pub fn augmented_tree(&'a self, new_action: Action) -> AugmentedRuleTree<'a> {
        AugmentedRuleTree {
            tree: self.tree,
            rule_override: Override(self.rule, new_action),
        }
    }

    pub fn action(&mut self) -> &mut Action {
        match &mut self.tree.nodes[self.rule] {
            RuleTreeNode::Node { .. } => panic!(),
            RuleTreeNode::Leaf(Leaf { action, .. }) => action,
        }
    }
}

#[derive(Debug)]
pub struct Leaf {
    domain: Cube,
    access_tracker: AtomicU64,
    pub action: Action,
    pub optimized: bool,
}

#[derive(Debug)]
pub enum RuleTreeNode {
    Node { domain: Cube, children: [usize; 8] },
    Leaf(Leaf),
}

impl RuleTreeNode {
    fn equals(
        lhs: &RuleTreeNode,
        lhs_tree: &CountingRuleTree,
        rhs: &RuleTreeNode,
        rhs_tree: &CountingRuleTree,
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
                        RuleTreeNode::equals(
                            &lhs_tree.nodes[*x],
                            lhs_tree,
                            &rhs_tree.nodes[*y],
                            rhs_tree,
                        )
                    })
            }
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

    const fn domain(&self) -> &Cube {
        match self {
            RuleTreeNode::Node { domain, .. } | RuleTreeNode::Leaf(Leaf { domain, .. }) => domain,
        }
    }
}

pub trait RuleOverride: Clone + Debug {
    fn try_override(&self, current: usize) -> Option<&Action>;
}

#[derive(Debug, Clone)]
pub struct Override(usize, Action);

#[derive(Debug, Clone)]
pub struct NoOverride;

impl RuleOverride for Override {
    fn try_override(&self, current: usize) -> Option<&Action> {
        if current == self.0 {
            Some(&self.1)
        } else {
            None
        }
    }
}

impl RuleOverride for NoOverride {
    fn try_override(&self, _current: usize) -> Option<&Action> {
        None
    }
}

#[derive(Debug)]
pub struct CountingRuleTree {
    root: usize,
    nodes: Vec<RuleTreeNode>,
}

fn _push_whisker_tree(nodes: &mut Vec<RuleTreeNode>, value: &WhiskerTree) -> usize {
    let domain = Cube {
        min: Point::from_memory(&value.domain.lower),
        max: Point::from_memory(&value.domain.upper),
    };
    let new_node = if value.leaf.is_some() {
        RuleTreeNode::Leaf(Leaf {
            domain,
            action: Action::from_whisker(&value.leaf),
            access_tracker: AtomicU64::new(0),
            optimized: false,
        })
    } else {
        RuleTreeNode::Node {
            domain,
            children: value
                .children
                .iter()
                .map(|child| _push_whisker_tree(nodes, child))
                .collect::<Vec<_>>()
                .try_into()
                .expect("vector of length 8"),
        }
    };
    nodes.push(new_node);
    nodes.len() - 1
}

fn push_tree(nodes: &mut Vec<RuleTreeNode>, root: usize, tree: &CountingRuleTree) -> usize {
    let new_node = match &tree.nodes[root] {
        RuleTreeNode::Node { domain, children } => RuleTreeNode::Node {
            children: children
                .iter()
                .map(|child| push_tree(nodes, *child, tree))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            domain: domain.clone(),
        },
        RuleTreeNode::Leaf(Leaf { domain, action, .. }) => RuleTreeNode::Leaf(Leaf {
            domain: domain.clone(),
            access_tracker: AtomicU64::new(0),
            action: action.clone(),
            optimized: false,
        }),
    };
    nodes.push(new_node);
    nodes.len() - 1
}

impl CountingRuleTree {
    #[must_use]
    pub fn from_tree(self: &CountingRuleTree) -> CountingRuleTree {
        let mut nodes = Vec::new();
        let root = push_tree(&mut nodes, self.root, self);
        CountingRuleTree { root, nodes }
    }

    pub fn _action<'a, O, const COUNT: bool>(
        &'a self,
        current_idx: usize,
        point: &Point,
        rule_override: &'a O,
    ) -> Option<&Action>
    where
        O: RuleOverride + 'a,
    {
        let current = &self.nodes[current_idx];
        if !current.domain().contains(point) {
            return None;
        }
        match current {
            RuleTreeNode::Node { children, .. } => children
                .iter()
                .find_map(|x| self._action::<O, COUNT>(*x, point, rule_override)),
            RuleTreeNode::Leaf(leaf) => {
                if let Some(a) = rule_override.try_override(current_idx) {
                    return Some(a);
                }
                if COUNT {
                    leaf.access_tracker.fetch_add(1, Ordering::Relaxed);
                }
                Some(&leaf.action)
            }
        }
    }

    fn _most_used_rule<const ONLY_OPTIMIZED: bool>(&mut self) -> Option<LeafHandle<'_>> {
        self.nodes
            .iter_mut()
            .enumerate()
            .filter_map(|(i, n)| match n {
                RuleTreeNode::Node { .. } => None,
                RuleTreeNode::Leaf(Leaf {
                    access_tracker,
                    optimized,
                    ..
                }) => {
                    if ONLY_OPTIMIZED && !*optimized {
                        None
                    } else {
                        Some((*access_tracker.get_mut(), i))
                    }
                }
            })
            .max_by_key(|x| x.0)
            .map(|x| LeafHandle {
                tree: self,
                rule: x.1,
            })
    }

    pub fn most_used_rule(&mut self) -> LeafHandle {
        self._most_used_rule::<false>().unwrap()
    }

    pub fn most_used_unoptimized_rule(&mut self) -> Option<LeafHandle> {
        self._most_used_rule::<true>()
    }

    #[must_use]
    pub fn default(dna: &RemyConfig) -> Self {
        CountingRuleTree {
            root: 0,
            nodes: vec![RuleTreeNode::Leaf(Leaf {
                domain: Cube::default(),
                access_tracker: AtomicU64::new(0),
                action: dna.default_action.clone(),
                optimized: false,
            })],
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
            RuleTreeNode::Leaf(Leaf { action, .. }) => {
                tree.leaf = MessageField::some(Whisker::create(action, &cube.min, &cube.max));
            }
        };
        tree
    }

    #[must_use]
    pub fn to_whisker_tree(&self) -> WhiskerTree {
        self._to_whisker_tree(self.root)
    }

    pub fn from_whisker_tree(value: &WhiskerTree) -> CountingRuleTree {
        let mut nodes = Vec::new();
        let root = _push_whisker_tree(&mut nodes, value);
        CountingRuleTree { root, nodes }
    }
}

impl PartialEq for CountingRuleTree {
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

impl RuleTree for CountingRuleTree {
    fn action(&self, point: &Point) -> Option<&Action> {
        self._action::<_, true>(self.root, point, &NoOverride)
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
        trainers::remy::{rule_tree::CountingRuleTree, RemyDna},
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

    fn check_to_pb(dna: &RemyDna) {
        let cycled = CountingRuleTree::from_whisker_tree(&dna.tree.to_whisker_tree());
        assert_eq!(dna.tree, cycled);
    }

    fn check_to_dna(pb: &WhiskerTree) {
        let cycled = CountingRuleTree::from_whisker_tree(pb).to_whisker_tree();
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
