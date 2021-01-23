use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use std::sync::atomic::{self, AtomicUsize};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use anyhow::{Result, Context, anyhow};
 
use crate::spec::*;
use crate::executer::{Executer, ExecuterProperties};
use crate::launcher::*;
use crate::options::*;

pub struct CC0Executer {
    cc0_path: CString,

    cc0_memory: u64,
    cc0_time: u64,

    test_memory: u64,
    test_time: u64
}

impl CC0Executer {
    pub fn new(options: &Options) -> Result<CC0Executer> {
        let cc0_path = make_cstr_path(options.c0_home.clone(), &["bin", "cc0"])?;

        Ok(CC0Executer {
            cc0_path,

            cc0_memory: options.compilation_mem,
            cc0_time: options.compilation_time,

            test_memory: options.test_memory,
            test_time: options.test_time
        })
    }
}

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

        let compilation_result = compile(&self.cc0_path, &args, self.cc0_memory, self.cc0_time)?;
        if let Err(output) = compilation_result {
            return Ok((output, Behavior::CompileError))
        }
        
        let exec_result = execute(test, &out_file, self.test_time, self.test_memory);
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

pub struct C0VMExecuter {
    cc0_path: CString,

    cc0_memory: u64,
    cc0_time: u64,

    c0vm_path: CString,

    test_memory: u64,
    test_time: u64
}

impl C0VMExecuter {
    pub fn new(options: &Options) -> Result<C0VMExecuter> {
        let cc0_path = make_cstr_path(options.c0_home.clone(), &["bin", "cc0"])?;
        let c0vm_path = make_cstr_path(options.c0_home.clone(), &["vm", "c0vm"])?;

        Ok(C0VMExecuter {
            cc0_path,

            cc0_memory: options.compilation_mem,
            cc0_time: options.compilation_time,

            c0vm_path,

            test_memory: options.test_memory,
            test_time: options.test_time
        })
    }    
}

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

        let compilation_result = 
            compile(
                &self.cc0_path, 
                &args,
                self.cc0_time,
                self.cc0_memory)?;
        
        if let Err(output) = compilation_result {
            return Ok((output, Behavior::CompileError))
        }

        // Run test case
        let exec_result = 
            execute_with_args(
                test, 
                &self.c0vm_path, 
                &[out_file.as_ref()], 
                self.test_time, 
                self.test_memory);
        
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

pub struct CoinExecuter {
    coin_path: CString,

    test_time: u64,
    test_memory: u64
}

impl CoinExecuter {
    pub fn new(options: &Options) -> Result<CoinExecuter> {
        let coin_path = make_cstr_path(options.c0_home.clone(), &["bin", "coin-exec"])?;
        
        Ok(CoinExecuter {
            coin_path,

            test_time: options.test_time,
            test_memory: options.test_memory
        })
    }
}

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

        execute_with_args(test, &self.coin_path, &args, self.test_time, self.test_memory)
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

fn make_cstr_path(mut base: PathBuf, path: &[&str]) -> Result<CString> {
    base.extend(["bin", "cc0"].iter());

    if !base.is_file() {
        return Err(anyhow!("'{:?}' is not a file", path))
    }

    Ok(CString::new(base.as_os_str().as_bytes()).unwrap())
}

fn str_to_cstring(s: &str) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}

fn string_to_cstring(s: &String) -> CString {
    CString::new(s.as_bytes()).unwrap()        
}
