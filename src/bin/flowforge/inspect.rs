use std::{fs::File, io::Write, path::Path};

use flowforge::{
    ccas::{
        remy::{action::Action, dna::RemyDna, point::Point, rule_tree::RuleTree},
        remyr::{
            dna::RemyrDna,
            net::{HiddenLayers, PolicyNet},
        },
    },
    quantities::{seconds, Float, Time},
    Config, Custom,
};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Serialize;

use crate::FlowAdders;

type PolicySummary = Vec<((Float, Float, Float), (Float, Float, Float))>;

#[derive(Serialize)]
struct RemyInspection {
    maximum_depth: u64,
    policy: PolicySummary,
}

#[derive(Serialize)]
struct RemyrInspection {
    min_point: Point,
    max_point: Point,
    min_action: Action,
    max_action: Action,
    hidden_layers: HiddenLayers,
    policy: PolicySummary,
}

fn inspect_rule_tree(rule_tree: &(impl RuleTree + Sync)) -> PolicySummary {
    let resolution = 100;
    let range = |min, max| {
        (0..resolution).map(move |x| (Float::from(x) / Float::from(resolution)) * (max - min) + min)
    };
    let rtt_ratios = vec![1.02, 1.1].into_iter();
    let ack_ewmas = range(0., 0.5).map(seconds);
    let send_ewmas = range(0., 0.5).map(seconds);
    let points = rtt_ratios
        .cartesian_product(ack_ewmas)
        .cartesian_product(send_ewmas)
        .map(|((rtt_ratio, ack_ewma), send_ewma)| Point {
            ack_ewma,
            send_ewma,
            rtt_ratio,
        })
        .collect_vec();
    points
        .into_par_iter()
        .map(|point| {
            rule_tree
                .action(&point, Time::SIM_START)
                .map(|a| {
                    (
                        (
                            point.ack_ewma.seconds(),
                            point.send_ewma.seconds(),
                            point.rtt_ratio,
                        ),
                        (
                            a.window_multiplier,
                            Float::from(a.window_increment),
                            a.intersend_delay.seconds(),
                        ),
                    )
                })
                .unwrap()
        })
        .collect()
}

fn inspect_remy(dna: &Path) -> String {
    let dna = RemyDna::load(dna).unwrap();
    serde_json::to_string(&RemyInspection {
        maximum_depth: dna.tree.num_parents() as u64,
        policy: inspect_rule_tree(&dna),
    })
    .unwrap()
}

fn inspect_remyr(dna: &Path) -> String {
    let dna = <RemyrDna as Config<Custom>>::load(dna).unwrap();
    serde_json::to_string(&RemyrInspection {
        min_point: dna.min_point.clone(),
        max_point: dna.max_point.clone(),
        min_action: dna.min_action.clone(),
        max_action: dna.max_action.clone(),
        hidden_layers: dna.policy.hidden_layers(),
        policy: inspect_rule_tree(&dna),
    })
    .unwrap()
}

pub fn inspect(dna: &Path, mode: &FlowAdders, output: Option<&Path>) {
    let s = match mode {
        FlowAdders::Remy => inspect_remy(dna),
        FlowAdders::Remyr => inspect_remyr(dna),
        _ => panic!("Not supported!"),
    };
    match output {
        Some(file) => {
            let mut file = File::create(file).unwrap();
            write!(file, "{}", s).unwrap();
        }
        None => println!("{}", s),
    }
}
