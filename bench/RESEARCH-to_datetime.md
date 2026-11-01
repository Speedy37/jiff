# `Offset::to_datetime` performance research

Experiments on top of [#640](https://github.com/BurntSushi/jiff/pull/640)
(branch-free time handling) and Ben Joffe's randomized conversion benchmarks.
Everything here is selected with `--cfg` flags so variants can be compared from
one tree. It is research code, not a proposed change as-is.

Background reading:

- Fast time-of-day: <https://www.benjoffe.com/fast-time-of-day>
- Andrew Consroe, one multiply for two divisions: <https://aconz2.github.io/2026/09/09/muldiv.html>
- Fast 64-bit date algorithm: <https://www.benjoffe.com/fast-date-64>
- Fast floor division: <https://www.benjoffe.com/fast-floordiv>

## Result

Best variant: Ben Joffe's fast 64-bit date algorithm in `UnixEpochDay::to_date`,
Consroe's single-multiply hour/minute in `TimeSecond::to_time`, `÷ 86400` as one
`imul`, and no range checks in `to_datetime` (keeping them costs about 2%, see
Findings):

```sh
RUSTFLAGS='--cfg jiff_date="fast128" --cfg jiff_time="consroe"' cargo bench -- conversion_random/
```

One session, AMD Ryzen 9 5950X (~4.8 GHz), pinned to one core, ns per conversion:

| input         | throughput PR #640 | throughput best | latency PR #640 | latency best |
|---------------|-------------------:|----------------:|----------------:|-------------:|
| uniform       |               5.46 |            4.72 |           10.48 |         9.43 |
| secs-zero     |               5.93 |            4.68 |           10.25 |         9.45 |
| zero-min-secs |               5.79 |            4.67 |           10.33 |         9.35 |
| all-same      |               5.77 |            4.64 |           10.07 |         9.57 |
| split         |               5.84 |            4.70 |           10.31 |         9.47 |
| subsec1970    |               5.90 |            4.66 |           10.15 |         9.37 |
| subsec2000    |               5.81 |            4.74 |           10.25 |         9.38 |

Best is 14–21% faster than #640 in throughput and 5–10% in latency. In the
original per-element loop (`/jiff`) the gain is 2–7%. Against current `master`
the gain is much smaller on typical inputs (see "Against current `master`").

## Benchmark loops

`bench/src/timestamp.rs`, `timestamp/to_civil_datetime_offset_conversion_random/<kind>/…`:

- `jiff`: the original loop. `bb(offset)` and `bb(dt)` per element go through
  the same stack slot, which serializes conversions. Equivalent builds measured
  up to 25% apart with it, so it is unreliable for comparing variants.
- `jiff-throughput`: results written into a `Vec`, one `bb` per pass.
  Reproduces within about 1% across reruns.
- `jiff-latency`: each result selects the next input's index through a zero
  hidden by `bb`, so conversions form a serial chain.

## `--cfg` switches (all in `jiff-core`)

| flag | effect |
|------|--------|
| `jiff_date="neri"` | `UnixEpochDay::to_date`: Neri–Schneider (the original) |
| `jiff_date="fast128"` | fast-date-64 as published (64×64→128 multiplies) |
| `jiff_date="fast128arm"` | fast-date-64's ARM64 variant (`SCALE = 1`): 16-bit year-part constants, January/February fixed from the month. Not benchmarked: no ARM64 machine was available |
| *(no `jiff_date`)* | fast-date-64 with a 32-era shift: every product fits `u64`, every constant 31 bits (`imul` with immediates) |
| `jiff_time="consroe"` | `TimeSecond::to_time`: one 64×64→128 multiply yields hour and the low bits of total minutes |
| *(no `jiff_time`)* | `TimeSecond::to_time`: two `imul`s (#640's V3) |
| `jiff_checked` | unfused `to_datetime` keeps the original checked constructors |
| `jiff_div128` | `pos_sec / 86400` as before (128-bit `mul`) instead of `div_86400` |
| `jiff_fused` | `to_datetime` as one fused computation on `pos_sec` (see below) |

With `jiff_fused`, `jiff_date` selects `fast64` or the default 128-bit date,
and `jiff_time` selects `v3`, `direct` (h/m/s straight from `pos_sec` using
`60 + 4 = 64` and `24 + 8 = 32`), or the default Consroe multiply.

PR #640 exactly is
`--cfg jiff_date="neri" --cfg jiff_time="v3" --cfg jiff_checked --cfg jiff_div128`.

## Benched variants

Every variant measured during this research, with the flags that rebuild it.
Run one like this (each flag set gets its own build in `target/`, so
alternating between variants doesn't recompile):

```sh
cd bench
RUSTFLAGS='--cfg jiff_date="fast128" --cfg jiff_time="consroe"' \
  taskset -c 2 cargo bench -- 'conversion_random/(uniform|split|subsec1970)/jiff-'
```

Results are `uniform` in ns per conversion, throughput / latency loops.
Compare only within a session: the same build drifted up to about 4% between
sessions. Sessions A–C ran before `div_86400` existed, so their variants
include `--cfg jiff_div128` to reproduce what was measured.

| variant | `RUSTFLAGS` | session | throughput | latency |
|---------|-------------|:-------:|-----------:|--------:|
| PR #640 | `--cfg jiff_date="neri" --cfg jiff_time="v3" --cfg jiff_checked --cfg jiff_div128` | A | 6.28 | 10.16 |
| neri + v3, unchecked | `--cfg jiff_date="neri" --cfg jiff_time="v3" --cfg jiff_div128` | A | 6.48 | 10.24 |
| neri + consroe | `--cfg jiff_date="neri" --cfg jiff_time="consroe" --cfg jiff_div128` | A | 5.04 | 9.82 |
| fast128 + v3 | `--cfg jiff_date="fast128" --cfg jiff_time="v3" --cfg jiff_div128` | A | 4.87 | 9.79 |
| fast128 + consroe | `--cfg jiff_date="fast128" --cfg jiff_time="consroe" --cfg jiff_div128` | A | 4.76 | 9.67 |
| fast64 + v3 | `--cfg jiff_div128` | A | 5.42 | 9.74 |
| fast64 + consroe | `--cfg jiff_time="consroe" --cfg jiff_div128` | A | 5.07 | 9.43 |
| fused fast128 + consroe | `--cfg jiff_fused --cfg jiff_div128` | B | 5.21 | 8.91 |
| fused fast64 + consroe | `--cfg jiff_fused --cfg jiff_date="fast64" --cfg jiff_div128` | B | 5.29 | 8.94 |
| fused fast128 + v3 | `--cfg jiff_fused --cfg jiff_time="v3" --cfg jiff_div128` | B | 5.57 | 8.82 |
| fused fast64 + v3 | `--cfg jiff_fused --cfg jiff_date="fast64" --cfg jiff_time="v3" --cfg jiff_div128` | B | 5.56 | 8.94 |
| fast128 + consroe, old ÷ 86400 | `--cfg jiff_date="fast128" --cfg jiff_time="consroe" --cfg jiff_div128` | C | 4.79 | 10.01 |
| fast128 + consroe, `div_86400` | `--cfg jiff_date="fast128" --cfg jiff_time="consroe"` | C | 4.65 | 9.17 |
| fused, old ÷ 86400 | `--cfg jiff_fused --cfg jiff_div128` | C | 5.19 | 9.04 |
| fused, `div_86400` | `--cfg jiff_fused` | C | 5.27 | 8.77 |
| fused fast128 + direct | `--cfg jiff_fused --cfg jiff_time="direct"` | D | 5.03 | 9.23 |
| fused fast64 + direct | `--cfg jiff_fused --cfg jiff_date="fast64" --cfg jiff_time="direct"` | D | 5.27 | 8.34 |
| PR #640 | `--cfg jiff_date="neri" --cfg jiff_time="v3" --cfg jiff_checked --cfg jiff_div128` | E | 5.46 | 10.48 |
| **best** | `--cfg jiff_date="fast128" --cfg jiff_time="consroe"` | E | **4.72** | **9.43** |
| best, unchecked | `--cfg jiff_date="fast128" --cfg jiff_time="consroe"` | F | 4.67 | 9.39 |
| best, checked | `--cfg jiff_date="fast128" --cfg jiff_time="consroe" --cfg jiff_checked` | F | 4.76 | 9.63 |
| `master` (`9ea4c2b`) | none: build upstream `master` with this branch's `bench/src/timestamp.rs` | G | 4.82 | 9.74 |
| PR #640 | `--cfg jiff_date="neri" --cfg jiff_time="v3" --cfg jiff_checked --cfg jiff_div128` | G | 5.68 | 10.02 |
| best | `--cfg jiff_date="fast128" --cfg jiff_time="consroe"` | G | 4.64 | 9.39 |
| `master` (`9ea4c2b`) | none: build upstream `master` with this branch's `bench/src/timestamp.rs` | H | 4.94 | 9.89 |
| `to-datetime-fast` (best without `div_86400`) | equivalent: `--cfg jiff_date="fast128" --cfg jiff_time="consroe" --cfg jiff_div128` | H | 5.00 | 9.56 |

Session D was noisier than the others (about ±8% between input kinds of the
same build), so its fused rows are only roughly comparable with C.

## Against current `master`

Session G: upstream `master` (`9ea4c2b`, without #640) built in a separate
worktree with this branch's `bench/src/timestamp.rs`, then PR #640 and best
from this branch, one after another on the same core. Each cell is µs per
run, as Criterion reports it (one run converts 65,536 timestamps), with ns
per conversion in parentheses:

| input | throughput master | PR #640 | best | best vs master | latency master | PR #640 | best | best vs master |
|-------|------:|------:|------:|------:|------:|------:|------:|------:|
| uniform | 316 (4.82) | 372 (5.68) | 304 (4.64) | -4% | 638 (9.74) | 657 (10.02) | 616 (9.39) | -4% |
| secs-zero | 319 (4.86) | 374 (5.71) | 309 (4.72) | -3% | 649 (9.91) | 657 (10.03) | 611 (9.32) | -6% |
| zero-min-secs | 285 (4.35) | 373 (5.69) | 304 (4.63) | +7% | 621 (9.48) | 658 (10.04) | 610 (9.30) | -2% |
| all-same | 316 (4.82) | 375 (5.73) | 305 (4.65) | -3% | 645 (9.84) | 658 (10.04) | 619 (9.44) | -4% |
| split | 477 (7.28) | 376 (5.74) | 304 (4.64) | -36% | 801 (12.22) | 661 (10.09) | 599 (9.13) | -25% |
| subsec1970 | 374 (5.71) | 380 (5.80) | 303 (4.62) | -19% | 743 (11.34) | 672 (10.26) | 623 (9.51) | -16% |
| subsec2000 | 315 (4.80) | 374 (5.71) | 300 (4.58) | -4% | 643 (9.81) | 656 (10.01) | 599 (9.14) | -7% |

To reproduce, from the repository root on this branch:

```sh
# master: a separate worktree with this branch's benchmark file
git worktree add ../jiff-master 9ea4c2b
git show to-datetime-research:bench/src/timestamp.rs \
  > ../jiff-master/bench/src/timestamp.rs
(cd ../jiff-master/bench && taskset -c 2 cargo bench -- 'conversion_random/')

# PR #640 and best, from this branch
cd bench
RUSTFLAGS='--cfg jiff_date="neri" --cfg jiff_time="v3" --cfg jiff_checked --cfg jiff_div128' \
  taskset -c 2 cargo bench -- 'conversion_random/'
RUSTFLAGS='--cfg jiff_date="fast128" --cfg jiff_time="consroe"' \
  taskset -c 2 cargo bench -- 'conversion_random/'

# afterwards
git worktree remove --force ../jiff-master
```

Throughput and latency are the `…/jiff-throughput` and `…/jiff-latency`
results; `taskset -c 2` pins to one core (any idle core will do).

- #640 on its own is slower than `master` in throughput on typical inputs
  (372 vs 316 µs per run, 5.68 vs 4.82 ns per conversion, on `uniform`) and about even in latency. It only wins where
  `master`'s branches mispredict: `split` and `subsec1970`.
- Best against `master`: 3–4% faster in throughput and 2–7% in latency on
  typical inputs, 36% / 25% faster on `split` and 19% / 16% on `subsec1970`,
  but 7% slower in throughput on `zero-min-secs`, where `master`'s
  on-the-hour early exit in `to_time` is perfectly predicted.
- So most of the "best vs #640" gain recovers what #640's branch-free code
  cost on typical inputs; the gain over `master` is small except on inputs
  that defeat `master`'s branches.

## Clean branch (`to-datetime-fast`) against `master`

`to-datetime-fast` is best without `div_86400`: one commit on `master` with
fast-date-64, Consroe's `to_time`, the branch-free nanosecond fix-up and no
range checks, but the plain `pos_sec / 86_400`. On x86-64 it is a branch-free
sequence of 66 instructions (7 `mul`, 1 `imul`). On this branch the same code
is `--cfg jiff_date="fast128" --cfg jiff_time="consroe" --cfg jiff_div128`
(identical instruction sequence, except the second of day is computed with a
32-bit `imul`/`add` instead of a 64-bit `imul`/`sub`).

Session H: both built in separate worktrees with this branch's
`bench/src/timestamp.rs`, one after another on the same core. µs per run
(ns per conversion):

| input | throughput master | to-datetime-fast | vs master | latency master | to-datetime-fast | vs master |
|-------|------:|------:|------:|------:|------:|------:|
| uniform | 324 (4.94) | 327 (5.00) | +1% | 648 (9.89) | 626 (9.56) | -3% |
| secs-zero | 331 (5.05) | 318 (4.86) | -4% | 656 (10.01) | 630 (9.61) | -4% |
| zero-min-secs | 324 (4.95) | 322 (4.91) | -1% | 613 (9.36) | 643 (9.80) | +5% |
| all-same | 324 (4.94) | 316 (4.83) | -2% | 658 (10.05) | 631 (9.62) | -4% |
| split | 491 (7.49) | 321 (4.89) | -35% | 807 (12.32) | 645 (9.84) | -20% |
| subsec1970 | 367 (5.60) | 315 (4.81) | -14% | 752 (11.47) | 623 (9.51) | -17% |
| subsec2000 | 324 (4.95) | 319 (4.87) | -2% | 663 (10.12) | 624 (9.53) | -6% |

To reproduce, from the repository root on this branch:

```sh
for b in master to-datetime-fast; do
  git worktree add --detach ../jiff-$b $b
  git show to-datetime-research:bench/src/timestamp.rs \
    > ../jiff-$b/bench/src/timestamp.rs
  (cd ../jiff-$b/bench && taskset -c 2 cargo bench -- 'conversion_random/')
  git worktree remove --force ../jiff-$b
done
```

- Typical inputs: about even with `master` in throughput (−4% to +1%) and
  3–6% better in latency.
- Inputs that defeat `master`'s branches: 35% / 20% faster on `split` and
  14% / 17% on `subsec1970`.
- `zero-min-secs`: about even in throughput this time (session G had
  `master` 7% ahead there, so that case is noisy) and 5% slower in latency.
- Dropping `div_86400` costs a few percent on typical inputs (compare
  session G's best: 3–4% ahead of `master` in throughput).

## Findings

- Throughput is ALU-bound (about 80 ALU uops per conversion, 4 ALUs), so
  instruction count matters more than the number of multiplies. On this CPU
  `imul r64` is 1/cycle with latency 3 and the 128-bit `mul` about 1.25 cycles
  with latency 3 (low) or 4 (high), so 128-bit multiplies are nearly free.
- fast-date-64's ARM64 variant (`fast128arm`) compiles on `aarch64` (with
  Consroe time) to 80 instructions against 82 for the x86 form: only 2 fewer
  `mov`/`movk` constant loads, same 8 multiplies. Unmeasured on ARM64.
- fast-date-64 beats Neri–Schneider on throughput; the 32-era `imul`-only form
  does not beat the published 128-bit form.
- Consroe's time-of-day beats #640's two-`imul` V3 with every date algorithm.
- `div_86400` (`(pos_sec >> 7) * ceil(2^41 / 675) >> 41`, exact because
  `pos_sec < 2^40`) gains about 2% throughput and 5% latency, unfused.
- Fused `to_datetime` has the best latency (8.7–9.1 ns) but 10% worse
  throughput than unfused; no fused variant (incl. `direct`) closed that gap.
- LLVM inlines the unfused best variant and folds `DAY_SHIFT` into the date
  constant on its own, with or without the range checks. Without them the
  function is one branch-free sequence: 65 instructions, 48 of them on the
  dependency path with 7 multiplies (PR #640: 59 and 11).
- The range checks cost the best variant little. Keeping them
  (`--cfg jiff_checked`) adds three predictable compare-and-branch pairs and
  about 16 hot-path instructions, and measures about 2% slower in throughput
  (4.69–4.83 vs 4.64–4.70 ns) and latency (9.58–9.64 vs 9.39–9.45 ns). Only the
  per-element `/jiff` loop shows a large gap (6.54–6.72 vs 5.34–5.41 ns, +22%),
  and that loop is sensitive to code layout.

## Correctness

- `unix_epoch_day_to_date_exhaustive`: every valid `UnixEpochDay` against
  Neri–Schneider (whichever `jiff_date` is selected).
- `unix_epoch_day_to_date_variants_exhaustive`: every date variant
  (`fast128`, `fast128_arm`, `fast64`) against Neri–Schneider on every valid
  day, regardless of `jiff_date`, so the ARM64 variant is verified on any host.
- `div_86400_exact_over_reachable_range`: both sides of every day boundary in
  the reachable `pos_sec` range.
- `to_datetime_fused_matches_unfused`: extremes, every second around midnight
  for hundreds of days and offsets, and 2M random timestamps/offsets.
- `TimeSecond::to_time` variants are exact for all `0..128*3600` (checked
  exhaustively outside the test suite).
