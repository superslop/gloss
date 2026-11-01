# Gloss

[![CI](https://github.com/superslop/gloss/actions/workflows/ci.yml/badge.svg)](https://github.com/superslop/gloss/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/superslop/gloss)](https://github.com/superslop/gloss/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**Gloss is a programming language implemented in Rust that removes Rust's
user-facing pain points — no lifetimes, no borrow checker, no compile wait in
development — while keeping a safe, fast Rust binary underneath.**

You write dynamic, readable code with string interpolation, pattern matching,
pipes, and a batteries-included standard library. The interpreter that runs it
is written in safe, std-only Rust (zero external crates).

> Name note: other, unrelated projects also use the name "Gloss" (for example
> a Haskell graphics library and a Ruby dialect). This project is independent
> of them.

## Rust pain points → Gloss fixes

| Rust pain point | Gloss fix | Status |
|---|---|---|
| Fighting the borrow checker | Values are shared and mutated freely — no ownership, no `Rc<RefCell<...>>` in your code; the interpreter handles memory internally | ✅ Delivered |
| Lifetime syntax (`&'a`, elision rules) | No lifetimes are exposed at all — there is no syntax for them | ✅ Delivered |
| Slow compile–run iteration | The interpreter runs your program instantly; there is no compile step in development | ✅ Delivered |
| Compile wait for release builds | A transpiler to Rust (native binary out) is planned for releases only, so the wait happens once per release, not per edit | 🗺️ Roadmap |
| Async / trait / generic complexity | Dynamic values and one small standard library instead of trait-bound puzzles | ✅ Delivered (for generics/traits) |
| Concurrency model complexity | Not solved yet — Gloss is single-threaded today | 🗺️ Roadmap |
| Cryptic, multi-page type errors | Short, single-line errors that name the problem and the line number | ✅ Delivered |
| Ecosystem choice paralysis (which crate for X?) | A batteries-included standard library: strings, math, collections, file I/O, higher-order functions | ✅ Delivered |

## Feature tour (v0.2)

This program (`examples/tour.gloss`) exercises interpolation, default
parameters, dot access, pipes, `map`/`filter`/`reduce`, `match`, and
`try`/`catch` in a few lines:

```gloss
let people = [
    {"name": "ana", "age": 31},
    {"name": "bo", "age": 17},
    {"name": "cy", "age": 25}
];

fn greet(name = "world") {
    return "Hello, {name}!";
}

let adults = people
    |> filter(fn(p) { return p.age >= 18; })
    |> map(fn(p) { return upper(p.name); });

print(greet());
print(adults);
print(match len(adults) {
    0 => "none",
    n => "{n} adults"
});

let total = range(1, 5) |> reduce(0, fn(acc, n) { return acc + n; });
print("total = {total}");

try {
    print(total / 0);
} catch err {
    print("caught: {err}");
}
```

Output:

```
Hello, world!
[ANA, CY]
2 adults
total = 10
caught: division by zero
```

## The same task in Rust and in Gloss

Rust (correct, safe — and ceremony-heavy for a small task):

```rust
fn main() {
    let mut names = vec!["ana", "bo", "cy"];
    names.push("di");
    let loud: Vec<String> = names.iter().map(|n| n.to_uppercase()).collect();
    println!("{:?}", loud);
}
```

Gloss:

```gloss
let names = ["ana", "bo", "cy"];
push(names, "di");
let loud = map(names, fn(n) { return upper(n); });
print(loud);
```

Both print the same list (Gloss prints strings without quotes:
`[ANA, BO, CY, DI]`).

**Honest trade-offs:** Gloss is interpreted and dynamically typed. It trades
Rust's raw performance and compile-time guarantees for iteration speed and
simplicity. Errors that Rust catches at compile time surface in Gloss at
runtime (with line numbers, and `try`/`catch` to handle them). Gloss is not
faster than Rust, and it is not a replacement for Rust where compile-time
guarantees are the point — it is the comfortable layer you reach for when
they are not.

## Features

**Language**

- Variables (`let`, assignment), block scope, `if`/`else`, `while`, `for-in`
- First-class functions, closures, recursion, anonymous `fn` expressions
- **Default parameter values**, evaluated at call time
- **String interpolation**: `"sum = {1 + 2}"`, with `{{` / `}}` for literal braces
- **`match` expressions**: literal patterns, `_` wildcard, binding patterns
- **Pipe operator `|>`**: `x |> f` is `f(x)`; `x |> f(a)` is `f(x, a)`
- **`try`/`catch`**: runtime errors become a catchable message string
- Arrays and maps, index access (`a[i]`, `m["k"]`) and **dot access** (`user.name`)
- **`import "file.gloss"`**: split programs across files
- Numbers, strings, booleans, `nil`; arithmetic, comparison, and logic operators

**Standard library (builtins)**

- Output and conversion: `print`, `str`, `num`, `type`, `args`
- Strings: `upper`, `lower`, `trim`, `split`, `join`, `contains`
- Collections: `len`, `push`, `contains`, `map`, `filter`, `reduce`, `range`
- Math: `abs`, `floor`, `ceil`, `round`, `sqrt`, `min`, `max`
- Files: `read_file`, `write_file`

## Syntax summary

```gloss
// Comments start with two slashes.
let name = "Gloss";              // declaration
name = name + "!";               // assignment (declare first with let)

let n = 1 + 2 * 3;               // arithmetic: + - * / %
print(n == 7, n < 10);           // comparison: == != < <= > >=
print(true && !false || false);  // logic: && || !
print("n is {n}");               // interpolation; {{ and }} are literal braces

if n > 5 {
    print("big");
} else if n == 5 {
    print("five");
} else {
    print("small");
}

let i = 0;
while i < 3 { i = i + 1; }

fn fib(k) {                      // functions are first-class values
    if k < 2 { return k; }
    return fib(k - 1) + fib(k - 2);
}
fn greet(who = "world") {        // default parameter values
    return "Hello, {who}!";
}

let label = match n {            // match is an expression
    7 => "lucky",
    0 => "zero",
    _ => "other"
};

let list = [1, 2, 3];            // arrays
list[0] = 10;                    // index assignment
push(list, 4);                   // append in place
for item in list { print(item); }
let doubled = list |> map(fn(v) { return v * 2; });   // pipe + higher-order

let user = {"name": "Ana"};      // maps with string keys
user.age = 30;                   // dot assignment (same as user["age"] = 30)
print(user.name, len(user));     // dot access (same as user["name"])

try {
    print(1 / 0);
} catch err {
    print("caught: {err}");      // err is the error message string
}

import "lib/helpers.gloss";      // run another file in the global scope

let nothing = nil;               // nil value
```

### `import` semantics

`import "path";` reads, parses, and executes the file in the **global**
environment: every function and variable it defines at the top level becomes
available after the import. Relative paths resolve against the directory of
the file doing the importing (for the main script, its own directory; for the
REPL and tests, the interpreter's configured base directory or the current
working directory). Importing a file that is currently being imported —
directly or through a chain — is a runtime error ("import cycle detected")
instead of infinite recursion.

## Installing

Download a prebuilt binary from the
[Releases](https://github.com/superslop/gloss/releases) page — Linux
(x86_64), Windows (x86_64), and macOS (Apple Silicon) builds are attached to
each release. No Rust installation is needed:

```sh
chmod +x gloss-linux-x86_64        # Linux/macOS builds only
./gloss-linux-x86_64 run your-program.gloss
./gloss-linux-x86_64               # start the REPL
```

Or build from source with a Rust toolchain:

```sh
cargo build --release
./target/release/gloss run examples/showcase.gloss
```

## Running

```sh
cargo build
cargo run -- run examples/hello.gloss       # run a file
cargo run -- run examples/tour.gloss        # the v0.2 feature tour
cargo run -- run examples/showcase.gloss    # every feature, deterministic output
cargo run -- run script.gloss a b c         # args() in the program returns ["a", "b", "c"]
cargo run                                   # start the REPL (prompt: gloss> )
cargo run -- --help
```

In the REPL, variables and functions persist between lines, and an error on
one line is reported without ending the session.

## Testing

```sh
cargo test
```

Integration tests in `tests/run_tests.rs` run Gloss source strings through
the library API (`gloss::run_source`) and assert the captured output;
`tests/fixtures/` holds the sample library and import-cycle files used by
the import tests. CI (GitHub Actions, `.github/workflows/ci.yml`) builds,
runs the tests, and diffs the showcase program's output against
`examples/showcase.expected.txt`.

Errors — lex, parse, and runtime — carry a line number, are printed to
stderr, and make `gloss run` exit with a non-zero status.

## Decisions

| Question | Decision |
|---|---|
| Implementation language | Rust |
| Execution route (now) | Tree-walk interpreter |
| Execution route (later) | Bytecode VM, then a transpiler to Rust — deferred |
| File extension | `.gloss` |
| Values / typing | Dynamic (numbers, strings, booleans, nil, arrays, maps, functions) |
| Memory model exposed to users | None — no lifetimes, no borrow checker, no manual memory |
| Dependencies | None (Rust standard library only) |
| String interpolation (v0.2) | `{expr}` inside strings; full expressions; `{{`/`}}` escape literal braces |
| Pipe precedence (v0.2) | Lowest, left-associative; desugars to a call at parse time |
| Error handling (v0.2) | Runtime errors carry line numbers; `try`/`catch` binds the message string |
| Modules (v0.2) | Path-based `import`, executed in the global environment, cycles are errors |
| Standard library (v0.2) | Batteries included: strings, math, collections, files, higher-order functions |

## Project layout

```
Cargo.toml                  package: gloss 0.2.0, std-only
src/
  main.rs                   CLI: run a file (with args), or the REPL
  lib.rs                    library API (run_source, run_source_in_dir, Interpreter)
  lexer.rs                  source text -> tokens (incl. interpolation scanning)
  parser.rs                 tokens -> AST (recursive descent)
  ast.rs                    AST node definitions
  interpreter.rs            tree-walk evaluator; builtins, imports, output buffer
  environment.rs            chained scopes (closures capture these)
  value.rs                  the dynamic Value type and its display rules
  error.rs                  GlossError with kind + line number
examples/
  hello.gloss               smallest program
  tour.gloss                the v0.2 feature tour from this README
  showcase.gloss            every feature, deterministic output
  showcase.expected.txt     exact expected stdout of the showcase
tests/
  run_tests.rs              integration tests
  fixtures/                 mathlib.gloss and the import-cycle pair
.github/workflows/ci.yml    build + test + showcase diff
```

## Roadmap

1. **v0.1 (done):** tree-walk interpreter, CLI, REPL, tests, first release.
2. **v0.2 (done):** core language features and a real standard library — see
   the changelog below.
3. **Later:** a bytecode compiler and VM for speed, keeping the same front
   end (lexer/parser/AST).
4. **Later:** a transpiler to Rust for release builds — Gloss source in,
   native binary out.
5. **Later:** a concurrency story (today Gloss is single-threaded).

## Changelog

### v0.2.0

- String interpolation (`{expr}`, `{{`/`}}` literal braces)
- `match` expressions (literal, wildcard, and binding patterns)
- Pipe operator `|>`
- `try`/`catch` error handling
- Dot access for maps (`user.name`, read and write, chained)
- Default parameter values
- `import` statements with cycle detection
- Standard library expansion: `type`, `str`, `num`, `upper`, `lower`,
  `trim`, `contains`, `split`, `join`, `abs`, `floor`, `ceil`, `round`,
  `sqrt`, `min`, `max`, `range`, `map`, `filter`, `reduce`, `read_file`,
  `write_file`, `args`
- CLI: extra arguments after the script path are exposed via `args()`;
  relative imports resolve against the script's directory

### v0.1.0

- Tree-walk interpreter in Rust: variables, scope, `if`/`while`/`for-in`,
  first-class functions and closures, arrays, maps, `print`/`len`/`push`
- CLI with `run` and REPL, integration tests, CI, prebuilt release binaries

## License

MIT — see [LICENSE](LICENSE).
