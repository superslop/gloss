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
