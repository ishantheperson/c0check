use std::sync::{Mutex, atomic::{self, AtomicUsize}};
use std::time::Instant;
use std::fs;
use rayon::prelude::*;
use anyhow::{Result, Error, Context};

mod spec;
mod discover_tests;
mod parse_spec;
mod launcher;
mod checker;
mod executer;
mod options;
mod implementations;

use crate::spec::*;
use crate::executer::Executer;
use crate::checker::{Failure, TestResult};
use crate::options::*;
use crate::implementations::*;

struct TestResults<'a> {
    failures: Vec<(&'a TestInfo, Failure)>,
    timeouts: Vec<&'a TestInfo>,
    errors: Vec<(&'a TestInfo, Error)>
}

fn run_tests<'a>(executer: &dyn Executer, tests: &'a [TestInfo]) -> TestResults<'a> {
    let failures: Mutex<Vec<(&TestInfo, Failure)>> = Mutex::new(Vec::new());
    let timeouts: Mutex<Vec<&TestInfo>> = Mutex::new(Vec::new());
    let errors: Mutex<Vec<(&TestInfo, Error)>> = Mutex::new(Vec::new());

    let count = AtomicUsize::new(1);
    let start = Instant::now();
    let len_width = tests.len().to_string().len();

    tests.par_iter().for_each(|test| {
        let status = checker::run_test(executer, test);
        // Clear 'race condition' but 🤷‍♀️
        let i = count.fetch_add(1, atomic::Ordering::Relaxed);
        let progress = format!("{:width$}/{:width$}", i, tests.len(), width = len_width);

        match status {
            Ok(TestResult::Success) => {
                eprintln!("{} ✅ {}", progress, test);
            },
            Ok(TestResult::Mismatch(failure)) => {
                if failure.is_timeout() {
                    eprintln!("{} ⌛ {}", progress, test);
                    timeouts.lock().unwrap().push(test);
                }
                else {
                    eprintln!("{} ❌ {}: {}", progress, test, failure);
                    failures.lock().unwrap().push((test, failure));
                }
            },
            Err(error) => {
                eprintln!("{} ⛔ {}: {:#}\n", progress, test, error);
                errors.lock().unwrap().push((test, error));
            }
        }
    });

    let elapsed = start.elapsed().as_secs_f64();
    println!("\nFinished testing in {:.3}s", elapsed);

    TestResults {
        failures: failures.into_inner().unwrap(),
        timeouts: timeouts.into_inner().unwrap(),
        errors: errors.into_inner().unwrap()
    }
}

fn main() -> Result<()> {
    let options = Options::from_args();
    let Options { ref executer, ref test_dir, .. } = options;
    
    let executer: Box<dyn Executer>  = match executer {
        ExecuterKind::CC0 => Box::new(CC0Executer::new(&options)?),
        ExecuterKind::C0VM => Box::new(C0VMExecuter::new(&options)?),
        ExecuterKind::Coin => Box::new(CoinExecuter::new(&options)?)
    };

    // Load test cases
    let test_dir = fs::canonicalize(test_dir).context("Couldn't resolve the test directory")?;
    let tests = discover_tests::discover(&test_dir)?;

    eprintln!("Discovered {} tests", tests.len());

    // Run test cases
    let TestResults { failures, timeouts, errors } = run_tests(&*executer, &tests);
    
    // Report results
    let successes = tests.len() - failures.len() - errors.len();

    println!("\nTimeouts:\n");
    for test in timeouts.iter() {
        println!("⌛ {}", test);
    }

    println!("\nFailed tests:\n");
    for (test, failure) in failures.iter() {
        println!("❌ {}\n{}", test, failure);
    }

    println!("\nErrors:\n");
    for (test, error) in errors.iter() {
        println!("⛔ {}\n{:#}", test, error);
    }

    println!("\nTest summary: ");
    println!("✅ Passed: {}", successes);
    println!("⌛ Timeouts: {}", timeouts.len());
    println!("❌ Failed: {}", failures.len());
    println!("⛔ Error: {}", errors.len());

    Ok(())
}
