use std::process::ExitCode;

mod bench;
mod convert;
mod lsp;

// musl's allocator serializes threads, which stalls the parallel indexer.
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => match lsp::serve() {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("phplumb: {err:#}");
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
                eprintln!("phplumb bench: {err:#}");
                ExitCode::from(2)
            }
        },
        Some("builtins") => {
            let Some(stubs) = args.get(1) else {
                eprintln!("usage: phplumb builtins <phpstorm-stubs dir>");
                return ExitCode::FAILURE;
            };
            match phplumb_index::builtins::generate(std::path::Path::new(stubs)) {
                Ok(list) => {
                    print!("{list}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("phplumb builtins: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--version") => {
            println!("phplumb {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "phplumb: unknown command {other}\nusage: phplumb [serve | bench --root <DIR> [--cases <FILE>] [--cache <FILE>] [--coverage <DIR>] [--exclude <GLOB>]... | --version]"
            );
            ExitCode::FAILURE
        }
    }
}
