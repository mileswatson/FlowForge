pub mod action;
pub mod cube;
pub mod point;
pub mod rule_tree;
pub mod dna;

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}
