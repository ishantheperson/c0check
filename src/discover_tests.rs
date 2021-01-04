use std::{fs::{self, File}, io::BufReader};
use std::io::prelude::*;
use std::path::Path;

use anyhow::{Context, Result};

pub fn discover(base: &Path) -> Result<()> {
    let paths = fs::read_dir(base)
        .context(format!("Couldn't open the root test directory '{}'", base.display()))?
        .filter_map(Result::ok);

    for path in paths {
        let path = path.path();
        if path.is_dir() {
            match discover_directory(&path) {
                Ok(_) => todo!(),
                Err(e) => eprintln!("⚠: skipping '{}': {}", path.display(), e)
            }
        }
    }

    Ok(())
}

fn discover_directory(dir: &Path) -> Result<()> {
    let sources_test_path = dir.join("sources.test");

    // Try to look for sources.test
    match File::open(sources_test_path).ok() {
        Some(sources_test) => read_source_test(sources_test),
        None => read_test_files(dir)
    }
}

fn read_source_test(sources_test: File) -> Result<()> {
    let reader = BufReader::new(sources_test);
    let lines = reader.lines();

    // Parse each line of each spec

    Ok(())
}

fn read_test_files(dir: &Path) -> Result<()> {
    let test_paths = fs::read_dir(dir)
        .context(format!("Couldn't open a test directory '{}'", dir.display()))?
        .filter_map(Result::ok);

    for test in test_paths {
        let file = match File::open(test.path()) {
            Ok(file) => file,
            Err(_) => continue
        };

        let reader = BufReader::new(file);
        let spec_line = match reader.lines().next() {
            Some(Ok(line)) => line,
            Some(Err(_)) => continue,
            None => { eprintln!("⚠: file '{}' is empty", test.path().display()); continue }
        };

        // Parse spec line
    }

    Ok(())
}
