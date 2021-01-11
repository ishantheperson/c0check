use std::fmt::{self, Display};
use anyhow::Result;

use crate::executer::*;
use crate::spec::*;

pub fn run_test(executer: &Box<dyn Executer>, test: &TestInfo) -> Result<TestResult> {
    let properties = executer.properties();
    
    // See if any behaviors apply
    let mut behaviors: Vec<&Behavior> = Vec::new();
    for spec in test.specs.iter() {
        if let Some(behavior) = find_behavior(spec, &properties) {
            behaviors.push(behavior)
        }
    }
    
    if behaviors.is_empty() {
        return Ok(TestResult::Success)
    }
    
    let (output, result) = executer.run_test(&test.execution)?;
    for &behavior in behaviors.iter() {
        if behavior != &result {
            return Ok(TestResult::Mismatch(Failure { expected: *behavior, actual: result, output }))
        }
    }

    Ok(TestResult::Success)    
}

pub enum TestResult {
    Success,
    Mismatch(Failure)
}

/// Contains all information from a failed test run,
/// including stdout/stderr from the compiler or program
/// (depending on which stage failed)
pub struct Failure {
    pub expected: Behavior,
    pub actual: Behavior, 
    pub output: String    
}

impl Failure {
    pub fn is_timeout(&self) -> bool {
        self.actual == Behavior::InfiniteLoop
    }    
}

fn matches_predicate(predicate: &ImplementationPredicate, properties: &ExecuterProperties) -> bool {
    use ImplementationPredicate::*;
    match predicate {
        Library => properties.libraries,
        Typechecked => properties.typechecked,
        GarbageCollected => properties.garbage_collected,
        Safe => properties.safe,
        False => false,
        ImplementationName(name ) => &properties.name == name,

        Not(p) => !matches_predicate(p, properties),
        And(p1, p2) => matches_predicate(p1, properties) && matches_predicate(p2, properties),
        Or(p1, p2) => matches_predicate(p1, properties) || matches_predicate(p2, properties),
    }
}

fn find_behavior<'a>(spec: &'a Spec, properties: &ExecuterProperties) -> Option<&'a Behavior> {
    match spec {
        Spec::Behavior(b) => Some(b),
        Spec::Implication(predicate, consequent) => {
            if matches_predicate(predicate, properties) {
                find_behavior(consequent, properties)
            }
            else {
                None
            }
        }
    }
}

impl Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.output.is_empty() {
            write!(f, "expected {}, got {}", self.expected, self.actual)
        }
        else {
            write!(f, "expected {}, got {}\n{}", self.expected, self.actual, self.output)
        }
    }
}
