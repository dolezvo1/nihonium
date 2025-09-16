
use std::process::Command;

fn main() {
    let commit_identifier = {
        if let Ok(output) = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .output() {
            let is_clean = match Command::new("git")
                .arg("ls-files")
                .arg("-m")
                .output() {
                    Ok(output) => String::from_utf8_lossy(&output.stdout).trim().is_empty(),
                    Err(_) => true,
                };

            format!("{}-{}", String::from_utf8_lossy(&output.stdout).trim(), if is_clean { "clean" } else { "custom" })
        } else {
            "unknown".to_string()
        }
    };

    println!("cargo:rustc-env=COMMIT_IDENTIFIER={}", commit_identifier);
}
