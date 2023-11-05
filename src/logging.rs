use std::cell::RefCell;

use tabled::{
    builder::Builder,
    settings::{style::HorizontalLine, Style},
};

macro_rules! log {
    ($logger:expr, $fmt_str:literal) => {
        $logger.log(|| format!($fmt_str))
    };
    ($logger:expr, $fmt_str:literal, $($args:expr),*) => {
        $logger.log(|| format!($fmt_str, $($args),*))
    };
}

pub trait Logger {
    fn log(&mut self, msg: impl FnOnce() -> String);
}

impl<'a, T> Logger for &'a mut T
where
    T: Logger,
{
    fn log(&mut self, msg: impl FnOnce() -> String) {
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
    fn log(&mut self, msg: impl FnOnce() -> String) {
        println!("[{}] {}", self.name, msg());
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
    fn log(&mut self, _msg: impl FnOnce() -> String) {}
}

pub struct LogTable {
    columns: usize,
    rows: RefCell<Vec<Vec<String>>>,
}

impl LogTable {
    #[must_use]
    pub const fn new(columns: usize) -> LogTable {
        LogTable {
            columns,
            rows: RefCell::new(Vec::new()),
        }
    }

    pub const fn logger(&self, index: usize) -> TableLogger {
        TableLogger { index, table: self }
    }

    pub fn write(&self, index: usize, str: String) {
        let mut row = Vec::with_capacity(self.columns);
        for _ in 0..index {
            row.push(String::new());
        }
        row.push(str);
        for _ in index + 1..self.columns {
            row.push(String::new());
        }
        self.rows.borrow_mut().push(row);
    }

    pub fn build(&self) -> String {
        let mut builder = Builder::default();
        let header = (0..self.columns).map(|i| i.to_string());
        builder.set_header(header);
        for row in self.rows.borrow().iter() {
            builder.push_record(row);
        }
        let rows = self.rows.borrow();
        builder
            .build()
            .with(
                Style::rounded().horizontals(
                    (1..rows.len())
                        .filter(|i| !rows[i - 1][0].is_empty())
                        .map(|i| HorizontalLine::new(i, Style::modern().get_horizontal())),
                ),
            )
            .to_string()
    }
}

pub struct TableLogger<'a> {
    index: usize,
    table: &'a LogTable,
}

impl Logger for TableLogger<'_> {
    fn log(&mut self, msg: impl FnOnce() -> String) {
        self.table.write(self.index, msg());
    }
}
