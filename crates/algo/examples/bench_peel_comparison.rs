use std::process::Command;

fn main() -> std::process::ExitCode {
    let run = Command::new("python3").arg("scripts/benchmark-peel-frontiers.py").args(std::env::args_os().skip(1)).status();
    match run {
        Ok(status) if status.success() => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(error) => {
            eprintln!("start benchmark comparison: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
