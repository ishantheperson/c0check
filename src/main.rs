use std::env;
use std::sync::{Mutex, atomic::{self, AtomicUsize}};
use std::time::Instant;
use std::path::Path;
use std::fs;
use std::process;
use rayon::prelude::*;
use anyhow::{Result, Error};

mod spec;
mod discover_tests;
mod parse_spec;
mod launcher;
mod checker;
mod executer;

use crate::spec::*;
use crate::executer::Executer;
use crate::launcher::{CC0Executer, C0VMExecuter, CoinExecuter};
use crate::checker::{Failure, TestResult};

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

fn parse_executer(name: &str) -> Box<dyn Executer> {
    match name {
        "cc0" => Box::new(CC0Executer()),
        "c0vm" => Box::new(C0VMExecuter()),
        "coin" => Box::new(CoinExecuter()),
        _ => print_usage(&format!("Invalid executer '{}'", name))
    }
}

fn print_usage(msg: &str) -> ! {
    if !msg.is_empty() {
        println!("{}", msg)
    }

    println!("usage: c0check [<cc0|c0vm|coin>=cc0] <path to test directory>");
    process::exit(0)
}

fn main() -> Result<()> {
    // Parse command line options
    let args: Vec<_> = env::args().collect();

    let (executer, test_path) = match args.as_slice() {
        [_, path] => (Box::new(CC0Executer()) as Box<dyn Executer>, path),
        [_, executer_name, path] => (parse_executer(executer_name), path),
        _ => {
            print_usage("");
        }
    };

    // Load test cases
    let test_dir = {
        let pathbuf = Path::new(test_path);
        fs::canonicalize(pathbuf)?
    };
    let tests = discover_tests::discover(&test_dir)?;

    eprintln!("Discovered {} tests", tests.len());

    // Run test cases
    let TestResults { failures, timeouts, errors } = run_tests(executer.as_ref(), &tests);
    
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
