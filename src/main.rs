use serde::Deserialize;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Deserialize, Debug)]
struct CompilerArtifact {
    reason: String,
    executable: String,
}

// #[derive(Parser, Debug)]
// #[command(author, version, about, long_about = None)]
// struct Opts {}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let extended_args_pos = args.iter().position(|i| i == "--");
    let Some(extended_args_pos) = extended_args_pos else {
        panic!("Ooops");
    };
    let _own_args = args[0..extended_args_pos].iter().collect::<Vec<_>>();
    let mut cargo_args = args[extended_args_pos + 1..]
        .iter()
        .map(|i| i.as_str())
        .collect::<Vec<_>>();

    cargo_args.insert(1, "--message-format=json");
    cargo_args.insert(1, "--no-run");

    let mut command = Command::new("cargo")
        .args(cargo_args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = command.stdout.take().unwrap();
    let stdout = BufReader::new(stdout);
    let mut artifacts = Vec::new();
    for line in stdout.lines() {
        let message = serde_json::from_str::<CompilerArtifact>(&line.unwrap());
        if let Ok(message) = message {
            if message.reason == "compiler-artifact" {
                artifacts.push(message);
            }
        };
    }
    command.wait().unwrap();

    for artfact in artifacts {
        let from = PathBuf::from(&artfact.executable);
        let file_name = from.file_name().and_then(|n| n.to_str()).unwrap();
        let file_name = trim_hash(file_name).unwrap_or(file_name);
        let to = format!("./{}", file_name);

        eprintln!("[cargo-export] copying '{}' to '{}'", from.display(), to);
        fs::copy(from, to).unwrap();
    }
}

/// Trims cargo hashe from file name (eg. `app-ebb8dd5b587f73a1` -> `app`)
fn trim_hash(input: &str) -> Option<&str> {
    let idx = match input.rfind('-') {
        Some(idx) if idx > 0 => idx,
        _ => return None,
    };

    let hash = &input[idx + 1..];
    // it's safe to check number of bytes instead of chars here because we still
    // check all the characters individually
    if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(&input[..idx])
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use crate::trim_hash;

    #[test]
    fn check_trim_hash() {
        assert_eq!(trim_hash("app-ebb8dd5b587f73a1"), Some("app"));
        assert_eq!(trim_hash("🥰-ebb8dd5b587f73a1"), Some("🥰"));
        assert_eq!(trim_hash("app-ebb8dd5b5"), None);
        assert_eq!(trim_hash("app-ebb8dd5b5000000000000000"), None);
        assert_eq!(trim_hash("app-"), None);
        assert_eq!(trim_hash("app"), None);
        assert_eq!(trim_hash(""), None);
        assert_eq!(trim_hash("-ebb8dd5b587f73a1"), None);
        assert_eq!(trim_hash("ebb8dd5b587f73a1"), None);
    }
}
