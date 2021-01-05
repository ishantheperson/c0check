use std::{fs::{self, File}, io::BufReader};
use std::io::prelude::*;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::parse_spec::{self, ParseOptions};
use crate::spec::*;

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

fn discover_directory(dir: &Path) -> Result<Vec<TestInfo>> {
    let sources_test_path = dir.join("sources.test");

    // Try to look for sources.test
    match File::open(sources_test_path).ok() {
        Some(sources_test) => read_source_test(dir, sources_test),
        None => read_test_files(dir)
    }
}

fn read_source_test(dir: &Path, sources_test: File) -> Result<Vec<TestInfo>> {
    let reader = BufReader::new(sources_test);
    let lines = reader.lines();
    let mut tests = Vec::new();

    for (line, lineno) in lines.zip(1usize..) {
        let line = line?;

        if line.trim().is_empty() {
            continue
        }
        
        let splitted: Vec<_> = line.splitn(2, '~').collect();

        // TODO: use split_once
        let (spec, cmdline) = match splitted.as_slice() {
            [spec, cmdline] => (*spec, *cmdline),
            _ => return Err(anyhow!("sources.test is missing '~' on line {}", lineno)),
        };

        let specs = parse_spec::parse(spec, ParseOptions { require_test_marker: false })
            .context(format!("in sources.test on line {}", lineno))?;

        let mut sources: Vec<String> = Vec::new();
        let mut compiler_options: Vec<String> = Vec::new();
        for arg in cmdline.split_ascii_whitespace() {
            if !arg.starts_with('-') && (arg.ends_with(".c0") || arg.ends_with(".c1")) {
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
            },
            specs
        };

        tests.push(test)
    }

    println!("Found {} tests", tests.len());

    // Parse each line of each spec
    Ok(tests)
}

fn read_test_files(dir: &Path) -> Result<Vec<TestInfo>> {
    let test_paths = fs::read_dir(dir)
        .context(format!("Couldn't open a test directory '{}'", dir.display()))?
        .filter_map(Result::ok);

    let mut tests = Vec::new();

    for test in test_paths {
        let path = test.path();

        match path.extension().map(|ext| ext.to_str().expect("Invalid path character")) {
            Some("c0") | Some("c1") => (),
            _ => continue
        };

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => continue
        };

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
            },
            specs
        };

        tests.push(test)
    }

    Ok(tests)
}
