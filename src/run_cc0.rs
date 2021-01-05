use std::{ffi::{CString}};
use std::process;
use std::env;
use std::fs;
use std::path::Path;
use lazy_static::lazy_static;
use nix::{unistd, sys::wait::{self, WaitStatus}, sys::signal::Signal};
use anyhow::{anyhow, Context, Result};

use crate::spec::*;
use crate::executer::*;

pub struct CC0Executer();

impl Executer for CC0Executer {
    fn run_test(info: &TestExecutionInfo) -> Result<Behavior> {
        let compilation_result = compile(info)?;
        match compilation_result {
            Some(name) => execute(info, &name),
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

lazy_static! {
    static ref DEVNULL: i32 = {
        use nix::fcntl::OFlag;
        use nix::sys::stat::Mode;
        nix::fcntl::open("/dev/null", OFlag::O_WRONLY, Mode::empty()).expect("Couldn't open /dev/null")
    };
}

static STDOUT_FILENO: i32 = 1;
static STDERR_FILENO: i32 = 2;

/// Timeout for compilation
static COMPILATION_TIMEOUT: u32 = 10;

fn compile(test: &TestExecutionInfo) -> Result<Option<CString>> {
    let compiler = CString::new("/home/ishan/c0-developer/cc0/bin/cc0").unwrap();

    // Create args
    let mut args: Vec<CString> = Vec::new();
    args.push(compiler.clone());
    args.extend(test.compiler_options.iter().map(string_to_cstring));
    args.extend(test.sources.iter().map(string_to_cstring));
    
    let out_file: CString = str_to_cstring(&format!("{}/a.out{}", env::current_dir().unwrap().display(), unistd::gettid()));
    args.push(str_to_cstring("-o"));
    args.push(out_file.clone());

    match unsafe { unistd::fork().context("when spawning CC0")? } {
        unistd::ForkResult::Child => {
            unistd::alarm::set(COMPILATION_TIMEOUT);
            
            unistd::dup2(*DEVNULL, STDOUT_FILENO).expect("Couldn't redirect stdout");
            unistd::dup2(*DEVNULL, STDERR_FILENO).expect("Couldn't redirect stderr");

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

fn execute(info: &TestExecutionInfo, executable: &CString) -> Result<Behavior> {
    let result_file = format!("{}/c0_result{}", env::current_dir().unwrap().display(), unistd::gettid());
    let result_env = string_to_cstring(&format!("C0_RESULT_FILE={}", result_file));

    match unsafe { unistd::fork().context("when spawning test process")? } {
        unistd::ForkResult::Child => {
            env::set_current_dir(Path::new(&*info.directory)).expect("Couldn't change to the test directory");
            
            unistd::alarm::set(TEST_TIMEOUT);

            unistd::dup2(*DEVNULL, STDOUT_FILENO).expect("Couldn't redirect stdout");
            unistd::dup2(*DEVNULL, STDERR_FILENO).expect("Couldn't redirect stderr");

            let _ = unistd::execve::<&CString, &CString>(executable, &[executable], &[&result_env]);
            // Couldn't exec
            process::exit(2);
        },

        unistd::ForkResult::Parent { child } => {
            let status = wait::waitpid(child, None).expect("Failed to wait() for test program");
            
            let result = match fs::read(&result_file) {
                Ok(bytes) => {
                    fs::remove_file(Path::new(&result_file))
                        .context("when removing test program result file")?;                    
                    bytes
                }
                Err(_) => Vec::new()
            };
            
            fs::remove_file(Path::new(&executable.to_str().unwrap()))
                .context("when removing test program")?;

            match status {
                WaitStatus::Exited(_, 0) => 
                    if result.len() == 5 {
                        let bytes = [result[1], result[2], result[3], result[4]];
                        Ok(Behavior::Return(Some(i32::from_ne_bytes(bytes))))
                    }
                    else {
                        Err(anyhow!("C0 program exited succesfully, but no return value was written"))
                    },
                WaitStatus::Exited(_, 1) => Ok(Behavior::Failure),
                WaitStatus::Exited(_, 2) => Err(anyhow!("Failed to exec the test program")),
                WaitStatus::Exited(_, 101) => Err(anyhow!("Test program process panic'd")),
                WaitStatus::Exited(_, status) => Err(anyhow!("Unexpected program exit status '{}'", status)),
                
                WaitStatus::Signaled(_, signal, _) => match signal {
                    Signal::SIGSEGV => Ok(Behavior::Segfault),
                    Signal::SIGALRM => Ok(Behavior::InfiniteLoop),
                    Signal::SIGFPE => Ok(Behavior::DivZero),
                    Signal::SIGABRT => Ok(Behavior::Abort),
                    other => Err(anyhow!("Program exited with unexpected signal '{}'", other))
                }   
                status => Err(anyhow!("CC0 unexpectedly failed: {:?}", status)) // unexpected
            }
        },
    }
}

#[cfg(test)]
mod compile_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test() -> Result<()> {
        let test = TestInfo {
            execution: TestExecutionInfo {
                compiler_options: vec![],
                sources: vec!["test_resources/test.c0".to_string()],
                directory: Arc::from("./")
            },
            specs: vec![]
        };

        let name = compile(&test.execution)?.ok_or(anyhow!("Test did not compile"))?;
        assert_eq!(execute(&test.execution, &name)?, Behavior::Return(Some(0)));

        Ok(())
    }
}

fn str_to_cstring(s: &str) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}

fn string_to_cstring(s: &String) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}
