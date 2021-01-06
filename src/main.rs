use std::env;
use std::path::Path;
use anyhow::{Result, Error};

mod spec;
mod discover_tests;
mod parse_spec;
mod run_cc0;
mod checker;
mod executer;

use crate::spec::*;
use crate::checker::{Failure, TestResult};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();

    let test_path = match args.as_slice() {
        [_, path] => path,
        _ => {
            eprintln!("usage: c0check <path to test directory>");
            return Ok(())
        }
    };

    let mut failures: Vec<(&TestInfo, Failure)> = Vec::new();
    let mut errors: Vec<(&TestInfo, Error)> = Vec::new();

    let tests = discover_tests::discover(Path::new(test_path))?;

    for (test, i) in tests.iter().zip(1usize..) {
        println!("Test {}/{}: {}", i, tests.len(), test);

        match checker::run_test::<run_cc0::CC0Executer>(test) {
            Ok(TestResult::Success) => {
                println!("✅ Test passed")
            },
            Ok(TestResult::Mismatch(failure)) => {
                println!("❌ {}", failure);
                failures.push((test, failure));
            },
            Err(error) => {
                println!("⛔ {:#}\n", error);
                errors.push((test, error));
            }
        }
    }

    let success = tests.len() - failures.len() - errors.len();

    println!("Failed tests:\n");
    for (test, failure) in failures.iter() {
        println!("❌ {}\n{}\n", test, failure);
    }

    println!("Errors:\n");
    for (test, error) in errors.iter() {
        println!("⛔ {}\n{:#}\n", test, error);
    }

    println!("Test summary: ");
    println!("✅ Passed: {}", success);
    println!("❌ Failed: {}", failures.len());
    println!("⛔ Error: {}", errors.len());

    Ok(())
}
