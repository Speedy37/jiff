use std::hint::black_box as bb;

use criterion::Criterion;

use jiff::{
    civil,
    tz::{Offset, TimeZone},
    SignedDuration, Timestamp, ToSpan,
};

use crate::{benchmark, convert::ConvertFrom};

pub(super) fn define(c: &mut Criterion) {
    add_time_secs(c);
    sub_time_secs(c);
    add_time_subsec(c);
    from_seconds(c);
    every_hour_in_week(c);
    to_civil_datetime_offset_conversion(c);
    to_civil_datetime_offset_conversion_random(c);
    to_civil_datetime_offset_holistic(c);
    to_civil_datetime_static(c);
}

/// Measures how long it takes to add 86400 seconds to a timestamp.
fn add_time_secs(c: &mut Criterion) {
    const NAME: &str = "timestamp/add_time_secs";
    const START: Timestamp = Timestamp::constant(1719755160, 0);
    const EXPECTED: Timestamp = Timestamp::constant(1719755160 + 86400, 0);

    {
        let span = jiff::Span::new().seconds(86400);
        benchmark(c, format!("{NAME}/span/jiff"), |b| {
            b.iter(|| {
                let end = bb(&START).checked_add(bb(span)).unwrap();
                assert_eq!(end, EXPECTED);
            })
        });

        let duration = jiff::SignedDuration::from_secs(86400);
        benchmark(c, format!("{NAME}/duration/jiff"), |b| {
            b.iter(|| {
                let end = bb(&START).checked_add(bb(duration)).unwrap();
                assert_eq!(end, EXPECTED);
            })
        });
    }

    {
        let expected = chrono::DateTime::convert_from(EXPECTED);
        let start = chrono::DateTime::convert_from(START);
        let delta = chrono::TimeDelta::new(86400, 0).unwrap();
        benchmark(c, format!("{NAME}/duration/chrono"), |b| {
            b.iter(|| {
                let end = bb(start).checked_add_signed(bb(delta)).unwrap();
                assert_eq!(end, expected);
            })
        });
    }

    {
        let expected = time::OffsetDateTime::convert_from(EXPECTED);
        let start = time::OffsetDateTime::convert_from(START);
        let duration = time::Duration::new(86400, 0);
        benchmark(c, format!("{NAME}/duration/time"), |b| {
            b.iter(|| {
                let end = bb(start).checked_add(bb(duration)).unwrap();
                assert_eq!(end, expected);
            })
        });
    }
}

/// Measures how long it takes to add 86400 seconds to a timestamp.
fn sub_time_secs(c: &mut Criterion) {
    const NAME: &str = "timestamp/sub_time_secs";
    const START: Timestamp = Timestamp::constant(1719755160, 0);
    const EXPECTED: Timestamp = Timestamp::constant(1719755160 - 86400, 0);

    {
        let span = jiff::Span::new().seconds(86400);
        benchmark(c, format!("{NAME}/span/jiff"), |b| {
            b.iter(|| {
                let end = bb(&START).checked_sub(bb(span)).unwrap();
                assert_eq!(end, EXPECTED);
            })
        });

        let duration = jiff::SignedDuration::from_secs(86400);
        benchmark(c, format!("{NAME}/duration/jiff"), |b| {
            b.iter(|| {
                let end = bb(&START).checked_sub(bb(duration)).unwrap();
                assert_eq!(end, EXPECTED);
            })
        });
    }

    {
        let expected = chrono::DateTime::convert_from(EXPECTED);
        let start = chrono::DateTime::convert_from(START);
        let delta = chrono::TimeDelta::new(86400, 0).unwrap();
        benchmark(c, format!("{NAME}/duration/chrono"), |b| {
            b.iter(|| {
                let end = bb(start).checked_sub_signed(bb(delta)).unwrap();
                assert_eq!(end, expected);
            })
        });
    }

    {
        let expected = time::OffsetDateTime::convert_from(EXPECTED);
        let start = time::OffsetDateTime::convert_from(START);
        let duration = time::Duration::new(86400, 0);
        benchmark(c, format!("{NAME}/duration/time"), |b| {
            b.iter(|| {
                let end = bb(start).checked_sub(bb(duration)).unwrap();
                assert_eq!(end, expected);
            })
        });
    }
}

/// Measures how long it takes to add 2 hours 500 nanos to a timestamp.
fn add_time_subsec(c: &mut Criterion) {
    const NAME: &str = "timestamp/add_time_subsec";
    const START: Timestamp = Timestamp::constant(1719755160, 0);
    const EXPECTED: Timestamp =
        Timestamp::constant(1719755160 + (2 * 60 * 60), 500);

    {
        let span = jiff::Span::new().hours(2).nanoseconds(500);
        benchmark(c, format!("{NAME}/span/jiff"), |b| {
            b.iter(|| {
                let end = bb(&START).checked_add(bb(span)).unwrap();
                assert_eq!(end, EXPECTED);
            })
        });

        let duration = jiff::SignedDuration::new(2 * 60 * 60, 500);
        benchmark(c, format!("{NAME}/duration/jiff"), |b| {
            b.iter(|| {
                let end = bb(&START).checked_add(bb(duration)).unwrap();
                assert_eq!(end, EXPECTED);
            })
        });
    }

    {
        let expected = chrono::DateTime::convert_from(EXPECTED);
        let start = chrono::DateTime::convert_from(START);
        let delta = chrono::TimeDelta::new(2 * 60 * 60, 500).unwrap();
        benchmark(c, format!("{NAME}/duration/chrono"), |b| {
            b.iter(|| {
                let end = bb(start).checked_add_signed(bb(delta)).unwrap();
                assert_eq!(end, expected);
            })
        });
    }

    {
        let expected = time::OffsetDateTime::convert_from(EXPECTED);
        let start = time::OffsetDateTime::convert_from(START);
        let duration = time::Duration::new(2 * 60 * 60, 500);
        benchmark(c, format!("{NAME}/duration/time"), |b| {
            b.iter(|| {
                let end = bb(start).checked_add(bb(duration)).unwrap();
                assert_eq!(end, expected);
            })
        });
    }
}

/// Measures how long it takes to build the library's canonical "timestamp"
/// type from an actual integer number of seconds.
fn from_seconds(c: &mut Criterion) {
    const NAME: &str = "timestamp/from_seconds";
    const SECONDS: i64 = 1719755160;
    const EXPECTED: Timestamp = Timestamp::constant(SECONDS, 0);

    {
        benchmark(c, format!("{NAME}/integer/jiff"), |b| {
            b.iter(|| {
                let got = Timestamp::from_second(bb(SECONDS)).unwrap();
                assert_eq!(got, EXPECTED);
            })
        });
    }

    {
        let expected = chrono::DateTime::convert_from(EXPECTED);
        benchmark(c, format!("{NAME}/integer/chrono"), |b| {
            b.iter(|| {
                let got =
                    chrono::DateTime::from_timestamp(bb(SECONDS), 0).unwrap();
                assert_eq!(got, expected);
            })
        });
    }

    {
        let expected = time::UtcDateTime::convert_from(EXPECTED);
        benchmark(c, format!("{NAME}/integer/time"), |b| {
            b.iter(|| {
                let got = time::UtcDateTime::from_unix_timestamp(bb(SECONDS))
                    .unwrap();
                assert_eq!(got, expected);
            })
        });
    }
}

/// Measures the time it takes to iterate over a series of hours for 1 week.
///
/// This is a regression benchmark for when the `Timestamp::series` API was
/// approximately 4 times slower than doing it manually. (At time of writing,
/// 2025-02-15, the `series` API is now just about as fast as doing it manually
/// in Jiff.)
fn every_hour_in_week(c: &mut Criterion) {
    const NAME: &str = "timestamp/every_hour_in_week";
    const START: civil::DateTime = civil::date(2025, 2, 16).at(0, 0, 0, 0);
    const EXPECTED: Timestamp = Timestamp::constant(1740265200, 0);
    let start = START.to_zoned(TimeZone::UTC).unwrap().timestamp();

    {
        benchmark(c, format!("{NAME}/series/jiff"), |b| {
            b.iter(|| {
                let last =
                    start.series(1.hour()).take(bb(168)).last().unwrap();
                assert_eq!(last, EXPECTED);
            })
        });
        benchmark(c, format!("{NAME}/byhand/jiff"), |b| {
            b.iter(|| {
                let mut last = start;
                for _ in 0..bb(167) {
                    last += SignedDuration::from_hours(1);
                }
                assert_eq!(last, EXPECTED);
            })
        });
    }

    {
        let expected = chrono::DateTime::convert_from(EXPECTED);
        let start = chrono::DateTime::convert_from(start);
        benchmark(c, format!("{NAME}/byhand/chrono"), |b| {
            b.iter(|| {
                let mut last = start;
                for _ in 0..bb(167) {
                    last += chrono::TimeDelta::hours(1);
                }
                assert_eq!(last, expected);
            })
        });
    }

    {
        let expected = time::OffsetDateTime::convert_from(EXPECTED);
        let start = time::OffsetDateTime::convert_from(start);
        benchmark(c, format!("{NAME}/byhand/time"), |b| {
            b.iter(|| {
                let mut last = start;
                for _ in 0..bb(167) {
                    last += time::Duration::hours(1);
                }
                assert_eq!(last, expected);
            })
        });
    }
}

/// Measures the time to convert a timestamp to a civil datetime for a fixed
/// offset.
///
/// This operation might be commonly used in time zones for
/// which there is no DST. This is also generally a component of
/// `timestamp_to_civil_datetime_static`, since the first step there is to
/// *find* the offset and then convert it to a datetime. This benchmarks that
/// second step.
///
/// Also, the `time` crate can't do time zone name lookups, so we also include
/// this benchmark to incorporate it into some measurements for a similar task.
fn to_civil_datetime_offset_conversion(c: &mut Criterion) {
    const NAME: &str = "timestamp/to_civil_datetime_offset_conversion";
    const OFFSET: Offset = Offset::constant(-4);
    const STAMP: Timestamp = Timestamp::constant(1719755160, 0);
    const EXPECTED: civil::DateTime = civil::date(2024, 6, 30).at(9, 46, 0, 0);

    {
        benchmark(c, format!("{NAME}/jiff"), |b| {
            b.iter(|| {
                let dt = bb(OFFSET).to_datetime(bb(STAMP));
                assert_eq!(dt, EXPECTED);
            })
        });
    }

    {
        use chrono::TimeZone;

        let expected = chrono::NaiveDateTime::convert_from(EXPECTED);
        let stamp = STAMP.as_second();
        let offset = chrono::FixedOffset::convert_from(OFFSET);
        benchmark(c, format!("{NAME}/chrono"), |b| {
            b.iter(|| {
                let zdt = (&bb(offset))
                    .timestamp_opt(bb(stamp), 0)
                    .single()
                    .unwrap();
                assert_eq!(zdt.naive_local(), expected);
            })
        });
    }

    {
        let expected = time::PrimitiveDateTime::convert_from(EXPECTED);
        let stamp = STAMP.as_second();
        let offset = time::UtcOffset::convert_from(OFFSET);
        benchmark(c, format!("{NAME}/time"), |b| {
            b.iter(|| {
                let zdt = time::OffsetDateTime::from_unix_timestamp(stamp)
                    .unwrap()
                    .to_offset(offset);
                let dt = time::PrimitiveDateTime::new(zdt.date(), zdt.time());
                assert_eq!(dt, expected);
            })
        });
    }
}

/// Like `to_civil_datetime_offset_conversion`, but over a batch of random
/// timestamps within 30 years of 1970, in a few different shapes.
fn to_civil_datetime_offset_conversion_random(c: &mut Criterion) {
    const OFFSET: Offset = Offset::constant(-4);
    const COUNT: usize = 65536;
    const NAME: &str = "timestamp/to_civil_datetime_offset_conversion_random";

    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/uniform"),
        OFFSET,
        COUNT,
        RandomKind::Uniform,
    );
    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/secs-zero"),
        OFFSET,
        COUNT,
        RandomKind::SecondsZero,
    );
    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/zero-min-secs"),
        OFFSET,
        COUNT,
        RandomKind::ZeroMinuteSecond,
    );
    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/all-same"),
        OFFSET,
        COUNT,
        RandomKind::AllSame,
    );
    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/split"),
        OFFSET,
        COUNT,
        RandomKind::Split,
    );
    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/subsec1970"),
        OFFSET,
        COUNT,
        RandomKind::Subsec1970,
    );
    random_offset_conversion_benchmark(
        c,
        format!("{NAME}/subsec2000"),
        OFFSET,
        COUNT,
        RandomKind::Subsec2000,
    );
}

fn random_offset_conversion_benchmark(
    c: &mut Criterion,
    name: String,
    offset: Offset,
    count: usize,
    kind: RandomKind,
) {
    let timestamps = random_timestamps(count, kind);

    {
        benchmark(c, format!("{name}/jiff"), |b| {
            b.iter(|| {
                for &stamp in bb(&timestamps) {
                    let dt = bb(offset).to_datetime(stamp);
                    bb(dt);
                }
            })
        });
    }
}

/// Nanosecond component is always 0.
#[derive(Clone, Copy)]
enum RandomKind {
    /// Uniformly random.
    Uniform,
    /// Uniformly random, second component always 0.
    SecondsZero,
    /// Random date and hour, minute and second always 0.
    ZeroMinuteSecond,
    /// Always `1719755160` (2024-06-30T09:46:00Z).
    AllSame,
    /// Half `Uniform`, half `ZeroMinuteSecond`, shuffled together.
    Split,
    /// Uniformly random around 1970, with a millisecond-granularity subsec.
    Subsec1970,
    /// Uniformly random around 2000, with a millisecond-granularity subsec.
    Subsec2000,
}

/// Random timestamps within 30 years of 1970 (or 2000, for the `Subsec2000`
/// kind), shaped according to `kind`.
fn random_timestamps(count: usize, kind: RandomKind) -> Vec<Timestamp> {
    let bound = |year| {
        civil::date(year, 1, 1)
            .at(0, 0, 0, 0)
            .to_zoned(TimeZone::UTC)
            .unwrap()
            .timestamp()
            .as_second()
    };
    let center = if let RandomKind::Subsec2000 = kind { 2000 } else { 1970 };
    let start = bound(center - 30);
    let end = bound(center + 30);
    let span = (end - start) as u64;

    let mut state = kind as u64;

    let zero_minute_second = |secs: i64| {
        let dt = Offset::UTC.to_datetime(Timestamp::new(secs, 0).unwrap());
        let dt = dt.date().at(dt.hour(), 0, 0, 0);
        Offset::UTC.to_timestamp(dt).unwrap()
    };

    if let RandomKind::Split = kind {
        let half = count / 2;
        let mut timestamps: Vec<Timestamp> = Vec::with_capacity(count);
        for _ in 0..half {
            let secs = start + (splitmix64(&mut state) % span) as i64;
            timestamps.push(Timestamp::new(secs, 0).unwrap());
        }
        for _ in half..count {
            let secs = start + (splitmix64(&mut state) % span) as i64;
            timestamps.push(zero_minute_second(secs));
        }
        // Fisher-Yates shuffle.
        for i in (1..timestamps.len()).rev() {
            let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
            timestamps.swap(i, j);
        }
        return timestamps;
    }

    (0..count)
        .map(|_| {
            let secs = start + (splitmix64(&mut state) % span) as i64;
            match kind {
                RandomKind::Uniform => Timestamp::new(secs, 0).unwrap(),
                RandomKind::SecondsZero => {
                    let secs = secs - secs.rem_euclid(60);
                    Timestamp::new(secs, 0).unwrap()
                }
                RandomKind::ZeroMinuteSecond => zero_minute_second(secs),
                RandomKind::AllSame => Timestamp::constant(1719755160, 0),
                RandomKind::Subsec1970 | RandomKind::Subsec2000 => {
                    let ms = (splitmix64(&mut state) % 1000) as i32;
                    Timestamp::new(secs, ms * 1_000_000).unwrap()
                }
                RandomKind::Split => unreachable!(),
            }
        })
        .collect()
}

/// splitmix64 PRNG.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Like instant_to_civil_datetime_offset_only_conversion, but also includes
/// accessing the individual fields of the resulting civil datetime.
///
/// Usually converting a datetime from a timestamp is not all you do. You then
/// usually *do* something with the datetime, like format it or whatever.
/// Chrono makes accessing the fields on a datetime slower in favor of making
/// the initial conversion cheaper. This benchmark is meant to be more holistic
/// and measure the conversion and the access.
fn to_civil_datetime_offset_holistic(c: &mut Criterion) {
    const NAME: &str = "timestamp/to_civil_datetime_offset_holistic";
    const OFFSET: Offset = Offset::constant(-4);
    const STAMP: Timestamp = Timestamp::constant(1719755160, 0);

    benchmark(c, format!("{NAME}/jiff"), |b| {
        b.iter(|| {
            let dt = bb(OFFSET).to_datetime(bb(STAMP));
            assert_eq!(dt.year(), 2024);
            assert_eq!(dt.month(), 6);
            assert_eq!(dt.day(), 30);
            assert_eq!(dt.hour(), 9);
            assert_eq!(dt.minute(), 46);
            assert_eq!(dt.second(), 0);
            assert_eq!(dt.subsec_nanosecond(), 0);
        })
    });

    {
        use chrono::{Datelike, TimeZone, Timelike};

        let offset = chrono::FixedOffset::convert_from(OFFSET);
        let stamp = STAMP.as_second();
        benchmark(c, format!("chrono/{NAME}"), |b| {
            b.iter(|| {
                let zdt = (&bb(offset))
                    .timestamp_opt(bb(stamp), 0)
                    .single()
                    .unwrap();

                let dt = zdt.naive_local();
                assert_eq!(dt.year(), 2024);
                assert_eq!(dt.month(), 6);
                assert_eq!(dt.day(), 30);
                assert_eq!(dt.hour(), 9);
                assert_eq!(dt.minute(), 46);
                assert_eq!(dt.second(), 0);
                assert_eq!(dt.nanosecond(), 0);
            })
        });
    }

    {
        let offset = time::UtcOffset::convert_from(OFFSET);
        let stamp = STAMP.as_second();
        benchmark(c, format!("{NAME}/time"), |b| {
            b.iter(|| {
                let zdt = time::OffsetDateTime::from_unix_timestamp(stamp)
                    .unwrap()
                    .to_offset(offset);

                let dt = time::PrimitiveDateTime::new(zdt.date(), zdt.time());
                assert_eq!(dt.year(), 2024);
                assert_eq!(dt.month(), time::Month::June);
                assert_eq!(dt.day(), 30);
                assert_eq!(dt.hour(), 9);
                assert_eq!(dt.minute(), 46);
                assert_eq!(dt.second(), 0);
                assert_eq!(dt.nanosecond(), 0);
            })
        });
    }
}

/// Measures the time to convert a timestamp to a civil datetime.
///
/// This type of operation might be commonly used when converting a timestamp
/// to a human readable representation (i.e., what you might see on a clock) in
/// a specific time zone.
///
/// This specifically does not included searching for a time zone by name in
/// a tzdb. This assumes the time zone is already in hand.
fn to_civil_datetime_static(c: &mut Criterion) {
    const NAME: &str = "timestamp/to_civil_datetime_static";

    fn benchmark_with(c: &mut Criterion, tz_name: &str, timestamp: i64) {
        let bench_tz_name = tz_name.replace("/", "-").replace("_", "-");
        let timestamp = Timestamp::from_second(timestamp).unwrap();
        let expected = civil::date(2024, 6, 30).at(9, 46, 0, 0);

        if let Ok(tz) = jiff::tz::TimeZone::get(tz_name) {
            benchmark(
                c,
                format!("{NAME}/{bench_tz_name}/zoneinfo/jiff"),
                |b| {
                    b.iter(|| {
                        let dt = bb(&tz).to_datetime(bb(timestamp));
                        assert_eq!(dt, expected);
                    })
                },
            );
        }

        #[cfg(unix)]
        if let Ok(tz) = tzfile::Tz::named(tz_name) {
            let timestamp = timestamp.as_second();
            let expected = chrono::NaiveDateTime::convert_from(expected);
            benchmark(
                c,
                format!("{NAME}/{bench_tz_name}/zoneinfo/chrono"),
                |b| {
                    use chrono::TimeZone;

                    b.iter(|| {
                        let mapped = (bb(&tz)).timestamp_opt(bb(timestamp), 0);
                        let zdt = mapped.single().unwrap();
                        assert_eq!(zdt.naive_local(), expected);
                    })
                },
            );
        }

        {
            let tz = tz_name.parse::<chrono_tz::Tz>().unwrap();
            let timestamp = timestamp.as_second();
            let expected = chrono::NaiveDateTime::convert_from(expected);

            benchmark(
                c,
                format!("{NAME}/{bench_tz_name}/bundled/chrono"),
                |b| {
                    use chrono::TimeZone;

                    b.iter(|| {
                        let mapped = tz.timestamp_opt(bb(timestamp), 0);
                        let zdt = mapped.single().unwrap();
                        assert_eq!(zdt.naive_local(), expected);
                    })
                },
            );
        }

        // The `time` crate doesn't support this.
    }

    // Benchmarks a time zone with DST.
    benchmark_with(c, "America/New_York", 1719755160);
    // Benchmarks a time zone without DST. Jiff has a
    // fast path for this and ends up being quite a bit
    // faster at time of writing (2025-02-14).
    benchmark_with(c, "Asia/Shanghai", 1719711960);
}
