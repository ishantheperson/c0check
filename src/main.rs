use std::env;
use std::sync::{Mutex, atomic::{self, AtomicUsize}};
use std::time::Instant;
use std::path::Path;
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

fn run_tests<'a>(executer: Box<dyn Executer>, tests: &'a Vec<TestInfo>) -> TestResults<'a> {
    let failures: Mutex<Vec<(&TestInfo, Failure)>> = Mutex::new(Vec::new());
    let timeouts: Mutex<Vec<&TestInfo>> = Mutex::new(Vec::new());
    let errors: Mutex<Vec<(&TestInfo, Error)>> = Mutex::new(Vec::new());

    let count = AtomicUsize::new(1);

    let start = Instant::now();

    tests.par_iter().for_each(|test| {
        let status = checker::run_test(&executer, test);
        // Clear 'race condition' but 🤷‍♀️
        let i = count.fetch_add(1, atomic::Ordering::Relaxed);
        match status {
            Ok(TestResult::Success) => {
                eprintln!("{:4}/{:4} ✅ {}", i, tests.len(), test);
            },
            Ok(TestResult::Mismatch(failure)) => {
                if failure.is_timeout() {
                    eprintln!("{:4}/{:4} ⌛ {}", i, tests.len(), test);
                    timeouts.lock().unwrap().push(test);
                }
                else {
                    eprintln!("{:4}/{:4} ❌ {}: {}", i, tests.len(), test, failure);
                    failures.lock().unwrap().push((test, failure));
                }
            },
            Err(error) => {
                eprintln!("{:4}/{:4} ⛔ {}: {:#}\n", i, tests.len(), test, error);
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

fn parse_executer(name: &String) -> Box<dyn Executer> {
    match name.as_str() {
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
    let args: Vec<_> = env::args().collect();

    let (executer, test_path) = match args.as_slice() {
        [_, path] => (Box::new(CC0Executer()) as Box<dyn Executer>, path),
        [_, executer_name, path] => (parse_executer(executer_name), path),
        _ => {
            print_usage("usage: c0check <path to test directory>");
        }
    };

    let tests = discover_tests::discover(Path::new(test_path))?;

    eprintln!("Discovered {} tests", tests.len());

    let TestResults { failures, timeouts, errors } = run_tests(executer, &tests);
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
