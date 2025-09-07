# Hashed sorting benchmark

Benchmarks for the following problem statement: you have a large array of mostly-unique uint64s, and you want to know how many unique values there are.

We try many hash table solutions and many sorting solutions. See [the main blog post](http://reiner.org/hashed-sorting) for discussion.

## Running benchmarks

Install a recent nightly version of Rust using [rustup](https://rustup.rs/). Then

```
./run.sh
```
