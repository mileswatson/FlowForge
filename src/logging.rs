pub trait Logger {
    fn log(&mut self, msg: &str);
}

impl<'a, T> Logger for &'a mut T
where
    T: Logger,
{
    fn log(&mut self, msg: &str) {
        T::log(self, msg);
    }
}

pub struct PrintLogger {
    name: String,
}

impl PrintLogger {
    #[must_use]
    pub const fn new(name: String) -> PrintLogger {
        PrintLogger { name }
    }
}

impl Logger for PrintLogger {
    fn log(&mut self, msg: &str) {
        println!("[{}] {}", self.name, msg);
    }
}

pub struct NothingLogger {}

impl NothingLogger {
    #[must_use]
    pub const fn new() -> NothingLogger {
        NothingLogger {}
    }
}

impl Logger for NothingLogger {
    fn log(&mut self, _msg: &str) {}
}
