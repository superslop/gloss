//! Integration tests for the Gloss interpreter.
//!
//! Each test runs a Gloss source string through the library API and
//! asserts the exact captured output.

use gloss::{run_source, Interpreter};

fn run(source: &str) -> String {
    run_source(source).expect("test program should run without an error")
}

#[test]
fn arithmetic() {
    let output = run(
        "
        print(1 + 2 * 3);
        print((1 + 2) * 3);
        print(10 % 3);
        print(7 / 2);
        print(-5 + 2);
        ",
    );
    assert_eq!(output, "7\n9\n1\n3.5\n-3\n");
}

#[test]
fn variables_and_scope() {
    let output = run(
        "
        let x = 1;
        {
            let x = 2;
            print(x);
            let y = 3;
            print(y);
        }
        print(x);
        x = 5;
        print(x);
        ",
    );
    assert_eq!(output, "2\n3\n1\n5\n");
}

#[test]
fn assignment_reaches_outer_scope_from_block() {
    let output = run(
        "
        let total = 0;
        {
            total = 10;
        }
        print(total);
        ",
    );
    assert_eq!(output, "10\n");
}

#[test]
fn if_else() {
    let output = run(
        "
        let score = 72;
        if score >= 90 {
            print(\"A\");
        } else if score >= 70 {
            print(\"B\");
        } else {
            print(\"C\");
        }
        if score < 50 {
            print(\"low\");
        } else {
            print(\"ok\");
        }
        ",
    );
    assert_eq!(output, "B\nok\n");
}

#[test]
fn while_loop() {
    let output = run(
        "
        let i = 1;
        let total = 0;
        while i <= 5 {
            total = total + i;
            i = i + 1;
        }
        print(total);
        ",
    );
    assert_eq!(output, "15\n");
}

#[test]
fn recursive_function_fib() {
    let output = run(
        "
        fn fib(n) {
            if n < 2 {
                return n;
            }
            return fib(n - 1) + fib(n - 2);
        }
        print(fib(10));
        ",
    );
    assert_eq!(output, "55\n");
}

#[test]
fn closure_counter() {
    let output = run(
        "
        fn make_counter() {
            let count = 0;
            fn increment() {
                count = count + 1;
                return count;
            }
            return increment;
        }
        let next = make_counter();
        print(next());
        print(next());
        let other = make_counter();
        print(other());
        print(next());
        ",
    );
    assert_eq!(output, "1\n2\n1\n3\n");
}

#[test]
fn first_class_functions() {
    let output = run(
        "
        fn apply(f, value) {
            return f(value);
        }
        fn double(n) {
            return n * 2;
        }
        print(apply(double, 21));
        let square = fn(n) {
            return n * n;
        };
        print(square(9));
        ",
    );
    assert_eq!(output, "42\n81\n");
}

#[test]
fn arrays_and_maps() {
    let output = run(
        "
        let a = [1, 2, 3];
        print(a[0]);
        a[1] = 20;
        push(a, 4);
        print(len(a));
        print(a);
        let m = {\"name\": \"Ana\", \"age\": 30};
        print(m[\"name\"]);
        m[\"age\"] = 31;
        m[\"city\"] = \"Metropolis\";
        print(m[\"age\"]);
        print(len(m));
        print(m[\"missing\"]);
        ",
    );
    assert_eq!(output, "1\n4\n[1, 20, 3, 4]\nAna\n31\n3\nnil\n");
}

#[test]
fn string_concatenation() {
    let output = run(
        "
        let greeting = \"Hello\" + \", \" + \"Gloss\" + \"!\";
        print(greeting);
        print(\"n = \" + 42);
        print(len(\"hello\"));
        ",
    );
    assert_eq!(output, "Hello, Gloss!\nn = 42\n5\n");
}

#[test]
fn for_in_loop() {
    let output = run(
        "
        let numbers = [10, 25, 30, 40];
        let sum = 0;
        for n in numbers {
            sum = sum + n;
        }
        print(sum);
        ",
    );
    assert_eq!(output, "105\n");
}

#[test]
fn booleans_logic_and_nil() {
    let output = run(
        "
        print(true && false);
        print(true || false);
        print(!false);
        let nothing = nil;
        print(nothing);
        print(nothing == nil);
        ",
    );
    assert_eq!(output, "false\ntrue\ntrue\nnil\ntrue\n");
}

#[test]
fn errors_carry_line_numbers() {
    let result = run_source("let a = 1;\nprint(missing);");
    let error = result.expect_err("reading an undefined variable must fail");
    assert!(error.to_string().contains("line 2"), "error was: {error}");
    assert!(
        error.to_string().contains("undefined variable"),
        "error was: {error}"
    );
}

#[test]
fn runtime_error_division_by_zero() {
    let result = run_source("print(1 / 0);");
    let error = result.expect_err("division by zero must fail");
    assert!(
        error.to_string().contains("division by zero"),
        "error was: {error}"
    );
}

#[test]
fn interpreter_keeps_state_between_calls() {
    // This is the property the REPL relies on.
    let mut interpreter = Interpreter::new();
    interpreter.interpret("let saved = 40;").unwrap();
    interpreter.interpret("print(saved + 2);").unwrap();
    assert_eq!(interpreter.take_output(), "42\n");
}

// ----- v0.2 features -------------------------------------------------------

#[test]
fn string_interpolation() {
    let output = run(
        "
        let name = \"Ana\";
        fn double(n) { return n * 2; }
        let a = [10, 20];
        print(\"Hello, {name}, sum = {1 + 2}, double = {double(4)}, pick = {a[1]}\");
        print(\"literal {{braces}} and }} {{\");
        ",
    );
    assert_eq!(
        output,
        "Hello, Ana, sum = 3, double = 8, pick = 20\nliteral {braces} and } {\n"
    );
}

#[test]
fn string_interpolation_keeps_escapes_working() {
    let output = run("print(\"a\\nb {1 + 1} \\\"q\\\"\");");
    assert_eq!(output, "a\nb 2 \"q\"\n");
}

#[test]
fn match_expression() {
    let output = run(
        "
        let label = match 2 { 1 => \"one\", 2 => \"two\", _ => \"other\" };
        print(label);
        print(match \"x\" { \"x\" => \"ex\", _ => \"why\" });
        let n = match 41 { m => m + 1 };
        print(n);
        print(match true { false => \"no\", true => \"yes\" });
        print(match nil { nil => \"nothing\", _ => \"something\" });
        print(match 9 { 1 => \"one\", _ => \"other\" });
        ",
    );
    assert_eq!(output, "two\nex\n42\nyes\nnothing\nother\n");
}

#[test]
fn match_without_a_matching_arm_is_an_error() {
    let result = run_source("let x = match 5 { 1 => \"one\", 2 => \"two\" };");
    let error = result.expect_err("a match with no matching arm must fail");
    assert!(
        error.to_string().contains("no pattern matched"),
        "error was: {error}"
    );
}

#[test]
fn pipe_operator() {
    let output = run(
        "
        fn add(a, b) { return a + b; }
        fn triple(n) { return n * 3; }
        print(10 |> add(5));
        print(3 |> triple);
        print(1 |> add(2) |> triple);
        print([1, 2] |> len);
        ",
    );
    assert_eq!(output, "15\n9\n9\n2\n");
}

#[test]
fn try_catch_recovers_and_continues() {
    let output = run(
        "
        try {
            print(1 / 0);
        } catch err {
            print(\"caught: \" + err);
        }
        print(\"after\");
        try {
            print(\"fine\");
        } catch err {
            print(\"not reached\");
        }
        ",
    );
    assert_eq!(output, "caught: division by zero\nafter\nfine\n");
}

#[test]
fn try_catch_return_still_works_and_catch_errors_propagate() {
    let output = run(
        "
        fn f() {
            try {
                return 7;
            } catch err {
                return 9;
            }
            return 0;
        }
        print(f());
        ",
    );
    assert_eq!(output, "7\n");

    let result = run_source(
        "try { print(1 / 0); } catch err { print(missing_variable); }",
    );
    let error = result.expect_err("an error inside catch must propagate");
    assert!(
        error.to_string().contains("undefined variable"),
        "error was: {error}"
    );
}

#[test]
fn dot_access_for_maps() {
    let output = run(
        "
        let user = {\"name\": \"Ana\", \"address\": {\"city\": \"Metropolis\"}};
        print(user.name);
        print(user.address.city);
        user.age = 31;
        user.address.city = \"Gotham\";
        print(user.age);
        print(user.address.city);
        print(user.missing);
        ",
    );
    assert_eq!(output, "Ana\nMetropolis\n31\nGotham\nnil\n");
}

#[test]
fn dot_access_on_a_non_map_is_an_error() {
    let result = run_source("let n = 5;\nprint(n.field);");
    let error = result.expect_err("dot access on a number must fail");
    assert!(
        error.to_string().contains("only maps have fields"),
        "error was: {error}"
    );
    assert!(error.to_string().contains("line 2"), "error was: {error}");
}

#[test]
fn default_parameters() {
    let output = run(
        "
        fn greet(name = \"world\") { return \"Hello, \" + name; }
        print(greet());
        print(greet(\"Gloss\"));
        fn describe(item, qty = 2) { return item + \":\" + qty; }
        print(describe(\"pen\"));
        print(describe(\"pen\", 5));
        fn make(a, b = a * 10) { return b; }
        print(make(4));
        ",
    );
    assert_eq!(output, "Hello, world\nHello, Gloss\npen:2\npen:5\n40\n");
}

#[test]
fn default_parameters_still_check_arity() {
    let missing = run_source("fn f(a, b) { return a; } print(f(1));");
    let error = missing.expect_err("a missing argument without a default must fail");
    assert!(
        error.to_string().contains("argument"),
        "error was: {error}"
    );

    let too_many = run_source("fn f(a, b = 2) { return a; } print(f(1, 2, 3));");
    let error = too_many.expect_err("too many arguments must fail");
    assert!(
        error.to_string().contains("argument"),
        "error was: {error}"
    );
}

#[test]
fn type_str_and_num_builtins() {
    let output = run(
        "
        fn f() {}
        print(type(42), type(\"s\"), type(true), type(nil));
        print(type([1]), type({\"a\": 1}), type(len), type(f));
        print(str(42) + \"!\");
        print(str([1, 2]));
        print(num(\"3.5\") + 1);
        print(num(7));
        ",
    );
    assert_eq!(
        output,
        "number string bool nil\narray map function function\n42!\n[1, 2]\n4.5\n7\n"
    );

    let bad = run_source("print(num(\"abc\"));");
    let error = bad.expect_err("num on an unparseable string must fail");
    assert!(
        error.to_string().contains("could not parse"),
        "error was: {error}"
    );
}

#[test]
fn string_builtins() {
    let output = run(
        "
        print(upper(\"gloss\"), lower(\"GLOSS\"), trim(\"  hi  \"));
        print(contains(\"hello\", \"ell\"), contains(\"hello\", \"z\"));
        print(contains([1, 2, 3], 2), contains([1, 2, 3], 9));
        print(split(\"a,b,c\", \",\"));
        print(join([\"a\", \"b\", \"c\"], \"-\"));
        print(join(split(\"a,b\", \",\"), \"+\"));
        ",
    );
    assert_eq!(
        output,
        "GLOSS gloss hi\ntrue false\ntrue false\n[a, b, c]\na-b-c\na+b\n"
    );
}

#[test]
fn math_builtins() {
    let output = run(
        "
        print(abs(-7), abs(3));
        print(floor(2.9), ceil(2.1), round(2.5), round(2.4));
        print(sqrt(16));
        print(min(3, 9), max(3, 9));
        ",
    );
    assert_eq!(output, "7 3\n2 3 3 2\n4\n3 9\n");
}

#[test]
fn range_and_higher_order_builtins() {
    let output = run(
        "
        print(range(4));
        print(range(2, 5));
        print(map([1, 2, 3], fn(n) { return n * 2; }));
        print(filter([1, 2, 3, 4, 5], fn(n) { return n % 2 == 0; }));
        print(reduce([1, 2, 3, 4], 0, fn(acc, n) { return acc + n; }));
        let factor = 10;
        print(map([1, 2], fn(n) { return n * factor; }));
        ",
    );
    assert_eq!(output, "[0, 1, 2, 3]\n[2, 3, 4]\n[2, 4, 6]\n[2, 4]\n10\n[10, 20]\n");
}

#[test]
fn args_builtin_defaults_to_empty() {
    let output = run("print(args());");
    assert_eq!(output, "[]\n");

    let mut interpreter = Interpreter::new();
    interpreter.set_args(vec!["a".to_string(), "b".to_string()]);
    interpreter.interpret("print(args());").unwrap();
    assert_eq!(interpreter.take_output(), "[a, b]\n");
}

#[test]
fn import_loads_a_library_file() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let output = gloss::run_source_in_dir(
        "import \"mathlib.gloss\";\nprint(double(21));\nprint(library_name);",
        &fixtures,
    )
    .expect("importing the fixture library should work");
    assert_eq!(output, "42\nmathlib\n");
}

#[test]
fn import_cycle_is_an_error() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let result = gloss::run_source_in_dir("import \"cycle_a.gloss\";", &fixtures);
    let error = result.expect_err("an import cycle must fail instead of recursing forever");
    assert!(
        error.to_string().contains("import cycle"),
        "error was: {error}"
    );
}

#[test]
fn read_and_write_file_builtins() {
    let path = std::env::temp_dir().join(format!("gloss_v02_test_{}.txt", std::process::id()));
    let path_text = path.to_string_lossy().replace('\\', "/");
    let source = format!(
        "write_file(\"{path_text}\", \"hello gloss\");\nprint(read_file(\"{path_text}\"));"
    );
    let output = run(&source);
    assert_eq!(output, "hello gloss\n");
    let _ = std::fs::remove_file(&path);

    let missing = run_source("print(read_file(\"/nonexistent/gloss/nope.txt\"));");
    let error = missing.expect_err("reading a missing file must fail");
    assert!(
        error.to_string().contains("could not read"),
        "error was: {error}"
    );
}
