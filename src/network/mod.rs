pub mod config;

#[derive(Debug)]
pub struct Network {
    pub rtt: f32,
    pub throughput: f32,
    pub loss_rate: f32,
}
