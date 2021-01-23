use std::path::PathBuf;
use structopt::clap::{AppSettings, arg_enum};
use anyhow::{anyhow, Result, Context};

pub use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(setting(AppSettings::ColoredHelp))]
#[structopt(setting(AppSettings::DeriveDisplayOrder))]
#[structopt(set_term_width(80))]
pub struct Options {
    /// Which implementation to test
    ///
    /// 'cc0' tests the GCC backend.
    /// 'c0vm' tests the bytecode compiler and vm implementation.
    /// 'coin' tests the interpreter
    #[structopt(
        possible_values = &ExecuterKind::variants(),
        case_insensitive = true
    )]
    pub executer: ExecuterKind,

    /// Path to the top-level test directory.
    ///
    /// The directory should contain subdirectories which 
    /// should either contain test cases or a sources.test file
    #[structopt(parse(from_os_str))]
    pub test_dir: PathBuf,

    /// Path to CC0 directory.
    ///
    /// Should have bin/cc0, bin/coin-exec, and vm/c0vm.
    /// Will default to $C0_HOME if not provided
    #[structopt(
        long, 
        parse(from_os_str),
        env = "C0_HOME")]
    pub c0_home: PathBuf,

    /// Timeout in seconds for running each test
    ///
    /// This is real CPU time, not 'wall-clock' time, since it is 
    /// enforced using setrlimit()
    #[structopt(short = "t", long, default_value = "10")]
    pub test_time: u64,

    /// Max amount of memory a test can use. 
    ///
    /// Should be of the form <n> <unit>
    /// where unit is gb, mb, kb, or optionally blank to indicate 'n' is bytes
    #[structopt(
        short = "m", 
        long,
        parse(try_from_str = parse_size), 
        default_value = "2 GB")]
    pub test_memory: u64,

    /// Timeout in seconds for compilation via CC0
    ///
    /// Includes time spent in GCC
    #[structopt(long, default_value = "20")]
    pub compilation_time: u64,

    /// Maximum amount of memory CC0/GCC can use.
    #[structopt(
        long, 
        parse(try_from_str = parse_size),
        default_value = "4 GB")]
    pub compilation_mem: u64
}

arg_enum! {
    pub enum ExecuterKind {
        CC0,
        C0VM,
        Coin
    }
}

fn parse_size(size: &str) -> Result<u64> {
    let size = size.trim();

    let suffix_pos = match size.rfind(|c: char| c.is_ascii_digit()) {
        Some(pos) => pos + 1,
        None => return size.parse().context(format!("Invalid size '{}'", size))
    };
    let (n, unit) = size.split_at(suffix_pos);

    if n.is_empty() {
        return Err(anyhow!("No number found in '{}'", size))
    }

    let n = n.parse::<f64>()?;
    let bytes = match unit.trim().to_ascii_lowercase().as_str() {
        "g" | "gb" => n * 1024. * 1024. * 1024.,
        "m" | "mb" => n * 1024. * 1024.,
        "k" | "kb" => n * 1024.,
        "" => n,
        _ => return Err(anyhow!("Invalid size unit '{}'", unit))
    };

    Ok(bytes as u64)
}

#[cfg(test)]
mod options_tests {
    use super::*;

    #[test]
    fn test_parse_size() -> Result<()> {
        macro_rules! tests {
            ($($x: expr => $y: expr),*) => {
                $(
                    assert_eq!(parse_size($x)?, $y);
                )*
            }
        }

        tests!(
            "2GB" => 2 * 1024 * 1024 * 1024,
            "2 gb" => 2 * 1024 * 1024 * 1024,
            "0 mb" => 0,
            "  10 mb" => 10 * 1024 * 1024,
            "512  " => 512
        );

        Ok(())
    }

    #[test]
    fn test_parse_size_errors() {
        macro_rules! tests {
            ($($x: expr),*) => {
                $(
                    assert!(parse_size($x).is_err());
                )*
            }
        }

        tests!(
            "",
            "   mb",
            "   zmb",
            "-1",
            "-9999999999999999999999999999999999999999999999999999999999",
            "9999999999999999999999999999999999999999999999999999999999999"
        );
    }
}