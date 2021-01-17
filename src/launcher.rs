#![allow(non_upper_case_globals)]

use std::process;
use std::os::unix::io::RawFd;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{self, AtomicUsize};
use std::ffi::{CStr, CString};
use std::mem::MaybeUninit;
use lazy_static::lazy_static;
use nix::unistd::{self, ForkResult};
use nix::sys::wait::{self, WaitStatus};
use nix::sys::signal::Signal;
use nix::libc::{self, STDOUT_FILENO, STDERR_FILENO};
use anyhow::{anyhow, Context, Result};

use crate::spec::*;
use crate::executer::*;

pub struct CC0Executer();

impl Executer for CC0Executer {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)> {
        let mut args: Vec<CString> = Vec::new();
        args.extend(test.compiler_options.iter().map(string_to_cstring));
        args.extend(test.sources.iter().map(string_to_cstring));
        
        static mut test_counter: AtomicUsize = AtomicUsize::new(0);

        let out_file: CString = unsafe {
            let current_dir = env::current_dir().unwrap();
            let next_id = test_counter.fetch_add(1, atomic::Ordering::Relaxed);
            str_to_cstring(&format!("{}/a.out{}", current_dir.display(), next_id))
        };
        args.push(str_to_cstring("-vo"));
        args.push(out_file.clone());

        let compilation_result = compile(&args)?;
        match compilation_result {
            Ok(()) => {
                let exec_result = execute(test, &out_file, CC0_TEST_TIMEOUT);
                if let Err(e) = fs::remove_file(Path::new(&out_file.to_str().unwrap())) {
                    eprintln!("❗ Couldn't delete bc0 file: {:#}", e);
                }
                exec_result
            },
            Err(output) => Ok((output, Behavior::CompileError))
        }
    }

    fn properties(&self) -> ExecuterProperties {
        ExecuterProperties {
            libraries: true,
            garbage_collected: true,
            safe: true,
            typechecked: true,
            name: "cc0".to_string()
        }
    }
}

pub struct C0VMExecuter();

impl Executer for C0VMExecuter {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)> {
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
        match compilation_result {
            Ok(()) => {
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
            Err(output) => Ok((output, Behavior::CompileError))
        }
    }

    fn properties(&self) -> ExecuterProperties {
        ExecuterProperties {
            libraries: true,
            garbage_collected: false,
            safe: true,
            typechecked: true,
            name: "cc0_c0vm".to_string()
        }
    }
}

pub struct CoinExecuter();

impl Executer for CoinExecuter {
    fn run_test(&self, test: &TestExecutionInfo) -> Result<(String, Behavior)> {
        // Check if it uses C1, if so then skip the test
        for source in test.sources.iter() {
            if source.ends_with(".c1") {
                return Ok(("<C1 test skipped>".to_string(), Behavior::Skipped))
            }
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
            name: "coin".to_string()
        }
    }
}

lazy_static! {
    static ref C0_HOME: Option<String> = {
        env::var("C0_HOME").ok().map(|path| {
            let pathbuf = PathBuf::from(&path);
            fs::canonicalize(pathbuf).unwrap_or_else(|err| {
                eprintln!("Error: C0_HOME='{}': {:#}", path, err);
                process::exit(1)
            }).to_str().unwrap().to_string()        
        })
    };

    static ref CC0_PATH: CString = {
        let path = match C0_HOME.as_ref() {
            Some(path) => format!("{}/bin/cc0", path),
            None => "cc0".to_string()
        };

        CString::new(path).unwrap()
    };

    static ref C0VM_PATH: CString = {
        let path = match C0_HOME.as_ref() {
            Some(path) => format!("{}/vm/c0vm", path),
            None => "c0vm".to_string()
        };

        CString::new(path).unwrap()
    };

    static ref COIN_EXEC_PATH: CString = {
        let path = match C0_HOME.as_ref() {
            Some(path) => format!("{}/bin/coin-exec.bin", path),
            None => "coin-exec".to_string()
        };

        CString::new(path).unwrap()
    };    
}

/// Timeout for compilation
const COMPILATION_TIMEOUT: u32 = 10;
/// Timeout for running tests with the GCC backend
const CC0_TEST_TIMEOUT: i32 = 10;
/// Timeout for running tests in C0VM.
/// C0VM is probably more than 2x as slow as GCC,
/// but truly intensive tests should not be run in C0VM
const C0VM_TEST_TIMEOUT: i32 = 20;
/// Similar to C0VM, truly intensive tests should not be run in coin
const COIN_TEST_TIMEOUT: i32 = 20;

const COMPILATION_MAX_MEM: u64 = 4 * 1024 * 1024 * 1024;
const TEST_MAX_MEM: u64 = 4 * 1024 * 1024 * 1024;

const CC0_GCC_FAILURE_CODE: i32 = 2;
const EXEC_FAILURE_CODE: i32 = 100;
const RUST_PANIC_CODE: i32 = 101;

fn compile<Arg: AsRef<CStr>>(args: &[Arg]) -> Result<Result<(), String>> {
    // Create argv
    let mut argv = vec![CC0_PATH.as_ref()];
    argv.extend(args.iter().map(|arg| arg.as_ref()));

    // Create a pipe to record stdout and stuff
    let (read_pipe, write_pipe) = unistd::pipe().context("When creating a pipe to record CC0 output")?;

    match unsafe { unistd::fork().context("when spawning CC0")? } {
        ForkResult::Child => {
            unistd::close(read_pipe).unwrap();
            redirect_io(write_pipe);
            set_resource_limits(COMPILATION_MAX_MEM, COMPILATION_TIMEOUT as u64);

            let _ = unistd::execvp(CC0_PATH.as_ref(), &argv);
            unsafe { libc::_exit(EXEC_FAILURE_CODE); }
        },

        ForkResult::Parent { child } => {
            let output = read_from_pipe(read_pipe, write_pipe).unwrap_or("<couldn't read output>".to_string());
            let status = wait::waitpid(child, None).expect("Failed to wait() for compiler process");
            
            match status {
                WaitStatus::Exited(_, 0) => Ok(Ok(())),
                WaitStatus::Exited(_, 1) => Ok(Err(output)),
                WaitStatus::Exited(_, CC0_GCC_FAILURE_CODE) => {
                    Err(anyhow!("CC0 failed to invoke GCC"))
                },
                WaitStatus::Exited(_, EXEC_FAILURE_CODE) => Err(anyhow!("Failed to exec cc0")).context(output),
                WaitStatus::Exited(_, RUST_PANIC_CODE) => Err(anyhow!("CC0 process panic'd")).context(output),
                WaitStatus::Signaled(_, Signal::SIGKILL, _) => Err(anyhow!("CC0 timed out")).context(output),
                status => Err(anyhow!("CC0 unexpectedly failed: {:?}", status)).context(output)
            }
        }
    }
}

fn execute<Executable: AsRef<CStr>>(info: &TestExecutionInfo, executable: Executable, timeout: i32) -> Result<(String, Behavior)> {
    execute_with_args::<Executable, &CStr>(info, executable, &[], timeout)
}

fn execute_with_args<Executable: AsRef<CStr>, Arg: AsRef<CStr>>(
    info: &TestExecutionInfo, 
    executable: Executable, 
    args: &[Arg], 
    timeout: i32) -> Result<(String, Behavior)> 
{
    let result_file = format!("{}/c0_result{}", env::current_dir().unwrap().display(), unistd::gettid());
    let result_env = string_to_cstring(&format!("C0_RESULT_FILE={}", result_file));

    let mut argv = vec![executable.as_ref()];
    argv.extend(args.iter().map(|arg| arg.as_ref()));

    let (read_pipe, write_pipe) = unistd::pipe().context("When creating a pipe to record test output")?;

    match unsafe { unistd::fork().context("when spawning test process")? } {
        ForkResult::Child => {
            unistd::close(read_pipe).unwrap();
            redirect_io(write_pipe);

            set_resource_limits(TEST_MAX_MEM, timeout as u64);
            env::set_current_dir(Path::new(&*info.directory)).expect("Couldn't change to the test directory");

            // Use a 'virtual timer' here, which only measures time actually spent
            // running our program in user mode. This means that if the OS
            // runs another program, the time spent doing that will not 
            // count against the timeout for the test
            // set_virtual_timer(timeout as i64);

            let _err = unistd::execve(executable.as_ref(), &argv, &[&result_env]).unwrap_err();
            // Couldn't exec
            process::exit(EXEC_FAILURE_CODE);
        },

        ForkResult::Parent { child } => {
            let output = read_from_pipe(read_pipe, write_pipe)?;
            let status = wait::waitpid(child, None).expect("Failed to wait() for test program");
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
                    // Some Linux versions send SIGKILL when the 
                    // time rlimit is exceeded
                    | Signal::SIGXCPU
                    | Signal::SIGKILL => Behavior::InfiniteLoop,
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

fn redirect_io(target_file: RawFd) {
    unistd::dup2(target_file, STDOUT_FILENO).expect("Couldn't redirect stdout");
    unistd::dup2(target_file, STDERR_FILENO).expect("Couldn't redirect stderr");
}

fn read_from_pipe(read_pipe: RawFd, write_pipe: RawFd) -> Result<String> {
    const PIPE_CAPACITY: usize = 65536;
    
    // Capture CC0 output
    unistd::close(write_pipe).unwrap();

    let mut bytes: Vec<u8> = Vec::with_capacity(PIPE_CAPACITY);

    loop {
        let mut buf: [u8; PIPE_CAPACITY] = unsafe { MaybeUninit::uninit().assume_init() };
        let num_bytes = unistd::read(read_pipe, &mut buf).context("When reading CC0 output")?;
        if num_bytes == 0 {
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

    let time_limit = libc::rlimit {
        rlim_cur: time,
        rlim_max: time
    };

    unsafe {
        assert!(libc::setrlimit(libc::RLIMIT_AS, &mem_limit) >= 0);
        assert!(libc::setrlimit(libc::RLIMIT_CPU, &time_limit) >= 0);
    }

    println!("Set resource limits");
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

        let args= [CString::new("test_resources/test.c0").unwrap()];
        compile(&args)?.map_err(|e| anyhow!(e))?;
        assert_eq!(execute(&test.execution, &CString::new("a.out").unwrap(), 5)?.1, Behavior::Return(Some(0)));

        Ok(())
    }
}

fn str_to_cstring(s: &str) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}

fn string_to_cstring(s: &String) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}
