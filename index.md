# Cpplumber

Cpplumber is a static analysis tool based on clang that helps detecting and
keeping track of C and C++ source code information that leaks into compiled
executable files.

This tool is aimed at people developing software that may contain sensitive
information in some debug or private configurations and want to make sure it
doesn't go out accidentally in release builds or for people that are just looking
to make it so that reverse engineers don't have it too easy on their software.

The command line interface is inspired from the one of [Cppcheck](https://github.com/danmar/cppcheck/)
to make it easier to get familiar with.

# Installation

To function, Cpplumber requires `libclang` >=10.0.0 to be properly installed on
the system.

## Linux

On Linux distributions, you can install `clang` using your package manager.
For example, on Ubuntu with `apt`:
```sh
sudo apt install clang
```

## Windows

On Windows, simply install LLVM using a pre-built installer (*i.e*,
`LLVM-*-win64.exe`) from the [LLVM project's release page](https://github.com/llvm/llvm-project/releases)
and make sure that `libclang.dll` is accessible from the `PATH` environment
variable.

# Getting started

## First test

Here is some simple C code:
```c
#include <stdio.h>

int main()
{
    printf("Magic number: %d\n", 1337);
    return 0;
}
```

If you save that into `file1.c` and compile it into an executable:
```sh
clang file1.c
```

And then execute:
```sh
# Note: Might be `a.exe` on Windows
cpplumber --bin a.out file1.c
```

The output from Cpplumber should be something like that:
```
"Magic number: %d\n" (string literal) leaked at offset 0x14f20 in "/full/path/to/a.out" [declared at /full/path/to/file1.c:5]
```

It basically tells you that a string literal declared in `file1.c` at line 5, has
been found in the executable file `a.out` at offset 0x14f20.

## Checking all files in a folder

On all platform, you can use glob expressions to include all files in a folder:

```sh
cpplumber --bin a.out "src/*"
```

Note: quotes are important if you're using a terminal that handles glob
expressions itself.

## Checking files matching a given file filter

You can also do the same as above, but to include files with more specific
filters:

```sh
cpplumber --bin a.out "src/**/*.cc" "src/**/*.h"
```

## Ignoring certain leak types

Cpplumber can currently report leaks for string literals and class/struct names.
However, it's possible to ignore a certain type of leaks with a command-line
argument:

```sh
# Ignore leaks of string literals
cpplumber --bin a.out --ignore-string-literals "src/*"
# Ignore leaks of class and struct names
cpplumber --bin a.out --ignore-struct-names "src/*"
```

## Specifying a minimum size for leaks

By default, Cpplumber ignores leaks stricly smaller than 4 bytes. This
diminishes the chance of reporting false positives and spamming the reports
with useless data.

However, it's possible to modify this behavior if needed:
```sh
cpplumber --bin a.out --minimum-leak-size 6 "src/*"
```

This tells Cpplumber to ignore potential leaks that would be of less than
6 bytes in size. Keep in mind that this is in bytes, so for example, a single
UTF-32 character would be considered a 4-byte leak.

# Importing a project

## CMake

To make things easier, it is possible to specify a compilation database to
fetch source file paths and compiler arguments from. With CMake, you can
generate a compilation database like so:

```sh
cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=ON .
```

A `compile_commands.json` file should have been created in the current folder.
You can now run `cpplumber` with the appropriate argument:

```sh
cpplumber --bin a.out --project=compile_commands.json
```

To ignore certain files or folders you can use suppression filters (see the
next section for details).

## Visual Studio

Support for Visual Studio's project and solution files is not yet implemented.
In the mean time, you can use tools like [Clang Power Tools](https://clangpowertools.com/)
to manually generate compilation databases from projects and solutions.

# Suppressions

Suppression files are YAML configuration files that can be used to prevent some
files or artifacts from generating leak reports.
Here's a simple example:

```yml
# Files to ignore (can include glob expressions)
files:
  - "*\\file2.cc"
  - "*\\extern\\*"

# Artifacts to ignore
artifacts:
  - nonsensitive_c_string
  - nonsensitive_utf32_string
```

To specify a suppression file:

```sh
cpplumber --bin a.out --suppressions-list suppressions.yml "src/*"
```

## Reporting leaks from system headers

By default, Cpplumber ignores potentially leaking data coming from system
headers as it's most likely nonsensitive data.
It's possible to tell Cpplumber to do otherwise with a command-line argument:
```sh
cpplumber --bin a.out --report-system-headers "src/*"
```

## Suppressing multiple reports for a single artifact

In larger projects, and especially for string literals, it may happen that
multiple declarations exist that lead to a single leak in the compiled binary,
or that the same string literal is found multiple times in the target binary.

By default, Cpplumber keeps track of and reports all source-to-binary
correspondences, but it's possible to force it to generate a single report
for each leaked artifacts like so:
```sh
cpplumber --bin a.out --ignore-multiple-locations "src/*"
```

This allow generating reports that give a compact overview of *unique* data
leaks that happen across a project.

# JSON output

Cpplumber can generate its output in JSON format. You can use the `--json`
argument for that:

```sh
cpplumber --json --bin a.out "src/*"
```

Here's an example of JSON reports generated by Cpplumber:
```json
{
    "version": {
        "executable": "0.1.0",
        "format": 1
    },
    "leaks": [
        {
            "data_type": "StringLiteral",
            "data": "sensitive_utf8_string",
            "location": {
                "source": {
                    "file": "/full/path/to/file.cc",
                    "line": 4
                },
                "binary": {
                    "file": "/full/path/to/a.out",
                    "offset": 86326
                }
            }
        }
    ]
}
```

## The `version` object

The `version` object contains information that can help contextualize and parse
the rest of the report.

* `executable`: Version of the `cpplumber` executable that generated the report
* `format`: Version of the report's format

## The `leaks` object

* `data_type`: The kind of data that has been leaked. Can be `StringLiteral`,
`StructName` or `ClassName`.
* `data`: The data that has been leaked, as declared in the source
code
* `location`: An object that contains two sub-objects which indicate where the
leaked data is located, in the `source` code and in the `binary` file.

# Caveats

## Cross-platform binary analysis

The resolution of wide char's sizes is currently done automatically. On Linux
and Mac, Cpplumber assumes that a `wchar_t` is 4-byte long (and encoded as
UTF-32) and on Windows, it assumes that a `wchar_t` is 2-byte long (and encoded
as UTF-16).

So keep in mind that analyzing an EXE file on Linux might miss leaks of wide
strings.


## Scaling

Cpplumber depends on the `clang` crate to use `libclang` and it's not currently
possible to parallelize source file parsing with that crate. Expect Cpplumber to
take a few minutes to parse source files on larger projects.

