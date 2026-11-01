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

Best is 14–21% faster in throughput and 5–10% in latency. In the original
per-element loop (`/jiff`) the gain is 2–7%.

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

Session D was noisier than the others (about ±8% between input kinds of the
same build), so its fused rows are only roughly comparable with C.

## Findings

- Throughput is ALU-bound (about 80 ALU uops per conversion, 4 ALUs), so
  instruction count matters more than the number of multiplies. On this CPU
  `imul r64` is 1/cycle with latency 3 and the 128-bit `mul` about 1.25 cycles
  with latency 3 (low) or 4 (high), so 128-bit multiplies are nearly free.
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
- `div_86400_exact_over_reachable_range`: both sides of every day boundary in
  the reachable `pos_sec` range.
- `to_datetime_fused_matches_unfused`: extremes, every second around midnight
  for hundreds of days and offsets, and 2M random timestamps/offsets.
- `TimeSecond::to_time` variants are exact for all `0..128*3600` (checked
  exhaustively outside the test suite).
