# c0check - CC0 Testing Harness

This is a re-implementation of cc0-check in Rust. The key enhancement
is that the test cases are run in parallel. On my i7-8700k, this runs
10 times faster than the SML based `cc0-check`. 

## Requirements

This program uses `gettid()` to generate unique per-thread IDs, 
so it only works on Linux. This could change once the Rust `ThreadId::as_u64()`
is stabilized (I didn't use nightly Rust, but that might change depending
on how much of a burden it is). 


## Usage

The program can be built using `cargo build` and run with `cargo run`.
The `--release` flag can be added in order to optimize (the difference can be substantial).

You should also set the `C0_HOME` environment variable, or the program
will use the `cc0` on your `$PATH`

```
$ cargo run -- <path to test folder>
# For example, 
$ C0_HOME=~/c0-developer/cc0 cargo run -- ~/c0-developer/cc0/tests/
Running tests: [00:00:22 elapsed] ###########>----------------------------  1030/3736   [00:01:00 remaining]

...

Failed tests:

Errors:

⛔ l5tests1/brachiosaurus-full-of-hot-air.c0: !cc0_c0vm => return 999
CC0 timed out

Test summary:
✅ Passed: 3735
❌ Failed: 0
⛔ Error: 1
```

This will show a progress bar with an ETA to completion. 
After all tests finish, a summary will be displayed, containing
an explanation of which tests failed, and which tests encountered an error.

## Known Issues

The program will generate `a.out123` and `c0_result123` files during execution.
If you halt the program with CTRL-C in the middle of testing, then these files
might stick around. You would have to delete them manually.

It seems that CC0 (incorrectly and nondeterministically) fails to compile 
some tests unexpectedly, when the compiler is run in parallel.
Could also possibily be a WSL issue. 
It's hard to reproduce and doesn't seem to happen without --release.
