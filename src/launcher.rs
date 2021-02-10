#![allow(non_upper_case_globals)]

use std::process;
use std::os::unix::io::RawFd;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::atomic::{self, AtomicUsize};
use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;

use nix::unistd::{self, ForkResult};
use nix::sys::wait::{self, WaitStatus};
use nix::sys::signal::Signal;
use nix::libc::{self, STDOUT_FILENO, STDERR_FILENO};

use anyhow::{anyhow, Context, Result};

use crate::spec::*;

const CC0_GCC_FAILURE_CODE: i32 = 2;
const EXEC_FAILURE_CODE: i32 = 100;
const RUST_PANIC_CODE: i32 = 101;

pub fn compile<CC0Path: AsRef<CStr>, Arg: AsRef<CStr>>(
    cc0: CC0Path, 
    args: &[Arg],
    timeout: u64,
    memory: u64) -> Result<Result<(), String>> 
{
    // Create argv
    let mut argv = vec![cc0.as_ref()];
    argv.extend(args.iter().map(|arg| arg.as_ref()));

    // Create a pipe to record stdout and stderr from the subprocess
    let (read_pipe, write_pipe) = unistd::pipe().context("When creating a pipe to record CC0 output")?;

    match unsafe { unistd::fork().context("when spawning CC0")? } {
        ForkResult::Child => {
            unistd::close(read_pipe).unwrap();
            redirect_output(write_pipe);
            set_resource_limits(memory, timeout);

            let _ = unistd::execvp(cc0.as_ref(), &argv);
            unsafe { libc::_exit(EXEC_FAILURE_CODE); }
        },

        ForkResult::Parent { child } => {
            let output = read_from_pipe(read_pipe, write_pipe).unwrap_or("<couldn't read output>".to_string());
            let status = wait::waitpid(child, None).expect("Failed to wait() for compiler process");
            
            match status {
                WaitStatus::Exited(_, 0) => Ok(Ok(())),
                WaitStatus::Exited(_, 1) => Ok(Err(output)),
                WaitStatus::Exited(_, CC0_GCC_FAILURE_CODE) => Err(anyhow!("CC0 failed to invoke GCC")).context(output),
                WaitStatus::Exited(_, EXEC_FAILURE_CODE) => Err(anyhow!("Failed to exec cc0")).context(output),
                WaitStatus::Exited(_, RUST_PANIC_CODE) => Err(anyhow!("CC0 process panic'd")).context(output),
                WaitStatus::Signaled(_, Signal::SIGXCPU, _) => Err(anyhow!("CC0 timed out")).context(output),
                status => Err(anyhow!("CC0 unexpectedly failed: {:?}", status)).context(output)
            }
        }
    }
}

pub fn execute<Executable: AsRef<CStr>>(info: &TestExecutionInfo, executable: Executable, timeout: u64, memory: u64) -> Result<(String, Behavior)> {
    execute_with_args::<Executable, &CStr>(info, executable, &[], timeout, memory)
}

pub fn execute_with_args<Executable: AsRef<CStr>, Arg: AsRef<CStr>>(
    info: &TestExecutionInfo, 
    executable: Executable, 
    args: &[Arg], 
    timeout: u64,
    memory: u64) -> Result<(String, Behavior)> 
{
    static mut test_counter: AtomicUsize = AtomicUsize::new(0);

    let result_file: String = unsafe {
        let current_dir = env::current_dir().unwrap();
        let next_id = test_counter.fetch_add(1, atomic::Ordering::Relaxed);
        format!("{}/c0_result{}", current_dir.display(), next_id)
    };

    let result_env = CString::new(format!("C0_RESULT_FILE={}", result_file)).unwrap();

    let mut argv = vec![executable.as_ref()];
    argv.extend(args.iter().map(|arg| arg.as_ref()));

    let (read_pipe, write_pipe) = unistd::pipe().context("When creating a pipe to record test output")?;

    match unsafe { unistd::fork().context("when spawning test process")? } {
        ForkResult::Child => {
            unistd::close(read_pipe).unwrap();
            redirect_output(write_pipe);
            set_resource_limits(memory, timeout);
            env::set_current_dir(Path::new(&*info.directory)).expect("Couldn't change to the test directory");

            let _ = unistd::execve(executable.as_ref(), &argv, &[&result_env]).unwrap_err();
            // Couldn't exec
            process::exit(EXEC_FAILURE_CODE);
        },

        ForkResult::Parent { child } => {
            let output = read_from_pipe(read_pipe, write_pipe)?;
            let status = wait::waitpid(child, None).expect("Failed to wait() for test program");

            // Read C0_RESULT_FILE, which consists of a null byte
            // followed by an i32 exit status, which is the 
            // return value from C0's main()
            let result = match fs::read(&result_file) {
                Ok(result) => {
                    fs::remove_file(Path::new(&result_file))
                        .context("when removing test program result file")?;

                    if result.len() == 5 {
                        let bytes = [result[1], result[2], result[3], result[4]];
                        Some(i32::from_ne_bytes(bytes))
                    }
                    else {
                        None
                    }
                }
                Err(_) => None
            };
            
            let behavior = match status {
                WaitStatus::Exited(_, 0) => 
                    if let Some(exit_code) = result {
                        Behavior::Return(Some(exit_code))
                    }
                    else {
                        return Err(anyhow!("C0 program exited succesfully, but no return value was written"))
                    },
                WaitStatus::Exited(_, 1) => Behavior::Failure,
                // Coin only. Hopefully other exit codes don't conflict
                WaitStatus::Exited(_, 2) => Behavior::CompileError,
                WaitStatus::Exited(_, 4) => Behavior::Failure,
                WaitStatus::Exited(_, EXEC_FAILURE_CODE) => return Err(anyhow!("Failed to exec the test program")).context(output),
                WaitStatus::Exited(_, RUST_PANIC_CODE) => return Err(anyhow!("Test program process panic'd")).context(output),
                WaitStatus::Exited(_, status) => return Err(anyhow!("Unexpected program exit status '{}'", status)).context(output),
                
                WaitStatus::Signaled(_, signal, _) => match signal {
                    Signal::SIGSEGV => Behavior::Segfault,
                    Signal::SIGXCPU => Behavior::InfiniteLoop,
                    Signal::SIGFPE => Behavior::DivZero,
                    Signal::SIGABRT => Behavior::Abort,
                    other => return Err(anyhow!("Program exited with unexpected signal '{}'", other)).context(output)
                }
                status => return Err(anyhow!("Program unexpectedly failed: {:?}", status)).context(output)
            };

            Ok((output, behavior))
        },
    }
}

/// Redirects stdout and stderr to the given file descriptor
fn redirect_output(target_file: RawFd) {
    unistd::dup2(target_file, STDOUT_FILENO).expect("Couldn't redirect stdout");
    unistd::dup2(target_file, STDERR_FILENO).expect("Couldn't redirect stderr");
}

/// Reads output from the given pipe set
fn read_from_pipe(read_pipe: RawFd, write_pipe: RawFd) -> Result<String> {
    // Capture CC0 output
    unistd::close(write_pipe).unwrap();
    
    const PIPE_CAPACITY: usize = 65536;
    let mut bytes: Vec<u8> = Vec::with_capacity(PIPE_CAPACITY);

    loop {
        #[allow(clippy::clippy::uninit_assumed_init)]
        let mut buf: [u8; PIPE_CAPACITY] = unsafe { MaybeUninit::uninit().assume_init() };
        let num_bytes = unistd::read(read_pipe, &mut buf).context("When reading CC0 output")?;
        if num_bytes == 0 {
            // read() returns 0 on EOF
            break;
        }

        bytes.extend(buf[..num_bytes].iter());
    }

    unistd::close(read_pipe).unwrap();
    let output = String::from_utf8_lossy(&bytes).to_string();
    Ok(output)
}

fn set_resource_limits(memory: u64, time: u64) {
    let mem_limit = libc::rlimit {
        rlim_cur: memory,
        rlim_max: memory
    };

    // Use a 'virtual timer' here, which only measures time actually spent
    // running our program in user mode. This means that if the OS
    // runs another program, the time spent doing that will not 
    // count against the timeout for the test

    let time_limit = libc::rlimit {
        rlim_cur: time,
        // If rlim_max == rlim_cur, then
        // the process gets SIGKILL.
        // Note that the Boehm GC used by C0RT 
        // sometimes uses SIGXCPU for its own purposes
        // if not configured with --disable-threads. 
        // However this might cause issues if we want to
        // do --enable-parallel-mark
        rlim_max: time.saturating_add(5)
    };

    unsafe {
        assert!(libc::setrlimit(libc::RLIMIT_AS, &mem_limit) >= 0);
        assert!(libc::setrlimit(libc::RLIMIT_CPU, &time_limit) >= 0);
    }
}

#[cfg(test)]
mod compile_tests {
    use super::*;
    use std::sync::Arc;

    const TEST_MEM: u64 = 4 * 1024 * 1024 * 1024;

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

        let args = [CString::new("test_resources/test.c0").unwrap()];
        compile(CString::new("bin/cc0")?, &args, 5, TEST_MEM)?.map_err(|e| anyhow!(e))?;
        assert_eq!(execute(&test.execution, &CString::new("a.out").unwrap(), 5, TEST_MEM)?.1, Behavior::Return(Some(0)));

        Ok(())
    }
}
