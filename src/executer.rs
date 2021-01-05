use anyhow::Result;

use crate::spec::*;

pub struct ExecuterProperties {
    pub libraries: bool,
    pub typechecked: bool,
    pub garbage_collected: bool,
    pub safe: bool,
    pub name: String,
}

pub trait Executer {
    fn run_test(info: &TestExecutionInfo) -> Result<Behavior>;
    fn properties() -> ExecuterProperties;
}
