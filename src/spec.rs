use std::fmt::{self, Formatter, Display};
use std::path::Path;

#[derive(Debug)]
pub struct TestInfo {
    pub execution: TestExecutionInfo,
    pub specs: Specs
}

#[derive(Debug)]
pub struct TestExecutionInfo {
    pub sources: Vec<String>,
    pub compiler_options: Vec<String>
}

#[derive(Debug)]
pub enum Spec {
    Implication(ImplementationPredicate, Box<Spec>),
    Behavior(Behavior)
}

pub type Specs = Vec<Spec>;

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

#[derive(Debug)]
pub enum Behavior {
    CompileError,
    Runs,
    InfiniteLoop,
    Abort,
    Failure,
    Segfault,
    DivZero,
    Return(Option<i32>)
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
        write!(f, "{}", self.execution.compiler_options.join(" "))?;
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
            Return(Some(x)) => write!(f, "return {}", x)
        }
    }
}