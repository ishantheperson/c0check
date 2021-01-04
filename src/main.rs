use std::env;
use std::path::Path;
use anyhow::Result;

mod discover_tests;
mod parse_spec;
mod run_cc0;

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();

    match args.as_slice() {
        [_, path] => {
            let tests = discover_tests::discover(Path::new(path))?;
            for test in tests.iter() {
                println!("{:?}", test);
            }

        },
        _ => {
            eprintln!("usage: c0check <path to test directory>");
        }
    }

    Ok(())
}
