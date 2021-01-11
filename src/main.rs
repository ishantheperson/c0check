use std::env;
use std::sync::{Mutex, atomic::{self, AtomicUsize}};
use std::time::Instant;
use std::path::Path;
use rayon::prelude::*;
use anyhow::{Result, Error};

mod spec;
mod discover_tests;
mod parse_spec;
mod run_cc0;
mod checker;
mod executer;

use crate::spec::*;
use crate::checker::{Failure, TestResult};

fn run_tests<'a>(tests: &'a Vec<TestInfo>) -> (Vec<(&'a TestInfo, Failure)>, Vec<(&'a TestInfo, Error)>) {
    let failures: Mutex<Vec<(&TestInfo, Failure)>> = Mutex::new(Vec::new());
    let errors: Mutex<Vec<(&TestInfo, Error)>> = Mutex::new(Vec::new());

    let count = AtomicUsize::new(1);

    let start = Instant::now();

    tests.par_iter().for_each(|test| {
        let status = checker::run_test::<run_cc0::CC0Executer>(test);
        // Clear 'race condition' but 🤷‍♀️
        let i = count.fetch_add(1, atomic::Ordering::Relaxed);
        match status {
            Ok(TestResult::Success) => {
                eprintln!("{:5}/{:5} ✅ {}", i, tests.len(), test);
            },
            Ok(TestResult::Mismatch(failure)) => {
                eprintln!("{:5}/{:5} ❌ {}: {}", i, tests.len(), test, failure);
                failures.lock().unwrap().push((test, failure));
            },
            Err(error) => {
                eprintln!("{:5}/{:5} ⛔ {}: {:#}\n", i, tests.len(), test, error);
                errors.lock().unwrap().push((test, error));
            }
        }
    });

    let elapsed = start.elapsed().as_secs_f64();
    println!("\nFinished testing in {:.3}s", elapsed);

    (failures.into_inner().unwrap(), errors.into_inner().unwrap())
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();

    let test_path = match args.as_slice() {
        [_, path] => path,
        _ => {
            println!("usage: c0check <path to test directory>");
            return Ok(())
        }
    };

    let tests = discover_tests::discover(Path::new(test_path))?;

    eprintln!("Discovered {} tests", tests.len());

    let (failures, errors) = run_tests(&tests);
    let success = tests.len() - failures.len() - errors.len();

    println!("Failed tests:\n");
    for (test, failure) in failures.iter() {
        println!("❌ {}\n{}", test, failure);
    }

    println!("Errors:\n");
    for (test, error) in errors.iter() {
        println!("⛔ {}\n{:#}", test, error);
    }

    println!("Test summary: ");
    println!("✅ Passed: {}", success);
    println!("❌ Failed: {}", failures.len());
    println!("⛔ Error: {}", errors.len());

    Ok(())
}
