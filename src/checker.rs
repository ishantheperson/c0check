use anyhow::Result;

use crate::executer::*;
use crate::spec::*;

pub fn run_test<T: Executer>(test: &TestInfo) -> Result<TestResult> {
    let properties = T::properties();
    
    // See if any behaviors apply
    let mut behaviors: Vec<&Behavior> = Vec::new();
    for spec in test.specs.iter() {
        if let Some(behavior) = behavior(spec, &properties) {
            behaviors.push(behavior)
        }
    }
    
    if behaviors.is_empty() {
        return Ok(TestResult::Success)
    }
    
    let result = T::run_test(&test.execution)?;
    for &behavior in behaviors.iter() {
        if behavior != &result {
            return Ok(TestResult::Mismatch { expected: *behavior, actual: result })
        }
    }

    Ok(TestResult::Success)    
}

pub enum TestResult {
    Success,
    Mismatch { expected: Behavior, actual: Behavior }
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

fn behavior<'a>(spec: &'a Spec, properties: &ExecuterProperties) -> Option<&'a Behavior> {
    match spec {
        Spec::Behavior(b) => Some(b),
        Spec::Implication(predicate, consequent) => {
            if matches_predicate(predicate, properties) {
                behavior(consequent, properties)
            }
            else {
                None
            }
        }
    }
}

