pub mod lossy_window;

/*
#[derive(Debug)]
struct Timeout {
    ewma: EWMA<TimeSpan>,
    timeout: TimeSpan,
}

impl Timeout {
    pub fn new(update_weight: Float) -> Timeout {
        Timeout {
            ewma: EWMA::new(update_weight, TimeSpan::new(0.)),
            timeout: TimeSpan::new(1.),
        }
    }

    pub fn received_ack(&mut self, rtt: TimeSpan) {
        self.ewma.update(rtt);
        self.timeout = self.ewma.value();
    }

    pub fn timed_out(&mut self) {
        self.timeout *= 2.;
    }

    pub fn value(&self) -> TimeSpan {
        self.timeout
    }
}*/
