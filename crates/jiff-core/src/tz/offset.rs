use crate::{
    bounds::{self as b, RangeError},
    civil::{self, DateTime},
    constants as c,
    macros::{rtry, unwrapr},
    Timestamp,
};

/// A fixed offset, in seconds, from UTC.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Offset {
    seconds: i32,
}

/// `pos_sec / 86_400` for the shifted, always-positive second counts used by
/// `Offset::to_datetime`.
///
/// Those are below 2^40 (at most ~6.32e11), so with `86_400 = 2^7 * 675` the
/// division is `(pos_sec >> 7) / 675`, done as a 64-bit multiply-shift by
/// `m = ceil(2^41 / 675)`. With `e = m * 675 - 2^41 = 373`, the result is
/// exact whenever `(pos_sec >> 7) * e < 2^41`, i.e. `pos_sec >> 7 < 5.89e9`,
/// and the product stays below 2^64. Unlike `pos_sec / 86_400`, which
/// compiles to a 64x64->128 `mul` on x86-64 (pinning `rax:rdx`), this is a
/// plain `imul`.
#[inline(always)]
const fn div_86400(pos_sec: u64) -> u64 {
    #[cfg(jiff_div128)]
    return pos_sec / 86_400;
    #[cfg(not(jiff_div128))]
    {
        const M: u64 = (1 << 41) / 675 + 1;
        debug_assert!((pos_sec >> 7) < (1 << 41) / (M * 675 - (1 << 41)));
        ((pos_sec >> 7) * M) >> 41
    }
}

impl Offset {
    /// The minimum possible offset from UTC.
    pub const MIN: Offset = Offset { seconds: b::OffsetTotalSeconds::MIN };

    /// The maximum possible offset from UTC.
    pub const MAX: Offset = Offset { seconds: b::OffsetTotalSeconds::MAX };

    /// The UTC offset.
    pub const UTC: Offset = Offset { seconds: 0 };

    /// The zero offset.
    pub const ZERO: Offset = Offset { seconds: 0 };

    /// Creates a new time zone offset in a `const` context from a given number
    /// of hours.
    #[inline]
    pub const fn constant(hours: i8) -> Offset {
        unwrapr!(Offset::from_hours(hours), "invalid time zone offset hours")
    }

    /// Creates a new time zone offset in a `const` context from a given number
    /// of seconds.
    #[inline]
    pub const fn constant_seconds(seconds: i32) -> Offset {
        unwrapr!(
            Offset::from_seconds(seconds),
            "invalid time zone offset seconds",
        )
    }

    /// Creates a new time zone offset from a given number of hours.
    ///
    /// Negative offsets correspond to time zones west of the prime meridian,
    /// while positive offsets correspond to time zones east of the prime
    /// meridian. Equivalently, in all cases, `civil-time - offset = UTC`.
    #[inline]
    pub const fn from_hours(hours: i8) -> Result<Offset, RangeError> {
        Offset::from_seconds(hours as i32 * c::SECS_PER_HOUR_32)
    }

    /// Returns a new time zone offset from UTC given its representation in
    /// seconds.
    ///
    /// An error is also returned when `seconds` is not in the range specified
    /// by [`OffsetTotalSeconds`](b::OffsetTotalSeconds).
    #[inline]
    pub const fn from_seconds(seconds: i32) -> Result<Offset, RangeError> {
        let seconds = rtry!(b::OffsetTotalSeconds::checkc(seconds as i64));
        Ok(Offset { seconds })
    }

    /// Returns the seconds value corresponding to this time zone offset.
    #[inline]
    pub const fn seconds(self) -> i32 {
        self.seconds
    }

    /// Returns the negation of this offset.
    ///
    /// A negative offset will become positive and vice versa. This is a no-op
    /// if the offset is zero.
    ///
    /// This never panics.
    #[inline]
    pub const fn negate(self) -> Offset {
        // OK because of the boundaries we enforce. `seconds` can never be
        // `i32::MIN`.
        Offset { seconds: -self.seconds() }
    }

    /// Returns the "sign number" or "signum" of this offset.
    ///
    /// The number returned is `-1` when this offset is negative,
    /// `0` when this offset is zero and `1` when this span is positive.
    #[inline]
    pub const fn signum(self) -> i8 {
        self.seconds().signum() as i8
    }

    /// Returns true if and only if this offset is positive.
    ///
    /// This returns false when the offset is zero or negative.
    #[inline]
    pub const fn is_positive(self) -> bool {
        self.seconds() > 0
    }

    /// Returns true if and only if this offset is less than zero.
    ///
    /// This returns false when the offset is zero or positive.
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.seconds() < 0
    }

    /// Returns true if and only if this offset is zero.
    ///
    /// Or equivalently, when this offset corresponds to [`Offset::UTC`].
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.seconds() == 0
    }

    /// Adds the given number of seconds to this offset.
    ///
    /// If the resulting offset would be outside the an offset's boundaries,
    /// an error is returned.
    #[inline]
    pub const fn checked_add(
        self,
        seconds: i32,
    ) -> Result<Offset, RangeError> {
        let seconds =
            rtry!(b::OffsetTotalSeconds::checked_add(self.seconds(), seconds));
        Ok(Offset { seconds })
    }

    /// Subtracts the given number of seconds from this offset.
    ///
    /// If the resulting offset would be outside the an offset's boundaries,
    /// an error is returned.
    #[inline]
    pub const fn checked_sub(
        self,
        seconds: i32,
    ) -> Result<Offset, RangeError> {
        let seconds = rtry!(b::OffsetTotalSeconds::checkc(
            self.seconds() as i64 - seconds as i64,
        ));
        Ok(Offset { seconds })
    }

    /// Returns the number of seconds from this offset to `other`.
    #[inline]
    pub const fn until(self, other: Offset) -> i32 {
        other.seconds() - self.seconds()
    }

    /// Returns the number of seconds since this offset from `other`.
    #[inline]
    pub const fn since(self, other: Offset) -> i32 {
        self.seconds() - other.seconds()
    }

    /// Converts a Unix timestamp with an offset to a Gregorian datetime.
    ///
    /// The offset should correspond to the number of seconds required to
    /// add to this timestamp to get the local time.
    #[inline]
    pub const fn to_datetime(self, timestamp: Timestamp) -> civil::DateTime {
        #[cfg(jiff_fused)]
        return self.to_datetime_fused(timestamp);
        #[cfg(not(jiff_fused))]
        return self.to_datetime_unfused(timestamp);
    }

    /// EXPERIMENT: `to_datetime` as one fused computation on the shifted,
    /// always-positive second count, with no intermediate `UnixEpochDay` /
    /// `TimeSecond` values, no `i32` round trips and no range checks.
    ///
    /// Date: Ben Joffe's fast 64-bit date algorithm
    /// (<https://www.benjoffe.com/fast-date-64>), with the day shift folded
    /// into its reverse-day constant.
    ///
    /// Time: one 64x64->128 multiply yields both `t / 3600` (high half) and
    /// the low 6 bits of `t / 60` (low half), then the `60 + 4 = 64` trick
    /// gives minute and second (<https://www.benjoffe.com/fast-time-of-day>,
    /// after Andrew Consroe's <https://aconz2.github.io/2026/09/09/muldiv.html>).
    #[inline]
    #[allow(dead_code)]
    const fn to_datetime_fused(self, timestamp: Timestamp) -> civil::DateTime {
        // Shift into the positive domain so that division and remainder can
        // use unsigned math. 30 * 400 years covers [-9,999..1970].
        const DAY_SHIFT: u64 = 30 * 146097;
        const SEC_SHIFT: i64 = (DAY_SHIFT as i64) * 86_400;

        let mut nanosecond = timestamp.subsec_nanosecond();
        let is_neg_nano = nanosecond < 0;
        nanosecond += if is_neg_nano { 1_000_000_000 } else { 0 };
        let pos_sec = (timestamp.as_second()
            + (self.seconds() as i64)
            + SEC_SHIFT) as u64
            - (is_neg_nano as u64);

        let pos_day = div_86400(pos_sec);
        let sod = pos_sec - pos_day * 86_400; // second of day, < 86_400

        // ---- Date ----
        // `rev = D_SHIFT - unix_day` and `unix_day = pos_day - DAY_SHIFT`, so
        // the day shift folds into the reverse-day constant.
        #[cfg(not(jiff_date = "fast64"))]
        let (year, month, day) = {
            const ERAS: u64 = 4726498270;
            const D_SHIFT: u64 = 146097 * ERAS - 719469 + DAY_SHIFT;
            const Y_SHIFT: u64 = 400 * ERAS - 1;
            const C1: u64 = 505054698555331; // floor(2^64 * 4 / 146097)
            const C2: u64 = 50504432782230121; // ceil(2^64 * 4 / 1461)
            const C3: u64 = 8619973866219416; // floor(2^64 / 2140)
            let rev = D_SHIFT - pos_day; // reverse day count
            let cen = ((rev as u128 * C1 as u128) >> 64) as u64; // / 36524.25
            let jul = rev + cen - cen / 4; // Julian map
            let num = jul as u128 * C2 as u128; // / 365.25
            let yrs = Y_SHIFT.wrapping_sub((num >> 64) as u64); // forward year
            let ypt = ((782432 * (num as u64 as u128)) >> 64) as u64; // year-part
            let bump = ypt < 126464; // January or February
            let shift = if bump { 191360 } else { 977792 }; // month offset
            let n = (yrs % 4) * 512 + shift - ypt;
            let d = (((n % 65536) as u128 * C3 as u128) >> 64) as u64; // / 2140
            (yrs.wrapping_add(bump as u64) as i16, (n / 65536) as i8, (d + 1) as i8)
        };
        // Same algorithm with a small era shift so that every product fits
        // in a u64 and every constant in 31 bits (`imul` with immediates).
        // Exhaustively verified, see `UnixEpochDay::to_date_fast64`.
        #[cfg(jiff_date = "fast64")]
        let (year, month, day) = {
            const ERAS: u64 = 32;
            const D_SHIFT: u64 = 146097 * ERAS - 719469 + DAY_SHIFT;
            const Y_SHIFT: u64 = 400 * ERAS - 1;
            const S1: u32 = 40;
            const S2: u32 = 39;
            const S3: u32 = 26;
            const C1: u64 = (1 << S1) * 4 / 146097;
            const C2: u64 = ((1 << S2) * 4 + 1460) / 1461;
            const C3: u64 = (1 << S3) / 2140;
            let rev = D_SHIFT - pos_day;
            let cen = (rev * C1) >> S1;
            let jul = rev + cen - cen / 4;
            let num = jul * C2;
            let yrs = Y_SHIFT.wrapping_sub(num >> S2);
            let ypt = ((num & ((1 << S2) - 1)) * 782432) >> S2;
            let bump = ypt < 126464;
            let shift = if bump { 191360 } else { 977792 };
            let n = (yrs % 4) * 512 + shift - ypt;
            let d = ((n % 65536) * C3) >> S3;
            (yrs.wrapping_add(bump as u64) as i16, (n / 65536) as i8, (d + 1) as i8)
        };

        // ---- Time ----
        // Directly from `pos_sec`, without waiting for `pos_day`/`sod`:
        // `SEC_SHIFT` is a whole number of days, so with `60 + 4 = 64` and
        // `24 + 8 = 32`:
        //   second = (pos_sec + 4 * tmin) % 64,   tmin  = pos_sec / 60
        //   minute = (tmin + 4 * thour) % 64,     thour = pos_sec / 3600
        //   hour   = (thour + 8 * pos_day) % 32
        // `pos_sec < 2^40`, so the two quotients use 64x64->128 `mulhi` by
        // `ceil(2^64 / d)`, exact far beyond that range.
        #[cfg(jiff_time = "direct")]
        let (hour, minute, second) = {
            const M60: u64 = u64::MAX / 60 + 1;
            const M3600: u64 = u64::MAX / 3600 + 1;
            let tmin = ((pos_sec as u128 * M60 as u128) >> 64) as u64;
            let thour = ((pos_sec as u128 * M3600 as u128) >> 64) as u64;
            let _ = sod;
            (
                (thour + 8 * pos_day) % 32,
                (tmin + 4 * thour) % 64,
                (pos_sec + 4 * tmin) % 64,
            )
        };
        #[cfg(not(any(jiff_time = "v3", jiff_time = "direct")))]
        let (hour, minute, second) = {
            const MUL_H: u64 = (1 << 32) / 3600 + 1;
            const MUL_M: u64 = (1 << 25) / 60 + 1;
            const MUL: u64 = (MUL_H << 32) + MUL_M;
            let prd = sod as u128 * MUL as u128;
            let hour = (prd >> 64) as u64;
            let tmin = (prd as u64) >> 25; // only its low 6 bits matter
            (hour, (tmin + 4 * hour) % 64, (sod + 4 * tmin) % 64)
        };
        // Two independent `imul`s with 31-bit immediates.
        #[cfg(jiff_time = "v3")]
        let (hour, minute, second) = {
            let tmin = (sod * 71582789) >> 32; // = sod / 60
            let hour = (sod * 1193047) >> 32; // = sod / 3600
            (hour, (tmin + 4 * hour) % 64, (sod + 4 * tmin) % 64)
        };

        civil::DateTime::from_parts(
            civil::Date::from_raw_unchecked(year, month, day),
            civil::Time::from_raw_unchecked(
                hour as i8,
                minute as i8,
                second as i8,
                nanosecond,
            ),
        )
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) const fn to_datetime_unfused(
        self,
        timestamp: Timestamp,
    ) -> civil::DateTime {
        let offset = self;
        let second = timestamp.as_second();
        let mut nanosecond = timestamp.subsec_nanosecond();

        // Shift second comfortably into the postive domain
        // so that division and remainder can use unsigned math
        // which is much faster.
        // 30 * 400 years: 12,000 yr range > [-9,999..1970]
        // (146097 being the number of days per 400 years).
        const DAY_SHIFT: i32 = 30 * 146097;
        const SEC_SHIFT: i64 = (DAY_SHIFT as i64) * 86_400;

        let mut pos_sec =
            (second + (offset.seconds() as i64) + SEC_SHIFT) as u64;

        let is_neg_nano: bool = nanosecond < 0;
        pos_sec -= is_neg_nano as u64;
        nanosecond += if is_neg_nano { 1_000_000_000 } else { 0 };

        let pos_day = div_86400(pos_sec);
        let mut epoch_day = pos_day as i32;
        let second = (pos_sec - pos_day * 86_400) as i32;

        epoch_day -= DAY_SHIFT;

        // The math above, coupled with the ranges allowed on `Timestamp` and
        // `Offset` (by design) guarantee that our resulting datetime will
        // always be in range, so skip the (never failing) range checks.
        #[cfg(not(jiff_checked))]
        let date = civil::UnixEpochDay::new_unchecked(epoch_day).to_date();
        #[cfg(not(jiff_checked))]
        let time = civil::TimeSecond::new_unchecked(second)
            .to_time()
            .with_subsec_nanosecond_unchecked(nanosecond);
        // EXPERIMENT: the original checked constructors.
        #[cfg(jiff_checked)]
        let date = unwrapr!(
            civil::UnixEpochDay::new(epoch_day),
            "always valid Unix epoch day",
        )
        .to_date();
        #[cfg(jiff_checked)]
        let time = unwrapr!(
            unwrapr!(
                civil::TimeSecond::new(second),
                "always valid civil second time"
            )
            .to_time()
            .with_subsec_nanosecond(nanosecond),
            "always valid civil subsecond"
        );
        civil::DateTime::from_parts(date, time)
    }

    /// Converts the given civil datetime to a timestamp using this offset.
    ///
    /// # Errors
    ///
    /// This returns an error if this would have returned a timestamp outside
    /// of its minimum and maximum values.
    #[inline]
    pub const fn to_timestamp(
        self,
        dt: civil::DateTime,
    ) -> Result<Timestamp, RangeError> {
        let offset = self;
        let epoch_day = dt.date().to_unix_epoch_day().day();
        let mut second = (epoch_day as i64) * c::SECS_PER_CIVIL_DAY
            + (dt.time().to_second().second() as i64);
        let mut nanosecond = dt.time().subsec_nanosecond();
        second -= offset.seconds() as i64;
        if second < 0 && nanosecond != 0 {
            second += 1;
            nanosecond -= c::NANOS_PER_SEC_32;
        }
        let second = rtry!(b::UnixEpochSeconds::checkc(second));
        Ok(Timestamp::new_unchecked(second, nanosecond))
    }
}

impl Offset {
    #[inline]
    fn part_hours(self) -> i8 {
        (self.seconds() / c::SECS_PER_HOUR_32) as i8
    }

    #[inline]
    fn part_minutes(self) -> i8 {
        ((self.seconds() / c::SECS_PER_MIN_32) % c::MINS_PER_HOUR_32) as i8
    }

    #[inline]
    fn part_seconds(self) -> i8 {
        (self.seconds() % c::SECS_PER_MIN_32) as i8
    }
}

/// Negate this offset.
///
/// A positive offset becomes negative and vice versa. This is a no-op for the
/// zero offset.
///
/// This never panics.
impl core::ops::Neg for Offset {
    type Output = Offset;

    #[inline]
    fn neg(self) -> Offset {
        self.negate()
    }
}

/// Adds a number of seconds to an `Offset`.
///
/// # Panics
///
/// When adding would result in a value outside the boundaries of a
/// `Offset`.
impl core::ops::Add<i32> for Offset {
    type Output = Offset;

    fn add(self, seconds: i32) -> Offset {
        self.checked_add(seconds).unwrap()
    }
}

/// Adds a number of seconds into an `Offset`.
///
/// # Panics
///
/// When adding would result in a value outside the boundaries of a
/// `Offset`.
impl core::ops::AddAssign<i32> for Offset {
    #[inline]
    fn add_assign(&mut self, rhs: i32) {
        *self = *self + rhs;
    }
}

/// Subtracts a number of seconds from an `Offset`.
///
/// # Panics
///
/// When adding would result in a value outside the boundaries of a
/// `Offset`.
impl core::ops::Sub<i32> for Offset {
    type Output = Offset;

    fn sub(self, seconds: i32) -> Offset {
        self.checked_sub(seconds).unwrap()
    }
}

/// Subtracts a number of seconds from an `Offset` in place.
///
/// # Panics
///
/// When adding would result in a value outside the boundaries of a
/// `Offset`.
impl core::ops::SubAssign<i32> for Offset {
    #[inline]
    fn sub_assign(&mut self, rhs: i32) {
        *self = *self - rhs;
    }
}

impl core::fmt::Debug for Offset {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let sign = if self.is_negative() { "-" } else { "" };
        write!(
            f,
            "{sign}{:02}:{:02}:{:02}",
            self.part_hours().unsigned_abs(),
            self.part_minutes().unsigned_abs(),
            self.part_seconds().unsigned_abs(),
        )
    }
}

impl core::fmt::Display for Offset {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let sign = if self.is_negative() { "-" } else { "+" };
        let hours = self.part_hours().unsigned_abs();
        let minutes = self.part_minutes().unsigned_abs();
        let seconds = self.part_seconds().unsigned_abs();
        if hours == 0 && minutes == 0 && seconds == 0 {
            f.write_str("+00")
        } else if hours != 0 && minutes == 0 && seconds == 0 {
            write!(f, "{sign}{hours:02}")
        } else if minutes != 0 && seconds == 0 {
            write!(f, "{sign}{hours:02}:{minutes:02}")
        } else {
            write!(f, "{sign}{hours:02}:{minutes:02}:{seconds:02}")
        }
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Offset {
    fn format(&self, f: defmt::Formatter) {
        let sign = if self.is_negative() { "-" } else { "" };
        defmt::write!(
            f,
            "{=str}{=u8:02}:{=u8:02}:{=u8:02}",
            sign,
            self.part_hours().unsigned_abs(),
            self.part_minutes().unsigned_abs(),
            self.part_seconds().unsigned_abs(),
        )
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Offset {
    fn arbitrary(
        u: &mut arbitrary::Unstructured<'a>,
    ) -> arbitrary::Result<Offset> {
        let secs = u.int_in_range(
            b::OffsetTotalSeconds::MIN..=b::OffsetTotalSeconds::MAX,
        )?;
        Ok(Offset::from_seconds(secs).unwrap_or(Offset::UTC))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <i32 as arbitrary::Arbitrary>::size_hint(depth)
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for Offset {
    fn arbitrary(g: &mut quickcheck::Gen) -> Offset {
        let secs = b::OffsetTotalSeconds::arbitrary(g);
        Offset::from_seconds(secs).unwrap_or(Offset::UTC)
    }

    fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = Self>> {
        let secs = self.seconds();
        alloc::boxed::Box::new(secs.shrink().filter_map(|secs| {
            let secs = b::OffsetTotalSeconds::check(secs).ok()?;
            Offset::from_seconds(secs).ok()
        }))
    }
}

/// A possibly ambiguous [`Offset`].
///
/// One of three possibilities encoded by this type occurs when converting a
/// civil datetime into a specific instant in time. In rare cases, the civil
/// datetime can fall into a gap or a fold, in which case, one of two offsets
/// could be applicable. Or, perhaps, neither. Callers must decide how best to
/// handle these cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AmbiguousOffset {
    /// The offset for a particular civil datetime and time zone is
    /// unambiguous.
    ///
    /// This is the overwhelmingly common case. In general, the only time this
    /// case does not occur is when there is a transition to a different time
    /// zone (rare) or to/from daylight saving time (occurs for 1 hour twice
    /// in year in many geographic locations).
    Unambiguous {
        /// The offset from UTC for the corresponding civil datetime given. The
        /// offset is determined via the relevant time zone data, and in this
        /// case, there is only one possible offset that could be applied to
        /// the given civil datetime.
        offset: Offset,
    },
    /// The offset for a particular civil datetime and time zone is ambiguous
    /// because there is a gap.
    ///
    /// This most commonly occurs when a civil datetime corresponds to an hour
    /// that was "skipped" in a jump to DST (daylight saving time).
    Gap {
        /// The offset corresponding to the time before a gap.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time immediately preceding `2020-03-08T02:00:00` is `-08`.
        before: Offset,
        /// The offset corresponding to the later time in a gap.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time immediately following `2020-03-08T02:59:59` is `-07`.
        after: Offset,
    },
    /// The offset for a particular civil datetime and time zone is ambiguous
    /// because there is a fold.
    ///
    /// This most commonly occurs when a civil datetime corresponds to an hour
    /// that was "repeated" in a jump to standard time from DST (daylight
    /// saving time).
    Fold {
        /// The offset corresponding to the earlier time in a fold.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time on the first `2020-11-01T01:00:00` is `-07`.
        before: Offset,
        /// The offset corresponding to the earlier time in a fold.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time on the second `2020-11-01T01:00:00` is `-08`.
        after: Offset,
    },
}

impl AmbiguousOffset {
    #[inline]
    pub(crate) const fn into_ambiguous_timestamp(
        self,
        dt: DateTime,
    ) -> AmbiguousTimestamp {
        AmbiguousTimestamp { dt, offset: self }
    }
}

/// A possibly ambiguous [`Timestamp`].
///
/// While this is called an ambiguous _timestamp_, the thing that is
/// actually ambiguous is the offset. That is, an ambiguous timestamp is
/// actually a pair of a [`civil::DateTime`](crate::civil::DateTime) and an
/// [`AmbiguousOffset`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AmbiguousTimestamp {
    dt: DateTime,
    offset: AmbiguousOffset,
}

impl AmbiguousTimestamp {
    /// Returns the civil datetime that was used to create this ambiguous
    /// timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff_core::{civil::date, tz::posix};
    ///
    /// let tz = posix::TimeZone::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
    /// let dt = date(2024, 7, 10).at(17, 15, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.datetime(), dt);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub const fn datetime(&self) -> DateTime {
        self.dt
    }

    /// Returns the possibly ambiguous offset that is the ultimate source of
    /// ambiguity.
    ///
    /// Most civil datetimes are not ambiguous, and thus, the offset will not
    /// be ambiguous either. In this case, the offset returned will be the
    /// [`AmbiguousOffset::Unambiguous`] variant.
    ///
    /// But, not all civil datetimes are unambiguous. There are exactly two
    /// cases where a civil datetime can be ambiguous: when a civil datetime
    /// does not exist (a gap) or when a civil datetime is repeated (a fold).
    /// In both such cases, the _offset_ is the thing that is ambiguous as
    /// there are two possible choices for the offset in both cases: the offset
    /// before the transition (whether it's a gap or a fold) or the offset
    /// after the transition.
    ///
    /// This type captures the fact that computing an offset from a civil
    /// datetime in a particular time zone is in one of three possible states:
    ///
    /// 1. It is unambiguous.
    /// 2. It is ambiguous because there is a gap in time.
    /// 3. It is ambiguous because there is a fold in time.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff_core::{civil::date, tz::{self, posix, AmbiguousOffset}};
    ///
    /// let tz = posix::TimeZone::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.offset(), AmbiguousOffset::Unambiguous {
    ///     offset: tz::offset(-4),
    /// });
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.offset(), AmbiguousOffset::Gap {
    ///     before: tz::offset(-5),
    ///     after: tz::offset(-4),
    /// });
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.offset(), AmbiguousOffset::Fold {
    ///     before: tz::offset(-4),
    ///     after: tz::offset(-5),
    /// });
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub const fn offset(&self) -> AmbiguousOffset {
        self.offset
    }

    /// Returns true if and only if this possibly ambiguous timestamp is
    /// actually ambiguous.
    ///
    /// This occurs precisely in cases when the offset is _not_
    /// [`AmbiguousOffset::Unambiguous`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff_core::{civil::date, tz::posix};
    ///
    /// let tz = posix::TimeZone::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(!ts.is_ambiguous());
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.is_ambiguous());
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.is_ambiguous());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub const fn is_ambiguous(&self) -> bool {
        !matches!(self.offset(), AmbiguousOffset::Unambiguous { .. })
    }

    /// Disambiguates this timestamp according to the "compatible" strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "compatible" strategy selects the offset corresponding to the civil
    /// time after a gap, and the offset corresponding to the civil time before
    /// a fold. This is what is specified in [RFC 5545].
    ///
    /// [RFC 5545]: https://datatracker.ietf.org/doc/html/rfc5545
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    #[inline]
    pub const fn compatible(self) -> Result<Timestamp, RangeError> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { before, .. } => before,
            AmbiguousOffset::Fold { before, .. } => before,
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this timestamp according to the "earlier" strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "earlier" strategy selects the offset corresponding to the civil
    /// time before a gap, and the offset corresponding to the civil time
    /// before a fold.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    #[inline]
    pub const fn earlier(self) -> Result<Timestamp, RangeError> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { after, .. } => after,
            AmbiguousOffset::Fold { before, .. } => before,
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this timestamp according to the "later" strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "later" strategy selects the offset corresponding to the civil
    /// time after a gap, and the offset corresponding to the civil time
    /// after a fold.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    #[inline]
    pub const fn later(self) -> Result<Timestamp, RangeError> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { before, .. } => before,
            AmbiguousOffset::Fold { after, .. } => after,
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this timestamp according to the "reject" strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "reject" strategy always returns an error when the timestamp
    /// is ambiguous.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// This also returns an error when the timestamp is ambiguous.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff_core::{civil::date, tz::{posix, Offset}};
    ///
    /// let tz = posix::TimeZone::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.later().unwrap().to_datetime(Offset::UTC),
    ///     date(2024, 7, 15).at(21, 30, 0, 0),
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.unambiguous().is_err());
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.unambiguous().is_err());
    /// ```
    #[inline]
    pub const fn unambiguous(self) -> Result<Timestamp, AmbiguousError> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { before, after } => {
                return Err(AmbiguousError {
                    kind: AmbiguousErrorKind::BecauseGap { before, after },
                });
            }
            AmbiguousOffset::Fold { before, after } => {
                return Err(AmbiguousError {
                    kind: AmbiguousErrorKind::BecauseFold { before, after },
                });
            }
        };
        match offset.to_timestamp(self.dt) {
            Ok(timestamp) => Ok(timestamp),
            Err(range_error) => Err(AmbiguousError {
                kind: AmbiguousErrorKind::Range(range_error),
            }),
        }
    }
}

/// An error that occurs when an unmabiguous civil datetime is demanded.
///
/// This surfaces via the [`AmbiguousTimestamp::unambiguous`] API.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AmbiguousError {
    kind: AmbiguousErrorKind,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum AmbiguousErrorKind {
    Range(RangeError),
    BecauseFold { before: Offset, after: Offset },
    BecauseGap { before: Offset, after: Offset },
}

impl core::fmt::Display for AmbiguousError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use self::AmbiguousErrorKind::*;

        match self.kind {
            Range(ref err) => core::fmt::Display::fmt(err, f),
            BecauseFold { before, after } => write!(
                f,
                "datetime is ambiguous since it falls into a \
                 fold between offsets {before} and {after}",
            ),
            BecauseGap { before, after } => write!(
                f,
                "datetime is ambiguous since it falls into a \
                 gap between offsets {before} and {after}",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AmbiguousError {}

#[cfg(test)]
mod tests {
    use super::Offset;
    use crate::Timestamp;

    #[test]
    fn div_86400_exact_over_reachable_range() {
        use super::div_86400;
        // The reachable `pos_sec` range in `to_datetime`, with a margin.
        const SEC_SHIFT: i64 = 30 * 146097 * 86_400;
        let lo = Timestamp::MIN.as_second() - 1 + Offset::MIN.seconds() as i64
            + SEC_SHIFT;
        let hi = Timestamp::MAX.as_second() + Offset::MAX.seconds() as i64
            + SEC_SHIFT;
        assert!(lo > 0);
        let (lo, hi) = (lo as u64, hi as u64);
        // Both sides of every day boundary, plus the range ends.
        for day in lo / 86_400..=hi / 86_400 + 1 {
            for x in [day * 86_400, day * 86_400 + 86_399] {
                assert_eq!(div_86400(x), x / 86_400, "x={x}");
            }
        }
        for x in [lo, hi, hi + 86_400] {
            assert_eq!(div_86400(x), x / 86_400, "x={x}");
        }
    }

    #[test]
    fn to_datetime_fused_matches_unfused() {
        let checked = core::cell::Cell::new(0u64);
        let check = |secs: i64, nanos: i32, offset: Offset| {
            let Ok(ts) = Timestamp::new(secs, nanos) else { return };
            checked.set(checked.get() + 1);
            assert_eq!(
                offset.to_datetime_fused(ts),
                offset.to_datetime_unfused(ts),
                "secs={secs} nanos={nanos} offset={}",
                offset.seconds(),
            );
        };
        let (smin, smax) = (Timestamp::MIN.as_second(), Timestamp::MAX.as_second());
        let (omin, omax) = (Offset::MIN.seconds(), Offset::MAX.seconds());

        // Extremes.
        for secs in [smin, smin + 1, -1, 0, 1, smax - 1, smax] {
            for nanos in [-999_999_999, -1, 0, 1, 999_999_999] {
                for off in [omin, -1, 0, 1, omax] {
                    check(secs, nanos, Offset::constant_seconds(off));
                }
            }
        }
        // Every second around a few hundred local midnights.
        for day in (-4_000_000i64..3_000_000).step_by(9_973) {
            for off in [omin, -3600, 0, 19800, omax] {
                let midnight = day * 86_400 - off as i64;
                for secs in midnight - 3..=midnight + 3 {
                    for nanos in [-1, 0, 1] {
                        check(secs, nanos, Offset::constant_seconds(off));
                    }
                }
            }
        }
        // Pseudo-random sample of the full range.
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for _ in 0..2_000_000 {
            let secs = smin + (next() % (smax - smin + 1) as u64) as i64;
            let nanos = (next() % 1_999_999_999) as i32 - 999_999_999;
            let off = omin + (next() % (omax - omin + 1) as u64) as i32;
            check(secs, nanos, Offset::constant_seconds(off));
        }
        // Guard against inputs silently being rejected by `Timestamp::new`.
        assert!(checked.get() > 2_000_000, "only {} checked", checked.get());
    }

    #[test]
    fn checked_subtracts_positive_and_negative_seconds() {
        assert_eq!(
            Offset::UTC.checked_sub(1).unwrap(),
            Offset::constant_seconds(-1),
        );
        assert_eq!(
            Offset::UTC.checked_sub(-1).unwrap(),
            Offset::constant_seconds(1),
        );
    }

    #[test]
    fn checked_sub_rejects_overflow() {
        assert!(Offset::MIN.checked_sub(1).is_err());
        assert!(Offset::MAX.checked_sub(-1).is_err());
        assert!(Offset::UTC.checked_sub(i32::MIN).is_err());
    }

    #[test]
    fn subtraction_operators_subtract_seconds() {
        assert_eq!(Offset::UTC - 1, Offset::constant_seconds(-1));

        let mut offset = Offset::UTC;
        offset -= 1;
        assert_eq!(offset, Offset::constant_seconds(-1));
    }
}
