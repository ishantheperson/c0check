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
spawns a lot of processes, it might get throttled on the Andrew servers.
You can set the environment variable `RAYON_NUM_THREADS` to something low 
to limit the resource usage of this program.

## Usage

The program can be installed by cloning the repository and running
`cargo install --path .`. This will install the binary to `~/.cargo/bin`.
Alternatively, you could use `cargo run --release -- <args>` if you don't
want to add it to your path.

```
c0check 1.0.0

USAGE:
    c0check [OPTIONS] <executer> <test-dir> --c0-home <c0-home>

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


OPTIONS:
        --c0-home <c0-home>
            Path to CC0 directory.

            Should have bin/cc0, bin/coin-exec, and vm/c0vm. Will default to
            $C0_HOME if not provided
    -t, --test-time <test-time>
            Timeout in seconds for running each test

            This is real CPU time, not 'wall-clock' time, since it is enforced
            using setrlimit() [default: 10]
    -m, --test-memory <test-memory>
            Max amount of memory a test can use.

            Should be of the form <n> <unit> where unit is gb, mb, kb, or
            optionally blank to indicate 'n' is bytes [default: 2 GB]
        --compilation-time <compilation-time>
            Timeout in seconds for compilation via CC0

            Includes time spent in GCC [default: 20]
        --compilation-mem <compilation-mem>
            Maximum amount of memory CC0/GCC can use [default: 4 GB]


ARGS:
    <executer>
            Which implementation to test

            'cc0' tests the GCC backend. 'c0vm' tests the bytecode compiler and
            vm implementation. 'coin' tests the interpreter [possible values:
            CC0, C0VM, Coin]
    <test-dir>
            Path to the top-level test directory.

            The directory should contain subdirectories which should either
            contain test cases or a sources.test file
```

Example:
```
$ c0check cc0 ~/c0-developer/cc0/tests/ --c0-home ~/c0-developer/cc0/
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
<output elided>
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

This will be fixed by the Fall 2021 release of CC0
