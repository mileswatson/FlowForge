pub mod action;
pub mod cube;
pub mod dna;
pub mod point;
pub mod rule_tree;

#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
mod autogen {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}
