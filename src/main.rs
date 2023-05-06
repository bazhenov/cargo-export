use getopts::{Fail, Options};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{exit, Command, Stdio},
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
    // skipping program name in arguments list
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let args = args.iter().map(|s| s.as_ref()).collect::<Vec<&str>>();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("v", "verbose", "prints files copied");
    opts.optflag(
        "n",
        "no-options",
        "do not add default cargo options (--no-run, --message-format)",
    );

    // splitting our/cargo arguments using `--` as a delimeter
    let (self_args, cargo_args) = if let Some(pos) = args.iter().position(|i| *i == "--") {
        (&args[0..pos], &args[pos + 1..])
    } else {
        (&args[..], &args[0..0])
    };
    let mut cargo_args = cargo_args.iter().map(|s| s.as_ref()).collect::<Vec<_>>();

    let matches = match opts.parse(self_args) {
        Ok(m) => m,
        Err(f) => print_usage(&opts, Some(f)),
    };

    if matches.opt_present("h") {
        print_usage(&opts, None);
    }

    if cargo_args.is_empty() {
        print_usage(
            &opts,
            Some(Fail::OptionMissing("CARGO_COMMAND".to_string())),
        );
    }

    if !matches.opt_present("n") {
        cargo_args.insert(1, "--message-format=json");
        cargo_args.insert(1, "--no-run");
    }

    let verbose = matches.opt_present("v");
    let Some(target) = matches.free.get(0) else {
        print_usage(&opts, Some(Fail::OptionMissing("PATH".to_string())));
    };
    let target = PathBuf::from(target);
    if !target.exists() {
        fs::create_dir_all(&target).unwrap();
    }

    let mut command = Command::new("cargo")
        .args(cargo_args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = command.stdout.take().unwrap();
    let stdout = BufReader::new(stdout);
    let mut artifacts = Vec::new();
    for line in stdout.lines() {
        let line = line.unwrap();
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            if verbose {
                eprintln!("cargo output: {}", line);
            }
            panic!("Unable to parse json from cargo");
        };
        let message = serde_json::from_value::<CompilerArtifact>(value)
            .ok()
            .filter(|m| m.reason == "compiler-artifact");
        if let Some(message) = message {
            artifacts.push(message);
        };
    }
    command.wait().unwrap();

    for artfact in artifacts {
        let from = PathBuf::from(&artfact.executable);
        let file_name = from.file_name().and_then(|n| n.to_str()).unwrap();
        let file_name = trim_hash(file_name).unwrap_or(file_name);
        let to = target.join(file_name);

        if verbose {
            eprintln!(
                "[cargo-export] copying '{}' to '{}'",
                from.display(),
                to.display()
            );
        }
        fs::copy(from, to).unwrap();
    }
}

fn print_usage(opts: &Options, fail: Option<Fail>) -> ! {
    if let Some(fail) = fail {
        eprintln!("[ERROR]: {}", fail);
        eprintln!();
    }
    let brief = "usage: cargo export [OPTIONS] PATH -- CARGO_COMMAND [CARGO_OPTIONS...]";
    eprintln!("{}", opts.usage(brief));

    eprintln!();
    eprintln!("  Examples:");
    eprintln!();
    eprintln!("    $ cargo export target/tests -- test");
    eprintln!("      Exporting all test binaries in target/tests directory");
    eprintln!();
    eprintln!("    $ cargo export target/benches -- bench");
    eprintln!("      Exporting all benchmark binaries in target/tests directory");
    eprintln!();
    exit(1)
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
