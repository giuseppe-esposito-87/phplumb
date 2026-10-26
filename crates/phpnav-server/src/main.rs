use std::process::ExitCode;

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
        Some("--version") => {
            println!("php-navigator {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "php-navigator: unknown command {other}\nusage: php-navigator [serve | --version]"
            );
            ExitCode::FAILURE
        }
    }
}
