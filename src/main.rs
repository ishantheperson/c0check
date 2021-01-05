use std::env;
use std::path::Path;
use std::sync::Mutex;
use rayon::prelude::*;
use anyhow::{Result, Error};

mod spec;
mod discover_tests;
mod parse_spec;
mod run_cc0;
mod checker;
mod executer;

use crate::spec::*;

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();

    let test_path = match args.as_slice() {
        [_, path] => path,
        _ => {
            eprintln!("usage: c0check <path to test directory>");
            return Ok(())
        }
    };

    let failures: Mutex<Vec<&TestInfo>> = Mutex::new(Vec::new());
    let errors: Mutex<Vec<(&TestInfo, Error)>> = Mutex::new(Vec::new());

    let tests = discover_tests::discover(Path::new(test_path))?;

    tests.par_iter().for_each(|test| {
        match checker::run_test::<run_cc0::CC0Executer>(test) {
            Ok(true) => {
                println!("✅ Test passed");
            },
            Ok(false) => {
                println!("❌ Test failed");
                failures.lock().unwrap().push(test);
            },
            Err(e) => {
                println!("⛔ Error when running test: {:#}\n", e);
                errors.lock().unwrap().push((test, e));
            }
        }
    });

    let failures = failures.lock().unwrap();
    let errors = errors.lock().unwrap();
    let success = tests.len() - failures.len() - errors.len();

    println!("--------------------------------");

    println!("Failed tests:\n");
    for &failure in failures.iter() {
        println!("❌ {}", failure);
    }

    println!("\nErrors:\n");
    for (test, error) in errors.iter() {
        println!("⛔ {}\n{:#}\n", test, error);
    }

    println!("Test summary: ");
    println!("✅ Passed: {}", success);
    println!("❌ Failed: {}", failures.len());
    println!("⛔ Error: {}", errors.len());

    Ok(())
}
