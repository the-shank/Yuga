# Yuga: Automatically Detecting Lifetime Annotation Bugs in the Rust Language

This is an anonymized code repository for our PLDI 2023 submission. It is adapted from a fork of [Rudra](https://github.com/sslab-gatech/Rudra).

To setup the code, clone the repository, `cd` into it, and run the following commands (tested on Mac and Ubuntu):
```
./setup_rudra_runner_home ./rudra_home
./install-debug.sh
```
If you face errors running the above commands, please refer to the instructions in the main Rudra repository for installing Rudra in debug mode.

Our tool can now be run using the `cargo-rudra` subcommand. For any Rust package that we want to analyze, run the following command from within the package folder:
```
cargo rudra
```
This will print the reported vulnerabilities, if any, to `stdout`.
