# c0check - CC0 Testing Harness

This is a re-implementation of cc0-check in Rust. The key enhancement
is that the test cases are run in parallel, and output from failed tests
is saved. On my i7-8700k, this runs several times faster than the 
SML-based `*-check` (~1 minute vs ~10 for CC0, ~6 minutes vs ~20 for C0VM).

## Requirements

This program uses `gettid()` to generate unique per-thread IDs and `pipe2`, 
so it only works on Linux. These two could be easily changed by using
an atomic usize for `gettid` and by using `fcntl` to set `O_NONBLOCK` 
on the pipes used to capture `stdout` and `stderr`.

## Usage

The program can be built using `cargo build` and run with `cargo run`.
The `--release` flag can be added in order to optimize (the difference can be substantial).

You should also set the `C0_HOME` environment variable, or the program
will use the `cc0`/`c0vm`/`coin-exec` on your `$PATH`

```
$ cargo run -- <test program=cc0|c0vm|coin> <path to test folder>
# For example, 
$ C0_HOME=~/c0-developer/cc0 cargo run -- cc0 ~/c0-developer/cc0/tests/
  246/ 3742 ✅ Test passed: l5tests1-f12/thorin-opt-0.c0: return 225520
  246/ 3742 ✅ Test passed: l3tests0/exception03.c0: infloop
  246/ 3742 ✅ Test passed: ibhargav-voidptr-lval-casts/invalid-lval-cast.c1: error
  246/ 3742 ✅ Test passed: l5tests1-f12/isildur-likes-useless-code.c0: return 0
  246/ 3742 ✅ Test passed: l2tests1/ankylosaurus-return01.c0: return 3
  246/ 3742 ✅ Test passed: l4tests1-f11/harrier-exception_2.c0: segfault
...

Failed tests:

Errors:

⛔ l5tests1/brachiosaurus-full-of-hot-air.c0: !cc0_c0vm => return 999
CC0 timed out

Test summary:
✅ Passed: 3741
❌ Failed: 0
⌛ Timeouts: 0
⛔ Error: 1
```

After all tests finish, a summary will be displayed, containing
an explanation of which tests failed, and which tests encountered an error.
If a test failed, its output will also be included.

## Known Issues

The program will generate `a.out123` and `c0_result123` files during execution.
If you halt the program with CTRL-C in the middle of testing, then these files
might stick around. You would have to delete them manually.

It seems that CC0 (incorrectly and nondeterministically) fails to compile 
some tests unexpectedly, when the compiler is run in parallel. Haven't seen
this happen recently so it might be fixed.
