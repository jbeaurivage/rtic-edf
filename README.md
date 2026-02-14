# `rtic-edf`

An EDF scheduler for modular RTIC (RTIC-eVo)

# Requirements

- A recent version of the Rust compiler
- (optional) `probe-rs` tools to flash benchmarks to hardware

The core scheduler implementation is located at `rtic/edf-pass/src/scheduler/`.

# Examples/benchmarks quickstart

## Requirements

- An ATSAMD51 board (for example, Adafruit Metro M4)
- An RTT-capable probe, ideally supported by `probe-rs`

To run benchmarks and examples, `cd` into the `benchmarks` directory. From there,

```sh
DEFMT_LOG=<log-level> cargo r --profile release --bin <benchmark>
```

Where `<log-level>` is one of: `trace, debug, info, warn` or `error`, and `<benchmark>` is one of `bench_oh, benchmark`, or `hello`.