use cargo_export::target_file_name;
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
    let mut args = std::env::args().collect::<VecDeque<_>>();
    // skipping program name in arguments list

    args.pop_front();
    // skipping subcommand name if it was called as `cargo export`
    if let Some("export") = args.front().map(String::as_str) {
        args.pop_front();
    }
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
        Err(f) => print_usage_and_exit(&opts, Some(f)),
    };

    if matches.opt_present("help") {
        print_usage_and_exit(&opts, None);
    }
    if matches.opt_present("version") {
        print_version_and_exit();
    }

    let Some(target) = matches.free.first() else {
        print_usage_and_exit(&opts, Some(Fail::OptionMissing("PATH".to_string())));
    };

    let Some(cargo_cmd) = cargo_args.first() else {
        print_usage_and_exit(
            &opts,
            Some(Fail::OptionMissing("CARGO_COMMAND".to_string())),
        );
    };
    if !matches.opt_present("no-default-options") {
        if *cargo_cmd == "bench" || *cargo_cmd == "test" {
            cargo_args.insert(1, "--no-run");
        }
        // inserting options right after cargo subcommand
        cargo_args.insert(1, "--message-format=json");
    }

    let tag_name = matches.opt_str("tag");
    let dry_run = matches.opt_present("dry-run");
    let verbose = matches.opt_present("verbose") || dry_run;

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

    let target_dir = PathBuf::from(target);
    if !dry_run && !target_dir.exists() {
        fs::create_dir_all(&target_dir).unwrap();
    }

    // Copying artifacts
    for artfact in artifacts {
        let from = PathBuf::from(&artfact.executable);
        let file_name = from.file_name().and_then(|n| n.to_str()).unwrap();
        let file_name = target_file_name(file_name, tag_name.as_deref());
        let to = target_dir.join(&file_name);

        if verbose {
            eprintln!(
                "[cargo-export] copying '{}' to '{}'{}",
                from.display(),
                to.display(),
                if dry_run { " (dry run)" } else { "" }
            );
        }
        if !dry_run {
            fs::copy(from, to).expect("Unable to copy file");
        }
    }
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
    opts.optflag("d", "dry-run", "do not copy any files (implies --verbose)");
    opts.optflag("h", "help", "print help");
    opts.optflag("V", "version", "print version");
    opts
}

fn print_version_and_exit() -> ! {
    println!("cargo-export {}", env!("CARGO_PKG_VERSION"));
    exit(0)
}

fn print_usage_and_exit(opts: &Options, fail: Option<Fail>) -> ! {
    if let Some(fail) = &fail {
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
    let exit_code = if fail.is_some() { 1 } else { 0 };
    exit(exit_code)
}
