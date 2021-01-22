use std::path::Path;
use std::fs;
use std::env;
use std::sync::atomic::{self, AtomicUsize};
use std::ffi::{CStr, CString};
use anyhow::{Result, Context};
 
use crate::spec::*;
use crate::executer::{Executer, ExecuterProperties};
use crate::launcher::*;

pub struct CC0Executer();

impl Executer for CC0Executer {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)> {
        let mut args: Vec<CString> = Vec::new();
        args.extend(test.compiler_options.iter().map(string_to_cstring));
        args.extend(test.sources.iter().map(string_to_cstring));
        
        // Global counter to come up with unique names for output files
        static mut test_counter: AtomicUsize = AtomicUsize::new(0);

        let out_file: CString = unsafe {
            let current_dir = env::current_dir().unwrap();
            let next_id = test_counter.fetch_add(1, atomic::Ordering::Relaxed);
            str_to_cstring(&format!("{}/a.out{}", current_dir.display(), next_id))
        };
        args.push(str_to_cstring("-vo"));
        args.push(out_file.clone());

        let compilation_result = compile(&args)?;
        if let Err(output) = compilation_result {
            return Ok((output, Behavior::CompileError))
        }
        
        let exec_result = execute(test, &out_file, CC0_TEST_TIMEOUT);
        if let Err(e) = fs::remove_file(Path::new(&out_file.to_str().unwrap())) {
            eprintln!("❗ Couldn't delete a.out file: {:#}", e);
        }

        // Remove debugging symbol directory on MacOS 
        if cfg!(target_os = "macos") {
            let dsym_str = format!("{}.dSYM", out_file.to_str().unwrap());
            let dsym_dir = Path::new(&dsym_str);
            if let Err(e) = fs::remove_dir_all(dsym_dir) {
                eprintln!("❗ Couldn't delete .dSYM directory: {:#}", e);
            }
        }

        exec_result
    }

    fn properties(&self) -> ExecuterProperties {
        ExecuterProperties {
            libraries: true,
            garbage_collected: true,
            safe: true,
            typechecked: true,
            name: "cc0"
        }
    }
}

pub struct C0VMExecuter();

impl Executer for C0VMExecuter {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)> {
        // Compile test case
        let mut args: Vec<CString> = Vec::new();
        args.extend(test.compiler_options.iter().map(string_to_cstring));
        args.extend(test.sources.iter().map(string_to_cstring));
        
        static mut test_counter: AtomicUsize = AtomicUsize::new(0);
        
        let out_file: CString = unsafe {
            let current_dir = env::current_dir().unwrap();
            let next_id = test_counter.fetch_add(1, atomic::Ordering::Relaxed);
            str_to_cstring(&format!("{}/a.out{}.bc0", current_dir.display(), next_id))
        };
        args.push(str_to_cstring("-vbo"));
        args.push(out_file.clone());

        let compilation_result = compile(&args)?;
        if let Err(output) = compilation_result {
            return Ok((output, Behavior::CompileError))
        }

        // Run test case
        let exec_result = 
            execute_with_args(
                test, 
                C0VM_PATH.as_ref(), 
                &[out_file.as_ref()], 
                C0VM_TEST_TIMEOUT);
        
        if let Err(e) = fs::remove_file(out_file.to_str().unwrap()) {
            eprintln!("❗ Couldn't delete bc0 file: {:#}", e);
        }

        exec_result
    }

    fn properties(&self) -> ExecuterProperties {
        ExecuterProperties {
            libraries: true,
            garbage_collected: false,
            safe: true,
            typechecked: true,
            name: "cc0_c0vm"
        }
    }
}

pub struct CoinExecuter();

impl Executer for CoinExecuter {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)> {
        // Check if it uses C1, if so then skip the test
        if test.sources.iter().any(|source| source.ends_with(".c1")) {
            return Ok(("<C1 test skipped>".to_string(), Behavior::Skipped))
        }

        // No need to compile tests for the C0in-trepter
        let mut args: Vec<CString> = Vec::new();
        args.extend(test.compiler_options.iter().map(string_to_cstring));
        args.extend(test.sources.iter().map(string_to_cstring));

        execute_with_args(test, COIN_EXEC_PATH.as_ref(), &args, COIN_TEST_TIMEOUT)
    }

    fn properties(&self) -> ExecuterProperties {
        ExecuterProperties {
            libraries: true,
            garbage_collected: false,
            safe: true,
            typechecked: true,
            name: "coin"
        }
    }
}

fn str_to_cstring(s: &str) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}

fn string_to_cstring(s: &String) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}
