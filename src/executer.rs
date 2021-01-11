use anyhow::Result;

use crate::spec::*;

pub struct ExecuterProperties {
    pub libraries: bool,
    pub typechecked: bool,
    pub garbage_collected: bool,
    pub safe: bool,
    pub name: String,
}

pub trait Executer: Send + Sync {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)>;
    fn properties(&self) -> ExecuterProperties;
}
