# c0check - CC0 Testing Harness

This is a re-implementation of cc0-check in Rust. 

The key enhancement is that the test cases are run in parallel, and output from failed tests
is saved. On my i7-8700k, this runs several times faster than the 
SML-based `*-check` (~1 minute vs ~10 for CC0, ~6 minutes vs ~20 for C0VM)

Other enhancements include better timeouts which measure user time spent
in the test process as opposed to wall clock time. This means that if the
system context-switches out the test programs for a long time, they will
not incorrectly time out. This is useful on Andrew servers

There is also a bound placed on the memory usage of the test program.
This prevents coin/c0vm testing from eating all available memory in
some infinite loop tests since they don't have a garbage collector and
making the system unusable for other purposes.  

## Requirements

This program should work on Linux and MacOS. Note that since this program
spawns a lot of processes, it might not work well on the Andrew Linux servers.
You can set `RAYON_NUM_THREADS` to something low to limit the resource 
usage of this program.

## Usage

The program can be built using `cargo build` and run with `cargo run`.
The `--release` flag can be added in order to optimize (the difference can be substantial).

You should also set the `C0_HOME` environment variable, or the program
will use the `cc0`/`c0vm`/`coin-exec` on your `$PATH`, which might be
incorrect for development usage

```
$ cargo run -- <test program=cc0|c0vm|coin> <path to test folder>
# For example, 
$ C0_HOME=~/c0-developer/cc0 cargo run -- cc0 ~/c0-developer/cc0/tests/
 246/3742 ✅ Test passed: l5tests1-f12/thorin-opt-0.c0: return 225520
 247/3742 ✅ Test passed: l3tests0/exception03.c0: infloop
 248/3742 ✅ Test passed: ibhargav-voidptr-lval-casts/invalid-lval-cast.c1: error
 249/3742 ✅ Test passed: l5tests1-f12/isildur-likes-useless-code.c0: return 0
 250/3742 ✅ Test passed: l2tests1/ankylosaurus-return01.c0: return 3
 251/3742 ✅ Test passed: l4tests1-f11/harrier-exception_2.c0: segfault
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

There is a race condition when running CC0 in parallel: when compiling
a sequence of files (e.g. `foo.c0 bar.c0 haz.c0`), CC0 generates a temporary file
with the name taken from the last file in the sequence (e.g. `haz.c0.c`).
Unfortunately if multiple tests in a folder contain the same file as the last 
file, then there is the following race condition:

| CC0 #1                  | CC0 #2 |
| ----------------------- | --------------- |
| Generates `haz.c0.c`      |                    |
|                         | Generates `haz.c0.c` |
| Invokes `gcc` on `haz.c0.c` |               |
| Deletes `haz.c0.c`        |                 |
|                           | Invokes `gcc` on `haz.c0.c` |
|                         | Error: `haz.c0.c` doesn't exist |
| Error: executable produces wrong result | |
