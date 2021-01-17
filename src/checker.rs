use std::fmt::{self, Display};
use anyhow::Result;

use crate::executer::*;
use crate::spec::*;

/// Runs the given test case using the given execution strategy
pub fn run_test(executer: &dyn Executer, test: &TestInfo) -> Result<TestResult> {
    let properties = executer.properties();
    
    // See if any behaviors apply
    let behaviors: Vec<Behavior> = test.specs.iter()
        .filter_map(|spec| find_behavior(spec, &properties))
        .collect();
    
    if behaviors.is_empty() {
        return Ok(TestResult::Success)
    }
    
    let (output, result) = executer.run_test(&test.execution)?;
    for &behavior in behaviors.iter() {
        if behavior != result {
            return Ok(TestResult::Mismatch(Failure { expected: behavior, actual: result, output }))
        }
    }

    Ok(TestResult::Success)    
}

/// Test cases either succeed or have a mismatch between the expected
/// behavior and the actual behavior
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

/// Finds the behavior a given spec prescribes. This basically just involves
/// checking if the execution strategy has the properties that the spec
/// needs (e.g. a garbage collected executor can run tests which require 
/// garbage collection)
fn find_behavior(spec: &Spec, properties: &ExecuterProperties) -> Option<Behavior> {
    match spec {
        Spec::Behavior(b) => Some(*b),
        Spec::Implication(predicate, consequent) => {
            if properties.matches_predicate(predicate) {
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
