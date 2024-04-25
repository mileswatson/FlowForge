use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use clap::{Parser, ValueEnum};

use flowforge::{
    ccas::{
        remy::{action::Action, dna::RemyDna, point::Point, RemyCcaTemplate, RemyPolicy},
        remyr::dna::RemyrDna,
    },
    eval::EvaluationConfig,
    flow::AlphaFairness,
    networks::remy::RemyNetworkConfig,
    quantities::{seconds, Time},
    trainers::DefaultEffect,
    util::rand::Rng,
    CcaTemplate, Config, Custom,
};
use itertools::Itertools;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    dna: PathBuf,

    #[arg(long)]
    mode: Mode,

    #[arg(long, short)]
    out: PathBuf,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Mode {
    Remy,
    Remyr,
}

#[derive(Clone, Debug)]
pub struct TimerWrapper<'a> {
    dna: &'a (dyn RemyPolicy + Sync),
    durations: &'a Mutex<Vec<Duration>>,
}

impl<'a> RemyPolicy<false> for TimerWrapper<'a> {
    fn action(&self, point: &Point, time: Time) -> Option<Action> {
        let start = Instant::now();
        let action = self.dna.action(point, time);
        let end = Instant::now();
        self.durations.lock().unwrap().push(end - start);
        action
    }
}

pub fn main() {
    let args = Args::parse();
    let mut rng = Rng::from_seed(139487293);
    let durations = Mutex::new(vec![]);
    let eval = EvaluationConfig {
        network_samples: 30,
        run_sim_for: seconds(30.),
    };
    let network = RemyNetworkConfig::default();
    let utility = AlphaFairness::PROPORTIONAL_THROUGHPUT_DELAY_FAIRNESS;
    let cca_template = RemyCcaTemplate::default();
    match args.mode {
        Mode::Remy => {
            let dna: RemyDna<false> = RemyDna::load(&args.dna).unwrap();
            let policy = TimerWrapper {
                dna: &dna,
                durations: &durations,
            };
            let _ = eval.evaluate::<_, DefaultEffect, _>(
                cca_template.with(&policy),
                &network,
                &utility,
                &mut rng,
            );
        }
        Mode::Remyr => {
            let dna: RemyrDna = <RemyrDna as Config<Custom>>::load(&args.dna).unwrap();
            let policy = TimerWrapper {
                dna: &dna,
                durations: &durations,
            };
            let _ = eval.evaluate::<_, DefaultEffect, _>(
                cca_template.with(&policy),
                &network,
                &utility,
                &mut rng,
            );
        }
    }
    let durations = durations
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|x| x.as_nanos())
        .collect_vec();
    let mut file = File::create(args.out).unwrap();
    write!(file, "{}", serde_json::to_string(&durations).unwrap()).unwrap();
}
