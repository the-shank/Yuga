# Yuga: Automatically Detecting Lifetime Annotation Bugs in the Rust Language

**[NEW]** Try out our (WIP) [web demo](https://yuga-rust-1ec69edea65b.herokuapp.com) of Yuga!

Yuga is a tool to detect lifetime annotation bugs in Rust [[ArXiv](https://arxiv.org/pdf/2310.08507.pdf)]. It is adapted from a fork of [Rudra](https://github.com/sslab-gatech/Yuga).

To setup the code, clone the repository, `cd` into it, and run the following commands (tested on Mac and Ubuntu):
```
./setup_yuga_runner_home ./yuga_home
./install-debug.sh
```
If you face errors running the above commands, please refer to the instructions in the main Yuga repository for installing Yuga in debug mode.

Our tool can now be run using the `cargo-yuga` subcommand. For any Rust package that we want to analyze, run the following command from within the package folder:
```
cargo yuga
```
This will print the reported vulnerabilities, if any, to `stdout`.

Here is a list of bugs in public Rust projects detected by Yuga so far:

|       Project       |                           Issue/PR                            | Public/Private API |          Status              |
|---------------------|---------------------------------------------------------------|--------------------|------------------------------|
| alsa                | https://github.com/diwic/alsa-rs/issues/117                   |       Public       |   Unconfirmed                |           
| bv                  | https://github.com/tov/bv-rs/issues/16                        |       Public       |   Confirmed with Miri        |
| pulse-binding-rust  | https://github.com/jnqnfe/pulse-binding-rust/issues/53        |       Public       |   Confirmed with Valgrind    |
| json-rust / jzon-rs | https://github.com/maciejhirsz/json-rust/pull/209             |       Public       |   Confirmed by dev           |
| cslice              | https://github.com/dherman/cslice/issues/5                    |       Public       |   Confirmed with Miri        |
| sled                | https://github.com/spacejam/sled/issues/1442                  |       Private      |   Confirmed by dev           |
| tokio               | https://github.com/tokio-rs/tokio/issues/5113                 |       Private      |   Unconfirmed                |
