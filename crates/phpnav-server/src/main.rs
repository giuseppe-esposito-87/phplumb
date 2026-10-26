use std::process::ExitCode;

mod bench;
mod convert;
mod lsp;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => match lsp::serve() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("php-navigator: {err:#}");
                ExitCode::FAILURE
            }
        },
        Some("bench") => match bench::BenchArgs::parse(&args[1..]).and_then(|a| bench::run(&a)) {
            Ok(report) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serializes")
                );
                if report.passed() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(err) => {
                eprintln!("php-navigator bench: {err:#}");
                ExitCode::from(2)
            }
        },
        Some("--version") => {
            println!("php-navigator {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "php-navigator: unknown command {other}\nusage: php-navigator [serve | bench --root <DIR> [--cases <FILE>] [--cache <FILE>] [--coverage <DIR>] [--exclude <GLOB>]... | --version]"
            );
            ExitCode::FAILURE
        }
    }
}
