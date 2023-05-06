# Exporting cargo compiler artifacts (tests, benches)

## Motivation

Right now it's quite challenging to export secondary artifacts like tests or benchmark executables. Those kind of artifacts can be very valuable for different reasons%

1. packing test executables in the contaier to run them later on a different platform
2. comparing and analyzing assembly when performance benchmarking

For final artifacts we have `target/(release|debug)/{crate-name}`, but test and benchmark file names are containing hashes like `target/release/deps/app-25de5d28a523d3c2` which will change when compiling options are changed. For this reason simple methods like `find` and `cp` doesn't work well.

Thankfully compiler generating compiler messages in json format `--message-format=json` which allows to list all compiler generated artifacts/

## Using `cargo-export`

```console
$ cargo export -o ./target/tests -- test
```

Under the hood this command will run `cargo test --no-run --message-format=json` and copy all the generated binaries in the `target/tests` dirctory.
