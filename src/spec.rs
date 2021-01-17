use std::fmt::{self, Formatter, Display};
use std::path::Path;
use std::sync::Arc;

/// Holds metadata about a test, as well as the parsed spec
#[derive(Debug)]
pub struct TestInfo {
    pub execution: TestExecutionInfo,
    pub specs: Specs
}

/// Test metadata
#[derive(Debug)]
pub struct TestExecutionInfo {
    /// Absolute paths to C0/C1 source files
    pub sources: Vec<String>,
    /// Any prescribed compiler options
    pub compiler_options: Vec<String>,
    /// The directory the test came from. Necessary since some
    /// test cases (e.g. <img> library tests) load resources
    pub directory: Arc<str>
}

/// Specs are of the form 'predicate => spec' or just a '<behavior>'
#[derive(Debug)]
pub enum Spec {
    Implication(ImplementationPredicate, Box<Spec>),
    Behavior(Behavior)
}

/// Test cases can have multiple specs i.e. if tests have one outcome in cc0
/// but another in coin
pub type Specs = Vec<Spec>;

/// Describes an implementation
#[derive(Debug)]
pub enum ImplementationPredicate {
    Library,
    Typechecked,
    GarbageCollected,
    Safe,
    False,
    ImplementationName(String),

    Not(Box<ImplementationPredicate>),
    And(Box<ImplementationPredicate>, Box<ImplementationPredicate>),
    Or(Box<ImplementationPredicate>, Box<ImplementationPredicate>)
}

/// An expected test behavior/test outcome.
/// Note that 'skipped' might be generated if the 
/// test was not actually run for some reason
/// (e.g. C1 tests in coin)
#[derive(Debug, Clone, Copy)]
pub enum Behavior {
    CompileError,
    Runs,
    InfiniteLoop,
    Abort,
    Failure,
    Segfault,
    DivZero,
    Return(Option<i32>),

    Skipped
}

impl PartialEq for Behavior {
    fn eq(&self, other: &Behavior) -> bool {
        use Behavior::*;
        match (self, other) {
            (CompileError, CompileError) => true,
            (Runs, Runs) => true,
            (InfiniteLoop, InfiniteLoop) => true,
            (Abort, Abort) => true,
            (Failure, Failure) => true,
            (Segfault, Segfault) => true,
            (DivZero, DivZero) => true,
            (Return(x), Return(y)) => 
                match (x, y) {
                    (None, _) => true,
                    (_, None) => true,
                    (Some(a), Some(b)) => a == b
                },
            (Skipped, _) => true,
            (_, Skipped) => true,
            _ => false
        }
    }
}

impl Eq for Behavior { }

// Display instances

impl Display for TestInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let sources: Vec<_> = self.execution.sources.iter().map(|source| {
            let mut path = Path::new(source);
            if let Some(prefix) = path.ancestors().nth(2) {
                path = path.strip_prefix(prefix).unwrap_or(path);
            }

            path.to_str().unwrap()
        }).collect();

        write!(f, "{}", sources.join(" "))?;
        for option in self.execution.compiler_options.iter() {
            write!(f, " {}", option)?;
        }
        write!(f, ": ")?;

        let mut first = true;
        for spec in self.specs.iter() {
            if !first {
                write!(f, "; ")?;
            }

            write!(f, "{}", spec)?;
            first = false;
        }

        Ok(())
    }    
}

impl Display for Spec {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use Spec::*;
        match self {
            Behavior(b) => write!(f, "{}", b),
            Implication(p, spec) => write!(f, "{} => {}", p, spec)
        }
    }
}

impl Display for ImplementationPredicate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use ImplementationPredicate::*;
        match self {
            Library => write!(f, "lib"),
            Typechecked => write!(f, "typecheck"),
            GarbageCollected => write!(f, "gc"),
            Safe => write!(f, "safe"),
            False => write!(f, "false"),
            ImplementationName(name) => write!(f, "{}", name),

            Not(p) => write!(f, "!{}", p),
            And(p1, p2) => write!(f, "{}, {}", p1, p2),
            Or(p1, p2) => write!(f, "{} or {}", p1, p2)
        }
    }
}

impl Display for Behavior {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use Behavior::*;
        match self {
            CompileError => write!(f, "error"),
            Runs => write!(f, "runs"),
            InfiniteLoop => write!(f, "infloop"),
            Abort => write!(f, "abort"),
            Failure => write!(f, "fail"),
            Segfault => write!(f, "segfault"),
            DivZero => write!(f, "div-by-zero"),
            Return(None) => write!(f, "return *"),
            Return(Some(x)) => write!(f, "return {}", x),
            
            Skipped => write!(f, "<skipped>")
        }
    }
}