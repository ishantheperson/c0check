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

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();

    let test_path = match args.as_slice() {
        [_, path] => path,
        _ => {
            eprintln!("usage: c0check <path to test directory>");
            return Ok(())
        }
    };

    let mut success = 0;
    let mut failures: Vec<&TestInfo> = Vec::new();
    let mut errors: Vec<(&TestInfo, Error)> = Vec::new();

    let tests = discover_tests::discover(Path::new(test_path))?;
    for (test, i) in tests.iter().zip(1..) {
        println!("Test {}/{}: {}", i, tests.len(), test);

        match checker::run_test::<run_cc0::CC0Executer>(test) {
            Ok(true) => {
                println!("✅ Test passed");
                success += 1;
            },
            Ok(false) => {
                println!("❌ Test failed");
                failures.push(test)
            },
            Err(e) => {
                println!("⛔ Error when running test: {:#}\n", e);
                errors.push((test, e));
            }
        }
    }

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
