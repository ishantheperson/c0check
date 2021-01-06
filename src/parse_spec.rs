use thiserror::Error;
use logos::{Lexer, Logos, Span};

use crate::spec::*;

/// Parses a 'spec' string with the given options
///
/// spec ::= <predicate> => <spec>
///        | <behavior>
///
/// predicate ::= lib | typechecked | gc | safe | false | <ident>
///             | ! <predicate>
///             | <predicate>, <predicate>
///             | <predicate> or <predicate> 
///
/// behavior ::= error | infloop | abort | failure | segfault | div-by-zero
///            | runs | return * | return <int>
pub fn parse(input: &str, options: ParseOptions) -> Result<Specs, SpecParseError> {
    let mut parser = SpecParser::new(input, options);
    parser.parse()
}

pub struct ParseOptions {
    /// Whether '//test' should appear at the start of the spec string
    pub require_test_marker: bool
}

struct SpecParser<'a> {
    input: &'a str,
    lexer: SpecLexer,

    options: ParseOptions
}

impl<'a> SpecParser<'a> {
    fn new(input: &'a str, options: ParseOptions) -> SpecParser<'a> {
        SpecParser { input, lexer: SpecLexer::new(input), options }
    }

    fn parse(&mut self) -> Result<Specs, SpecParseError> {
        use SpecParseError::*;
        use SpecToken::*;
    
        self.lexer = SpecLexer::new(self.input);
    
        // Make sure it starts with //test if required
        if self.options.require_test_marker {
            if !matches!(self.lexer.next(), Some((TestStartMarker, _))) {
                return Err(NotSpec)
            }
        }
    
        let mut tests: Specs = Vec::new();
    
        loop {
            let spec = self.parse_spec()?;
            tests.push(spec);

            match self.lexer.next() {
                Some((Semicolon, _)) => (),
                None => break,
                Some((_, range)) => return Err(self.unexpected_token(range, "semicolon to separate tests"))
            }
        };
    
        Ok(tests)
    }
    
    // Pratt parser based on matklad's blog post
    // https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html

    /// Parses a spec
    /// 
    /// spec ::= <predicate> => <spec>
    ///        | <behavior>
    fn parse_spec(&mut self) -> Result<Spec, SpecParseError> {
        use SpecParseError::*;
        use SpecToken::*;

        let (tok, _) = match self.lexer.peek() {
            Some(tok) => tok,
            None => return Err(UnexpectedEOF { msg: "implementation or behavior"} )
        };

        if tok.is_behavior() {
            let behavior = self.parse_behavior()?;
            Ok(Spec::Behavior(behavior))
        }
        else {
            let implementation = self.parse_implementation(0)?;

            // After a predicate we always expect => 
            match self.lexer.next() {
                Some((FatArrow, _)) => (),
                Some((_, range)) => 
                    return Err(self.unexpected_token(range, "'=>' between implementation and behavior")),
                None => 
                    return Err(UnexpectedEOF { msg: "'=>' between implementation and behavior" }),
            }

            // Could be a loop too
            let consequent = self.parse_spec()?;
            Ok(Spec::Implication(implementation, Box::new(consequent)))
        }
    }

    /// Parses an implementation predicate
    /// 
    /// predicate ::= lib | typechecked | gc | safe | false | <ident>
    ///             | ! <predicate>
    ///             | <predicate>, <predicate>
    ///             | <predicate> or <predicate> 
    fn parse_implementation(&mut self, min_bp: i32) -> Result<ImplementationPredicate, SpecParseError> {
        use SpecParseError::*;
        use ImplementationPredicate::*;

        fn infix_binding_power(tok: &SpecToken) -> Option<(i32, i32)> {
            match tok {
                SpecToken::Or => Some((1, 2)),
                SpecToken::Comma => Some((3, 4)),
                _ => None
            }
        }

        fn prefix_binding_power(tok: &SpecToken) -> Option<((), i32)> {
            match tok {
                SpecToken::Not => Some(((), 5)),
                _ => None
            }
        }

        let mut lhs = match self.lexer.next() {
            None => return Err(UnexpectedEOF { msg: "implementation predicate" }),
            Some((tok, range)) => {
                match tok {
                    // At the beginning of a predicate,
                    // we expect an atom or a prefix operator
                    SpecToken::Lib => Library,
                    SpecToken::Typechecked => Typechecked,
                    SpecToken::GarbageCollected => GarbageCollected,
                    SpecToken::Safe => Safe,
                    SpecToken::False => False,
                    SpecToken::Implementation(name) => ImplementationName(name),

                    // Not an atom, try to parse it as a prefix operator
                    tok => {
                        let ((), rhs_bp) = match prefix_binding_power(&tok) {
                            Some(result) => result,
                            None => return Err(self.unexpected_token(range, "implementation predicate type or prefix operator"))
                        };

                        let operand = self.parse_implementation(rhs_bp)?;
                        Not(Box::new(operand))
                    }
                }
            }
        };

        loop {
            // No postfix operators so 'peek' technically could be 'next'
            let (left_bp, right_bp) = match self.lexer.peek() {
                None => break,
                Some((tok, _)) => 
                    match infix_binding_power(&tok) {
                        Some(bps) => bps, 
                        None => break
                    }
            };

            if left_bp < min_bp {
                break;
            }

            let (tok, _) = self.lexer.next().unwrap();
            let rhs = self.parse_implementation(right_bp)?;

            lhs = match tok {
                SpecToken::Comma => And(Box::new(lhs), Box::new(rhs)),
                SpecToken::Or => Or(Box::new(lhs), Box::new(rhs)),

                _ => unreachable!()
            }
        }

        Ok(lhs)
    }

    /// Parses a program expected behavior
    /// 
    /// behavior ::= error | infloop | abort | failure | segfault | div-by-zero
    ///            | runs | return * | return <int>
    fn parse_behavior(&mut self) -> Result<Behavior, SpecParseError> {
        use SpecParseError::*;
        use Behavior::*;
    
        match self.lexer.next() {
            None => Err(UnexpectedEOF { msg: "behavior" }),
            Some((tok, range)) => 
                match tok {
                    SpecToken::CompileError => Ok(CompileError),
                    SpecToken::Runs => Ok(Runs),
                    SpecToken::InfiniteLoop => Ok(InfiniteLoop),
                    SpecToken::Abort => Ok(Abort),
                    SpecToken::Failure => Ok(Failure),
                    SpecToken::Segfault => Ok(Segfault),
                    SpecToken::DivZero => Ok(DivZero),
                    SpecToken::Return(x) => Ok(Return(x)),
    
                    _ => Err(self.unexpected_token(range, "behavior"))
                }
        }
    }

    fn unexpected_token(&mut self, range: Span, msg: &'static str) -> SpecParseError {
        SpecParseError::UnexpectedToken { 
            actual: String::from(&self.input[range.clone()]), 
            range,
            msg: msg 
        }        
    }
}


#[derive(Debug, Error)]
pub enum SpecParseError {
    #[error("Did not start with //test")]
    NotSpec,

    #[error("unexpected '{actual:?}' at {range:?}, expected {msg}")]
    UnexpectedToken { actual: String, range: Span, msg: &'static str },
    
    #[error("unexpected end of input, expected {msg}")]
    UnexpectedEOF { msg: &'static str }
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    fn parse_test(input: &str, valid: bool) {
        let result = parse(input, ParseOptions { require_test_marker: true });

        println!("{}: {:?}", input, result);
        if result.is_ok() != valid {
            panic!("{} {}", input, if result.is_ok() { "parsed" } else { "did not parse"})
        }
    }

    #[test]
    fn test() {
        parse_test("//test return 5", true);
        parse_test("//test safe => return 5", true);
        parse_test("//test safe, typecheck => return 5", true);
        parse_test("//test cc0 or coin => return 5", true);

        parse_test("//test safe => segfault; !safe => runs", true);
        parse_test("//test safe => !cc0_c0vm => div-by-zero", true)
    }
}

#[derive(Logos, Debug, PartialEq, Eq, Clone)]
enum SpecToken {
    #[token("//test")]
    TestStartMarker,

    #[token("error")]
    CompileError,
    #[token("runs")]
    Runs,
    #[token("infloop")]
    InfiniteLoop,
    #[token("abort")]
    Abort,
    #[token("failure")]
    Failure,
    #[token("segfault")]
    Segfault,
    #[token("div-by-zero")]
    DivZero,
    #[token("return", lex_return)]
    Return(Option<i32>),

    // Only used to help lex return
    #[token("*")]
    Star,
    #[regex("[+-]?(0|[1-9][0-9]*)", |lex| i32::from_str_radix(lex.slice(), 10).ok())]
    #[regex("0[xX][0-9a-fA-F]+", |lex| i32::from_str_radix(&lex.slice()[2..], 16).ok())]
    Number(i32),

    #[token("lib")]
    Lib,
    #[token("typecheck")]
    Typechecked,
    #[token("gc")]
    GarbageCollected,
    #[token("safe")]
    Safe,
    #[token("false")]
    False,

    #[regex(r"[a-zA-Z_][-a-zA-Z0-9_]*", |lex| String::from(lex.slice()))]
    Implementation(String),

    #[token("!")]
    Not,
    #[token(",")]
    Comma,
    #[token("or")]
    Or,
    #[token(";")]
    Semicolon,
    #[token("=>")]
    FatArrow,

    #[error]
    #[regex(r"[ \t]+", logos::skip)]
    Error
}

impl SpecToken {
    fn is_behavior(&self) -> bool {
        use SpecToken::*;

        matches!(self,
              CompileError 
            | Runs
            | InfiniteLoop 
            | Segfault 
            | Abort
            | Failure
            | DivZero
            | Return(_)
        )
    }
}

fn lex_return(lexer: &mut Lexer<SpecToken>) -> Option<Option<i32>> {
    match lexer.next() {
        Some(SpecToken::Number(x)) => Some(Some(x)),
        Some(SpecToken::Star) => Some(None),
        _ => None
    }
}

struct SpecLexer {
    tokens: Vec<(SpecToken, Span)>
}

impl SpecLexer {
    fn new(input: &str) -> SpecLexer {
        let lexer = SpecToken::lexer(input).spanned();
        let mut tokens: Vec<_> = lexer.collect();
        tokens.reverse();

        SpecLexer { tokens }
    }

    fn next(&mut self) -> Option<(SpecToken, Span)> {
        self.tokens.pop()
    }

    fn peek(&self) -> Option<(SpecToken, Span)> {
        self.tokens.last().cloned()
    }
}

#[cfg(test)]
mod lexer_tests {
    use super::*;
    use SpecToken::*;

    fn lex_test(input: &str, expected: &[SpecToken]) {
        let lexer = SpecLexer::new(input);
        let tokens = lexer.tokens.iter().rev().map(|(tok, _)| tok.clone()).collect::<Vec<_>>();
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test() {
        lex_test("safe => return 5", &[Safe, FatArrow, Return(Some(5))]);
        lex_test("safe => return *", &[Safe, FatArrow, Return(None)]);
        lex_test("safe => return *", &[Safe, FatArrow, Return(None)]);
    }
}
