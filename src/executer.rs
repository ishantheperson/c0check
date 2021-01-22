use anyhow::Result;

use crate::spec::*;

pub struct ExecuterProperties {
    pub libraries: bool,
    pub typechecked: bool,
    pub garbage_collected: bool,
    pub safe: bool,
    pub name: &'static str,
}

impl ExecuterProperties {
    /// Checks if the given predicate is true for this executer
    pub fn matches_predicate(&self, predicate: &ImplementationPredicate) -> bool {
        use ImplementationPredicate::*;
        match predicate {
            Library => self.libraries,
            Typechecked => self.typechecked,
            GarbageCollected => self.garbage_collected,
            Safe => self.safe,
            False => false,
            ImplementationName(name) => self.name == name,
    
            Not(p) => !self.matches_predicate(p),
            And(p1, p2) => self.matches_predicate(p1) && self.matches_predicate(p2),
            Or(p1, p2) => self.matches_predicate(p1) || self.matches_predicate(p2),
        }
    }    
}

pub trait Executer: Send + Sync {
    /// How to run a test. 
    /// Returns (Test output, Test actual behavior)
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)>;

    /// Gets the properties of this executer
    fn properties(&self) -> ExecuterProperties;
}
