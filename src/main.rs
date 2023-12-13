use getopts::{Fail, Options};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::VecDeque,
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

fn main() {
    // skipping program name in arguments list
    let mut args = std::env::args().collect::<VecDeque<_>>();
    // skipping "cargo" and "export"
    args.pop_front();
    let subcommand_name = args.pop_front().unwrap_or_default();
    let args = args.iter().map(|s| s.as_ref()).collect::<Vec<&str>>();

    // splitting our/cargo arguments using `--` as a delimeter
    let (self_args, cargo_args) = if let Some(pos) = args.iter().position(|i| *i == "--") {
        (&args[0..pos], &args[pos + 1..])
    } else {
        (&args[..], &args[0..0])
    };

    let mut cargo_args = cargo_args.to_vec();

    let opts = build_opts();
    let matches = match opts.parse(self_args) {
        Ok(m) => m,
        Err(f) => print_usage(&opts, Some(f)),
    };

    if matches.opt_present("h") {
        print_usage(&opts, None);
    }

    if subcommand_name != "export" {
        print_usage(&opts, Some(Fail::UnrecognizedOption(subcommand_name)));
    }

    if cargo_args.is_empty() {
        print_usage(
            &opts,
            Some(Fail::OptionMissing("CARGO_COMMAND".to_string())),
        );
    }

    let Some(target) = matches.free.get(0) else {
        print_usage(&opts, Some(Fail::OptionMissing("PATH".to_string())));
    };

    if !matches.opt_present("n") {
        // inserting options right after cargo subcommand
        cargo_args.insert(1, "--message-format=json");
        cargo_args.insert(1, "--no-run");
    }

    let tag_name = matches.opt_str("t");
    let verbose = matches.opt_present("v");

    let target_dir = PathBuf::from(target);
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    let mut command = Command::new("cargo")
        .args(cargo_args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Unable to spawn cargo process");
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
    let exit_code = command.wait().expect("Failed executing cargo");
    if !exit_code.success() {
        eprintln!("[cargo-export] cargo exited with {} status code", exit_code);
        exit(1);
    }

    for artfact in artifacts {
        let from = PathBuf::from(&artfact.executable);
        let file_name = from.file_name().and_then(|n| n.to_str()).unwrap();
        let file_name = target_file_name(file_name, tag_name.as_deref());
        let to = target_dir.join(&file_name);

        if verbose {
            eprintln!(
                "[cargo-export] copying '{}' to '{}'",
                from.display(),
                to.display()
            );
        }
        fs::copy(from, to).expect("Unable to copy file");
    }
}

fn target_file_name(file_name: &str, tag_name: Option<&str>) -> String {
    let (name, _, ext) = split_file_name(file_name);
    let mut result = String::new();
    result.push_str(name);
    if let Some(tag_name) = tag_name {
        result.push('-');
        result.push_str(tag_name);
    }
    if let Some(ext) = ext {
        result.push('.');
        result.push_str(ext);
    }

    result
}

fn build_opts() -> Options {
    let mut opts = Options::new();
    opts.optopt(
        "t",
        "tag",
        "tag name to add to the resulting binaries file names",
        "TAG",
    );
    opts.optflag(
        "n",
        "no-default-options",
        "do not add default cargo options (--no-run, --message-format)",
    );
    opts.optflag("v", "verbose", "prints files copied");
    opts.optflag("h", "help", "print this help menu");
    opts
}

fn print_usage(opts: &Options, fail: Option<Fail>) -> ! {
    if let Some(fail) = fail {
        eprintln!("[ERROR]: {}", fail);
        eprintln!();
    }
    let brief = "usage: cargo export [OPTIONS] PATH -- CARGO_COMMAND [CARGO_OPTIONS...]";
    eprintln!("{}", opts.usage(brief));

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

/// Returns tuple of 3 strings: file_name (without hash and etensions), hash and extension) 2 are optional
///
/// Filenmaes generated by cargo have the following format: `<name>-<hash>.<extension>`. Extension is optional and
/// is only present for windows executables. Hash is optional and is only present for tests and benchmarks.
///
/// This method strips hash and extension from the file name and returns them as separate strings.
/// If hash or extension are not present, they are returned as `None`. At the moment only `.exe` extension
/// is supported.
fn split_file_name(input: &str) -> (&str, Option<&str>, Option<&str>) {
    const EXE_EXTENSION: &str = ".exe";
    const RUSTC_HASH_LENGTH: usize = 16;

    let (file_name, extension) = if let Some(name) = input.strip_suffix(EXE_EXTENSION) {
        (name, Some(&EXE_EXTENSION[1..]))
    } else {
        (input, None)
    };

    let idx = match file_name.rfind('-') {
        Some(idx) if idx > 0 => idx,
        _ => return (file_name, None, extension),
    };

    let hash = &file_name[idx + 1..];
    // it's safe to check number of bytes instead of chars here because we still
    // check all the characters individually
    if hash.len() == RUSTC_HASH_LENGTH && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        (&file_name[..idx], Some(hash), extension)
    } else {
        (file_name, None, extension)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn check_target_file_name() {
        let cases = vec![
            // Simple cases without a tag
            ("app-ebb8dd5b587f73a1", None, "app"),
            ("🥰-ebb8dd5b587f73a1", None, "🥰"),
            ("app-ebb8dd5b5", None, "app-ebb8dd5b5"),
            ("app-ebb8dd5b5000000", None, "app-ebb8dd5b5000000"),
            ("app-", None, "app-"),
            ("app", None, "app"),
            ("", None, ""),
            ("-ebb8dd5b587f73a1", None, "-ebb8dd5b587f73a1"),
            ("ebb8dd5b587f73a1", None, "ebb8dd5b587f73a1"),
            // cases with tag
            ("app-ebb8dd5b587f73a1", Some("tag"), "app-tag"),
            ("🥰-ebb8dd5b587f73a1", Some("tag"), "🥰-tag"),
            ("app-ebb8dd5b5", Some("tag"), "app-ebb8dd5b5-tag"),
            (
                "app-ebb8dd5b5000000",
                Some("tag"),
                "app-ebb8dd5b5000000-tag",
            ),
            ("app-", Some("tag"), "app--tag"),
            ("app", Some("tag"), "app-tag"),
            ("", Some("tag"), "-tag"),
            ("-ebb8dd5b587f73a1", Some("tag"), "-ebb8dd5b587f73a1-tag"),
            ("ebb8dd5b587f73a1", Some("tag"), "ebb8dd5b587f73a1-tag"),
            // cases for windows with .exe extension without a tag
            ("app-ebb8dd5b587f73a1.exe", None, "app.exe"),
            ("🥰-ebb8dd5b587f73a1.exe", None, "🥰.exe"),
            ("app-ebb8dd5b5.exe", None, "app-ebb8dd5b5.exe"),
            ("app-ebb8dd5b5000000.exe", None, "app-ebb8dd5b5000000.exe"),
            ("app-.exe", None, "app-.exe"),
            ("app.exe", None, "app.exe"),
            (".exe", None, ".exe"),
            ("-ebb8dd5b587f73a1.exe", None, "-ebb8dd5b587f73a1.exe"),
            ("ebb8dd5b587f73a1.exe", None, "ebb8dd5b587f73a1.exe"),
            // windows cases with a tag
            // TODO: should we add a tag to the extension?
            ("app-ebb8dd5b587f73a1.exe", Some("tag"), "app-tag.exe"),
            ("🥰-ebb8dd5b587f73a1.exe", Some("tag"), "🥰-tag.exe"),
            ("app-ebb8dd5b5.exe", Some("tag"), "app-ebb8dd5b5-tag.exe"),
            (
                "app-ebb8dd5b5000000.exe",
                Some("tag"),
                "app-ebb8dd5b5000000-tag.exe",
            ),
            ("app-.exe", Some("tag"), "app--tag.exe"),
            ("app.exe", Some("tag"), "app-tag.exe"),
            (".exe", Some("tag"), "-tag.exe"),
            (
                "-ebb8dd5b587f73a1.exe",
                Some("tag"),
                "-ebb8dd5b587f73a1-tag.exe",
            ),
            (
                "ebb8dd5b587f73a1.exe",
                Some("tag"),
                "ebb8dd5b587f73a1-tag.exe",
            ),
        ];

        for (input, tag, expected) in cases {
            assert_eq!(target_file_name(input, tag), expected);
        }
    }
}
