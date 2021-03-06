use std::{fs::{self, File}, io::BufReader};
use std::io::prelude::*;
use std::path::Path;
use std::sync::Arc;
use anyhow::{anyhow, Context, Result};

use crate::parse_spec::{self, ParseOptions};
use crate::spec::*;

/// Discovers all CC0 test cases in all subdirectories of 'base'.
/// This assumes base contains directories which contain test cases.
/// If a subdirectory contains 'sources.test', then that file will be
/// read to discover test cases.
pub fn discover(base: &Path) -> Result<Vec<TestInfo>> {
    let paths = fs::read_dir(base)
        .context(format!("Couldn't open the root test directory '{}'", base.display()))?
        .filter_map(Result::ok);

    let mut tests = Vec::new();

    for path in paths {
        let path = path.path();
        if path.is_dir() {
            match discover_directory(&path) {
                Ok(new_tests) => tests.extend(new_tests.into_iter()),
                Err(e) => eprintln!("⚠: skipping '{}': {:#}", path.display(), e)
            }
        }
    }

    Ok(tests)
}

/// Loads all test cases inside a directory
fn discover_directory(dir: &Path) -> Result<Vec<TestInfo>> {
    let sources_test_path = dir.join("sources.test");

    // Try to look for sources.test
    match File::open(sources_test_path).ok() {
        Some(sources_test) => read_sources_file(dir, sources_test),
        None => read_test_files(dir)
    }
}

/// Parses a 'sources.test' 
fn read_sources_file(dir: &Path, sources_test: File) -> Result<Vec<TestInfo>> {
    let reader = BufReader::new(sources_test);
    let lines = reader.lines();
    let mut tests = Vec::new();

    let directory = Arc::<str>::from(dir.to_str().unwrap());

    for (line, lineno) in lines.zip(1usize..) {
        let line = line?;

        if line.trim().is_empty() {
            continue
        }
        
        let (spec, cmdline) = line
            .split_once('~')
            .ok_or_else(|| anyhow!("sources.test is missing '~' on line {}", lineno))?;

        let specs = parse_spec::parse(spec, ParseOptions { require_test_marker: false })
            .context(format!("in sources.test on line {}", lineno))?;

        let mut sources: Vec<String> = Vec::new();
        let mut compiler_options: Vec<String> = Vec::new();
        for arg in cmdline.split_ascii_whitespace() {
            if !arg.starts_with('-') && ([".c0", ".c1", ".h0", ".h1"].iter().any(|&ext| arg.ends_with(ext))) {
                let path = dir.join(arg);
                sources.push(path.into_os_string().into_string().expect("Invalid path character"));
            }
            else {
                compiler_options.push(String::from(arg));
            }
        }

        let test = TestInfo {
            execution: TestExecutionInfo {
                sources,
                compiler_options,
                directory: directory.clone()
            },
            specs
        };

        tests.push(test)
    }

    Ok(tests)
}

/// Loads all .c0, .c1 test files in the given directory
fn read_test_files(dir: &Path) -> Result<Vec<TestInfo>> {
    let test_paths = fs::read_dir(dir)
        .context(format!("Couldn't open a test directory '{}'", dir.display()))?
        .filter_map(Result::ok);

    let mut tests = Vec::new();
    let directory = Arc::<str>::from(dir.to_str().unwrap());

    for test in test_paths {
        let path = test.path();

        // Check if its a c0 or c1 file and open it if it is
        match path.extension().map(|ext| ext.to_str().expect("Invalid path character")) {
            Some("c0") | Some("c1") => (),
            _ => continue
        };

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => continue
        };

        // Read spec line
        let reader = BufReader::new(file);
        let spec_line = match reader.lines().next() {
            Some(Ok(line)) => line,
            Some(Err(_)) => continue,
            None => { eprintln!("⚠: file '{}' is empty", path.display()); continue }
        };

        // Parse spec line
        let specs: Specs = match parse_spec::parse(&spec_line, ParseOptions { require_test_marker: true }) {
            Ok(specs) => specs,
            Err(parse_spec::SpecParseError::NotSpec) => continue,
            Err(e) => { eprintln!("⚠: skipping '{}': {:#}", path.display(), e); continue }
        };

        let test = TestInfo {
            execution: TestExecutionInfo {
                sources: vec![String::from(test.path().to_str().expect("Invalid character in path"))],
                compiler_options: Vec::new(),
                directory: directory.clone()
            },
            specs
        };

        tests.push(test)
    }

    Ok(tests)
}

#[cfg(test)]
mod discover_tests {
    use super::*;

    use std::env;

    #[test]
    fn test() -> Result<()> {
        let testdir = env::var("C0_HOME")?;
        let tests = discover(&Path::new(&format!("{}/tests/", testdir)))?;

        assert_eq!(tests.len(), 3761);

        Ok(())
    }
}