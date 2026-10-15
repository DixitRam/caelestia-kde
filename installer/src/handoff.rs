use std::path::Path;
use std::process::Command;

use crate::exit::Outcome;

pub fn run_script(path: &Path) -> Outcome {
    if !path.is_file() {
        eprintln!("caelestia-install: {} is missing", path.display());
        return Outcome::Environment;
    }

    match Command::new("bash").arg(path).status() {
        Ok(status) if status.success() => {
            println!("\nFinished.");
            Outcome::Ok
        }
        Ok(status) => {
            println!("\nFinished with errors.");
            if let Some(code) = status.code() {
                eprintln!("caelestia-install: {} exited {code}", path.display());
            }
            Outcome::Failed
        }
        Err(error) => {
            eprintln!(
                "caelestia-install: could not run {}: {error}",
                path.display()
            );
            Outcome::Environment
        }
    }
}
