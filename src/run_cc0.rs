use std::ffi::{CString};
use std::process;
use std::fs;
use std::path::Path;
use nix::{unistd, sys::wait::{self, WaitStatus}, sys::signal::Signal};
use anyhow::{anyhow, Context, Result};

use crate::spec::*;
use crate::executer::*;

pub struct CC0Executer();

impl Executer for CC0Executer {
    fn run_test(info: &TestExecutionInfo) -> Result<Behavior> {
        let compilation_result = compile(info)?;
        match compilation_result {
            Some(name) => execute(&name),
            None => Ok(Behavior::CompileError)
        }
    }

    fn properties() -> ExecuterProperties {
        ExecuterProperties {
            libraries: true,
            garbage_collected: true,
            safe: true,
            typechecked: true,
            name: "cc0".to_string()
        }
    }
}

/// Timeout for compilation
static COMPILATION_TIMEOUT: u32 = 10;

fn compile(test: &TestExecutionInfo) -> Result<Option<CString>> {
    fn str_to_cstring(s: &str) -> CString {
        CString::new(s.as_bytes()).unwrap()        
    }

    fn string_to_cstring(s: &String) -> CString {
        CString::new(s.as_bytes()).unwrap()        
    }

    let compiler = CString::new("/home/ishan/c0-developer/cc0/bin/cc0").unwrap();

    // Create args
    let mut args: Vec<CString> = Vec::new();
    args.push(compiler.clone());
    args.extend(test.compiler_options.iter().map(string_to_cstring));
    args.extend(test.sources.iter().map(string_to_cstring));
    
    let out_file: CString = str_to_cstring(&format!("a.out{}", unistd::gettid()));
    args.push(str_to_cstring("-o"));
    args.push(out_file.clone());

    match unsafe { unistd::fork().context("when spawning CC0")? } {
        unistd::ForkResult::Child => {
            unistd::alarm::set(COMPILATION_TIMEOUT);
            // TODO: redirect IO
            let _ = unistd::execvp( &compiler, &args);
            process::exit(2);
        },
        unistd::ForkResult::Parent { child } => {
            match wait::waitpid(child, None).expect("Failed to wait() for compiler process") {
                WaitStatus::Exited(_, 0) => Ok(Some(out_file)),
                WaitStatus::Exited(_, 1) => Ok(None),
                WaitStatus::Exited(_, 2) => Err(anyhow!("Failed to exec cc0")),
                WaitStatus::Signaled(_, Signal::SIGALRM, _) => Err(anyhow!("CC0 timed out")),
                status => Err(anyhow!("CC0 unexpectedly failed: {:?}", status)) // unexpected
            }
        }
    }
}

/// Timeout for running tests
static TEST_TIMEOUT: u32 = 10;

fn execute(executable: &CString) -> Result<Behavior> {
    match unsafe { unistd::fork().context("when spawning test process")? } {
        unistd::ForkResult::Child => {
            // TODO: redirect IO, set result file, change to test directory first
            unistd::alarm::set(TEST_TIMEOUT);
            let _ = unistd::execve::<&CString, &CString>(executable, &[executable], &[]);
            process::exit(2);
        },

        unistd::ForkResult::Parent { child } => {
            let status = match wait::waitpid(child, None).expect("Failed to wait() for test program") {
                WaitStatus::Exited(_, 0) => Ok(Behavior::Return(Some(0))),
                WaitStatus::Exited(_, 1) => Ok(Behavior::Failure),
                WaitStatus::Exited(_, 2) => Err(anyhow!("Failed to exec the test program")),
                WaitStatus::Exited(_, status) => Err(anyhow!("Unexpected program exit status '{}'", status)),

                WaitStatus::Signaled(_, signal, _) => match signal {
                    Signal::SIGSEGV => Ok(Behavior::Segfault),
                    Signal::SIGALRM => Ok(Behavior::InfiniteLoop),
                    Signal::SIGFPE => Ok(Behavior::DivZero),
                    Signal::SIGABRT => Ok(Behavior::Abort),
                    other => Err(anyhow!("Program exited with unexpected signal '{}'", other))
                }   
                status => Err(anyhow!("CC0 unexpectedly failed: {:?}", status)) // unexpected
            };

            fs::remove_file(Path::new(&executable.to_str().unwrap()))
                .context("when removing test program")?;

            status
        },
    }
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[test]
    fn test() -> Result<()> {
        let test = TestInfo {
            execution: TestExecutionInfo {
                compiler_options: vec![],
                sources: vec!["test_resources/test.c0".to_string()],
            },
            specs: vec![]
        };

        let name = compile(&test.execution)?.ok_or(anyhow!("Test did not compile"))?;
        assert_eq!(execute(&name)?, Behavior::Return(Some(0)));

        Ok(())
    }
}