use anyhow::Result;
use protobuf::Message;
use serde::Serialize;

use crate::Dna;

use super::{
    action::Action, autogen::remy_dna::WhiskerTree, point::Point, rule_tree::RuleTree, RemyPolicy,
};

#[derive(Debug, PartialEq, Serialize)]
pub struct RemyDna<const TESTING: bool = false>(pub RuleTree<TESTING>);

impl RemyDna {
    #[must_use]
    pub fn default(action: Action) -> Self {
        RemyDna(RuleTree::default(action))
    }
}

impl<const TESTING: bool> Dna for RemyDna<TESTING> {
    const NAME: &'static str = "remy";
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_whisker_tree().write_to_bytes()?)
    }

    fn deserialize(buf: &[u8]) -> Result<RemyDna<TESTING>> {
        Ok(RemyDna(RuleTree::<TESTING>::from_whisker_tree(
            &WhiskerTree::parse_from_bytes(buf)?,
        )))
    }
}

impl RemyPolicy for RemyDna {
    fn action(&self, point: &Point) -> Option<Action> {
        self.0.action(point)
    }
}
