# Gloss

Gloss is a small programming language implemented in Rust. Its purpose is to be
a layer on top of Rust that removes Rust's user-facing pain points from the
code you write:

- **No lifetimes** in Gloss code — there is no `&'a` syntax at all.
- **No borrow checker** in Gloss code — values are shared and mutated freely;
  the interpreter manages memory with reference counting internally.
- **No trait bounds or generic type errors** — Gloss values are dynamic.

The safety and performance of Rust are used where they belong: in the
implementation. The interpreter itself is written in safe, std-only Rust.

> Name note: other, unrelated projects also use the name "Gloss" (for example
> a Haskell graphics library and a Ruby dialect). This project is independent
> of them, and a rename before any public release is still possible.

## Status

**v0.1 — interpreter scaffold.** A working tree-walk interpreter: lexer,
parser, AST, and interpreter, plus a CLI, a REPL, integration tests, and an
example program that exercises every v0.1 feature
(`examples/showcase.gloss`, with its exact expected output in
`examples/showcase.expected.txt`).

## Decisions

| Question | Decision |
|---|---|
| Implementation language | Rust |
| Execution route (now) | Tree-walk interpreter |
| Execution route (later) | Transpiler to Rust — deferred |
| File extension | `.gloss` |
| Values / typing | Dynamic (numbers, strings, booleans, nil, arrays, maps, functions) |
| Memory model exposed to users | None — no lifetimes, no borrow checker, no manual memory |
| Dependencies | None (Rust standard library only) |

## Syntax summary

```gloss
// Comments start with two slashes.
let name = "Gloss";            // declaration
name = name + "!";             // assignment (variables must be declared first)

let n = 1 + 2 * 3;             // arithmetic: + - * / %
print(n == 7, n < 10);         // comparison: == != < <= > >=
print(true && !false || false); // logic: && || !

if n > 5 {
    print("big");
} else if n == 5 {
    print("five");
} else {
    print("small");
}

let i = 0;
while i < 3 {
    i = i + 1;
}

fn fib(k) {                    // functions are first-class values
    if k < 2 { return k; }
    return fib(k - 1) + fib(k - 2);
}

let list = [1, 2, 3];          // arrays
list[0] = 10;                  // index assignment
push(list, 4);                 // builtin: append in place
for item in list {             // for-in loop
    print(item);
}

let user = {"name": "Ana"};    // maps with string keys
user["age"] = 30;
print(user["name"], len(user)); // builtins: print (variadic), len

let nothing = nil;             // nil value
```

## Running

```sh
cargo build
cargo run -- run examples/hello.gloss      # run a file
cargo run -- run examples/showcase.gloss   # the full feature tour
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
the library API (`gloss::run_source`) and assert the captured output. CI
(GitHub Actions, `.github/workflows/ci.yml`) builds, runs the tests, and
diffs the showcase program's output against
`examples/showcase.expected.txt`.

Errors — lex, parse, and runtime — carry a line number, are printed to
stderr, and make `gloss run` exit with a non-zero status.

## Project layout

```
Cargo.toml                  package: gloss 0.1.0, std-only
src/
  main.rs                   CLI: run a file, or the REPL
  lib.rs                    library API (run_source, Interpreter)
  lexer.rs                  source text -> tokens
  parser.rs                 tokens -> AST (recursive descent)
  ast.rs                    AST node definitions
  interpreter.rs            tree-walk evaluator; owns the output buffer
  environment.rs            chained scopes (closures capture these)
  value.rs                  the dynamic Value type and its display rules
  error.rs                  GlossError with kind + line number
examples/
  hello.gloss               smallest program
  showcase.gloss            every v0.1 feature, deterministic output
  showcase.expected.txt     exact expected stdout of the showcase
tests/
  run_tests.rs              integration tests
.github/workflows/ci.yml    build + test + showcase diff
```

## Roadmap

1. **v0.1 (this scaffold):** tree-walk interpreter, CLI, REPL, tests.
2. **Later:** a bytecode compiler and VM for speed, keeping the same
   front end (lexer/parser/AST).
3. **Later:** a transpiler to Rust for release builds — Gloss source in,
   native binary out — so the "layer on top of Rust" route is available
   once the language itself is stable.
