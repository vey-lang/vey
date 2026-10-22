use std::env;
use std::process::ExitCode;

mod cli;

use cli::{
    cmd_add, cmd_build, cmd_check, cmd_deploy, cmd_dev, cmd_docs, cmd_explain, cmd_fmt, cmd_lint, cmd_lsp,
    cmd_migrate, cmd_new, cmd_repl, cmd_run, cmd_test, print_help,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    ExitCode::from(run(&args) as u8)
}

fn run(args: &[String]) -> i32 {
    let Some(command) = args.first() else {
        print_help();
        return 0;
    };

    let rest = &args[1..];
    match command.as_str() {
        "run" => cmd_run(rest),
        "test" => cmd_test(rest),
        "build" => cmd_build(rest),
        "dev" => cmd_dev(rest),
        "new" => cmd_new(rest),
        "check" => cmd_check(rest),
        "fmt" => cmd_fmt(rest),
        "lint" => cmd_lint(rest),
        "repl" => cmd_repl(rest),
        "explain" => cmd_explain(rest),
        "docs" => cmd_docs(rest),
        "migrate" => cmd_migrate(rest),
        "deploy" => cmd_deploy(rest),
        "lsp" => cmd_lsp(rest),
        "add" => cmd_add(rest),
        "curate" => {
            eprintln!("'vey curate' was removed. Use 'vey new <directory>' instead.");
            1
        }
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        "version" | "--version" | "-v" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            0
        }
        other => {
            eprintln!("unknown command: {}", other);
            eprintln!("run 'vey help' for usage");
            1
        }
    }
}
