use anyhow::Result;

use crate::spec::*;

pub struct ExecuterProperties {
    pub libraries: bool,
    pub typechecked: bool,
    pub garbage_collected: bool,
    pub safe: bool,
    pub name: String,
}

impl ExecuterProperties {
    pub fn matches_predicate(&self, predicate: &ImplementationPredicate) -> bool {
        use ImplementationPredicate::*;
        match predicate {
            Library => self.libraries,
            Typechecked => self.typechecked,
            GarbageCollected => self.garbage_collected,
            Safe => self.safe,
            False => false,
            ImplementationName(name ) => &self.name == name,
    
            Not(p) => !self.matches_predicate(p),
            And(p1, p2) => self.matches_predicate(p1) && self.matches_predicate(p2),
            Or(p1, p2) => self.matches_predicate(p1) || self.matches_predicate(p2),
        }
    }    
}

pub trait Executer: Send + Sync {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)>;
    fn properties(&self) -> ExecuterProperties;
}
