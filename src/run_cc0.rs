use std::{os::unix::prelude::RawFd, process};
use std::env;
use std::fs;
use std::path::Path;
use std::ffi::CString;
use std::mem::MaybeUninit;
use lazy_static::lazy_static;
use nix::{unistd, sys::wait::{self, WaitStatus}, sys::signal::Signal};
use nix::fcntl::OFlag;
use anyhow::{anyhow, Context, Result};

use crate::spec::*;
use crate::executer::*;

pub struct CC0Executer();

impl Executer for CC0Executer {
    fn run_test(info: &TestExecutionInfo) -> Result<(String, Behavior)> {
        let compilation_result = compile(info)?;
        match compilation_result {
            Ok(name) => execute(info, &name),
            Err(output) => Ok((output, Behavior::CompileError))
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
        use nix::sys::stat::Mode;
        nix::fcntl::open("/dev/null", OFlag::O_WRONLY, Mode::empty()).expect("Couldn't open /dev/null")
    };

    static ref CC0_PATH: String = {
        match env::var("C0_HOME") {
            Ok(path) => format!("{}/bin/cc0", path),
            Err(_) => "cc0".to_string()
        }
    };
}

const STDOUT_FILENO: i32 = 1;
const STDERR_FILENO: i32 = 2;

/// Timeout for compilation
const COMPILATION_TIMEOUT: u32 = 15;
/// Timeout for running tests
const TEST_TIMEOUT: u32 = 10;

const CC0_GCC_FAILURE_CODE: i32 = 2;
const EXEC_FAILURE_CODE: i32 = 100;
const RUST_PANIC_CODE: i32 = 101;

fn compile(test: &TestExecutionInfo) -> Result<Result<CString, String>> {
    let compiler = CString::new(&*CC0_PATH.as_str()).unwrap();

    // Create args
    let mut args: Vec<CString> = Vec::new();
    args.push(compiler.clone());
    args.extend(test.compiler_options.iter().map(string_to_cstring));
    args.extend(test.sources.iter().map(string_to_cstring));
    
    let out_file: CString = str_to_cstring(&format!("{}/a.out{}", env::current_dir().unwrap().display(), unistd::gettid()));
    args.push(str_to_cstring("-o"));
    args.push(out_file.clone());

    // Create a pipe to record stdout and stuff
    let (read_pipe, write_pipe) = unistd::pipe2(OFlag::O_NONBLOCK).context("When creating a pipe to record CC0 output")?;

    match unsafe { unistd::fork().context("when spawning CC0")? } {
        unistd::ForkResult::Child => {
            unistd::close(read_pipe).unwrap();
            redirect_io(write_pipe);
            unistd::alarm::set(COMPILATION_TIMEOUT);

            let _ = unistd::execvp( &compiler, &args).expect("Couldn't exec!");
            process::exit(EXEC_FAILURE_CODE);
        },

        unistd::ForkResult::Parent { child } => {
            let status = wait::waitpid(child, None).expect("Failed to wait() for compiler process");
            let output = read_from_pipe(read_pipe, write_pipe)?;
            
            match status {
                WaitStatus::Exited(_, 0) => Ok(Ok(out_file)),
                WaitStatus::Exited(_, 1) => Ok(Err(output)),
                WaitStatus::Exited(_, CC0_GCC_FAILURE_CODE) => {
                    Err(anyhow!("CC0 failed to invoke GCC"))
                },
                WaitStatus::Exited(_, EXEC_FAILURE_CODE) => Err(anyhow!("Failed to exec cc0")),
                WaitStatus::Exited(_, RUST_PANIC_CODE) => Err(anyhow!("CC0 process panic'd")),
                WaitStatus::Signaled(_, Signal::SIGALRM, _) => Err(anyhow!("CC0 timed out")),
                status => Err(anyhow!("CC0 unexpectedly failed: {:?}", status)) // unexpected
            }
        }
    }
}

fn execute(info: &TestExecutionInfo, executable: &CString) -> Result<(String, Behavior)> {
    let result_file = format!("{}/c0_result{}", env::current_dir().unwrap().display(), unistd::gettid());
    let result_env = string_to_cstring(&format!("C0_RESULT_FILE={}", result_file));

    let (read_pipe, write_pipe) = unistd::pipe2(OFlag::O_NONBLOCK).context("When creating a pipe to record test output")?;

    match unsafe { unistd::fork().context("when spawning test process")? } {
        unistd::ForkResult::Child => {
            env::set_current_dir(Path::new(&*info.directory)).expect("Couldn't change to the test directory");
            redirect_io(write_pipe);
            unistd::close(read_pipe).unwrap();
            unistd::alarm::set(TEST_TIMEOUT);

            let _ = unistd::execve::<&CString, &CString>(executable, &[executable], &[&result_env]);
            // Couldn't exec
            process::exit(EXEC_FAILURE_CODE);
        },

        unistd::ForkResult::Parent { child } => {
            let status = wait::waitpid(child, None).expect("Failed to wait() for test program");
            let output = read_from_pipe(read_pipe, write_pipe)?;
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

            let behavior = match status {
                WaitStatus::Exited(_, 0) => 
                    if result.len() == 5 {
                        let bytes = [result[1], result[2], result[3], result[4]];
                        Behavior::Return(Some(i32::from_ne_bytes(bytes)))
                    }
                    else {
                        return Err(anyhow!("C0 program exited succesfully, but no return value was written"))
                    },
                WaitStatus::Exited(_, 1) => Behavior::Failure,
                WaitStatus::Exited(_, EXEC_FAILURE_CODE) => return Err(anyhow!("Failed to exec the test program")),
                WaitStatus::Exited(_, RUST_PANIC_CODE) => return Err(anyhow!("Test program process panic'd")),
                WaitStatus::Exited(_, status) => return Err(anyhow!("Unexpected program exit status '{}'", status)),
                
                WaitStatus::Signaled(_, signal, _) => match signal {
                    Signal::SIGSEGV => Behavior::Segfault,
                    Signal::SIGALRM => Behavior::InfiniteLoop,
                    Signal::SIGFPE => Behavior::DivZero,
                    Signal::SIGABRT => Behavior::Abort,
                    other => return Err(anyhow!("Program exited with unexpected signal '{}'", other))
                }   
                status => return Err(anyhow!("Program unexpectedly failed: {:?}", status))
            };

            Ok((output, behavior))
        },
    }
}

fn redirect_io(target_file: RawFd) {
    unistd::dup2(target_file, STDOUT_FILENO).expect("Couldn't redirect stdout");
    unistd::dup2(target_file, STDERR_FILENO).expect("Couldn't redirect stderr");
}

fn read_from_pipe(read_pipe: RawFd, write_pipe: RawFd) -> Result<String> {
    // Capture CC0 output
    unistd::close(write_pipe).unwrap();
    let mut buf: [u8; 1024] = unsafe { MaybeUninit::uninit().assume_init() };
    let num_bytes = match unistd::read(read_pipe, &mut buf) {
        Ok(n) => n,
        // We set O_NONBLOCK on the pipe and EAGAIN is raised if
        // this read would block us. We only call this function
        // after we have wait()'d for the process, so if
        // we get EAGAIN we want to ignore it
        Err(nix::Error::Sys(nix::errno::Errno::EAGAIN)) => 0,
        err => err.context("When reading CC0 output")?
    };
    let output = String::from_utf8_lossy(&buf[..num_bytes]).to_string();
    unistd::close(read_pipe).unwrap();
    Ok(output)
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

        let name = compile(&test.execution)?.map_err(|e| anyhow!(e))?;
        assert_eq!(execute(&test.execution, &name)?.1, Behavior::Return(Some(0)));

        Ok(())
    }
}

fn str_to_cstring(s: &str) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}

fn string_to_cstring(s: &String) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}
