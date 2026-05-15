use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RanjPrecision {
    Microseconds = 0b00,
    Nanoseconds = 0b01,
    Picoseconds = 0b10,
    Femtoseconds = 0b11,
}

impl RanjPrecision {
    pub fn from_micros_multiplier(self) -> u128 {
        match self {
            Self::Microseconds => 1,
            Self::Nanoseconds => 1_000,
            Self::Picoseconds => 1_000_000,
            Self::Femtoseconds => 1_000_000_000,
        }
    }

    pub fn to_micros_divisor(self) -> u128 {
        self.from_micros_multiplier()
    }

    pub fn from_millis_multiplier(self) -> u128 {
        self.from_micros_multiplier() * 1_000
    }

    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0b11 {
            0b00 => Some(Self::Microseconds),
            0b01 => Some(Self::Nanoseconds),
            0b10 => Some(Self::Picoseconds),
            0b11 => Some(Self::Femtoseconds),
            _ => None,
        }
    }

    pub fn to_bits(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Microseconds => "us",
            Self::Nanoseconds => "ns",
            Self::Picoseconds => "ps",
            Self::Femtoseconds => "fs",
        }
    }
}

static GENERATION_PRECISION: OnceLock<RanjPrecision> = OnceLock::new();

/// Parse `RANJID_PRECISION` without panicking.
///
/// Returns `Ok(precision)` for a recognized value or when the variable is
/// unset (defaulting to [`RanjPrecision::Nanoseconds`]), and
/// `Err(message)` when the variable is set to an unrecognized value.
///
/// Call this during application startup to fail fast with a typed error
/// instead of receiving a panic on the first ID generation.
///
/// # Example
///
/// ```rust
/// use heeranjid::try_generation_precision;
///
/// fn main() {
///     let precision = try_generation_precision()
///         .expect("RANJID_PRECISION is set to an invalid value");
///     // … rest of startup
/// }
/// ```
pub fn try_generation_precision() -> Result<RanjPrecision, String> {
    match std::env::var("RANJID_PRECISION") {
        Err(std::env::VarError::NotPresent) => Ok(RanjPrecision::Nanoseconds),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("RANJID_PRECISION is set but not valid UTF-8".to_string())
        }
        Ok(val) => match val.as_str() {
            "us" => Ok(RanjPrecision::Microseconds),
            "ns" => Ok(RanjPrecision::Nanoseconds),
            "ps" => Ok(RanjPrecision::Picoseconds),
            "fs" => Ok(RanjPrecision::Femtoseconds),
            invalid => Err(format!(
                "RANJID_PRECISION must be one of: us, ns, ps, fs (got '{invalid}')"
            )),
        },
    }
}

/// Return the configured generation precision.
///
/// Reads `RANJID_PRECISION` on first call and caches the result for the
/// lifetime of the process.  Valid values are `us`, `ns`, `ps`, and `fs`.
/// When the variable is unset the default is [`RanjPrecision::Nanoseconds`].
///
/// # Panics
///
/// Panics if `RANJID_PRECISION` is set to an unrecognized value.  Because the
/// result is cached in a [`OnceLock`], the panic occurs on the *first* call —
/// typically the first ID generation — not at process startup.  To validate
/// the configuration eagerly and handle errors without panicking, call
/// [`try_generation_precision()`] during initialization and propagate or
/// unwrap the result there.
pub fn generation_precision() -> RanjPrecision {
    *GENERATION_PRECISION
        .get_or_init(|| try_generation_precision().expect("invalid RANJID_PRECISION"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // Guard against test pollution: these tests set env vars and must not
    // touch the global OnceLock (which is already initialized in the test
    // binary if other tests ran first).  We call try_generation_precision()
    // directly — it always reads the env var fresh.
    //
    // All tests that mutate RANJID_PRECISION are annotated #[serial] so
    // Rust's parallel test runner serialises them and prevents races on the
    // process-global env.

    #[test]
    #[serial]
    fn try_valid_us() {
        // SAFETY: serialised by #[serial]; no concurrent env access.
        unsafe { std::env::set_var("RANJID_PRECISION", "us") };
        assert_eq!(try_generation_precision(), Ok(RanjPrecision::Microseconds));
        // SAFETY: same as above.
        unsafe { std::env::remove_var("RANJID_PRECISION") };
    }

    #[test]
    #[serial]
    fn try_valid_ns() {
        // SAFETY: serialised by #[serial]; no concurrent env access.
        unsafe { std::env::set_var("RANJID_PRECISION", "ns") };
        assert_eq!(try_generation_precision(), Ok(RanjPrecision::Nanoseconds));
        // SAFETY: same as above.
        unsafe { std::env::remove_var("RANJID_PRECISION") };
    }

    #[test]
    #[serial]
    fn try_valid_ps() {
        // SAFETY: serialised by #[serial]; no concurrent env access.
        unsafe { std::env::set_var("RANJID_PRECISION", "ps") };
        assert_eq!(try_generation_precision(), Ok(RanjPrecision::Picoseconds));
        // SAFETY: same as above.
        unsafe { std::env::remove_var("RANJID_PRECISION") };
    }

    #[test]
    #[serial]
    fn try_valid_fs() {
        // SAFETY: serialised by #[serial]; no concurrent env access.
        unsafe { std::env::set_var("RANJID_PRECISION", "fs") };
        assert_eq!(try_generation_precision(), Ok(RanjPrecision::Femtoseconds));
        // SAFETY: same as above.
        unsafe { std::env::remove_var("RANJID_PRECISION") };
    }

    #[test]
    #[serial]
    fn try_unset_defaults_to_nanoseconds() {
        // SAFETY: serialised by #[serial]; no concurrent env access.
        unsafe { std::env::remove_var("RANJID_PRECISION") };
        assert_eq!(try_generation_precision(), Ok(RanjPrecision::Nanoseconds));
    }

    #[test]
    #[serial]
    fn try_invalid_returns_err() {
        // SAFETY: serialised by #[serial]; no concurrent env access.
        unsafe { std::env::set_var("RANJID_PRECISION", "nanoseconds") };
        let result = try_generation_precision();
        // SAFETY: same as above.
        unsafe { std::env::remove_var("RANJID_PRECISION") };
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("nanoseconds"),
            "error message should contain the invalid value, got: {msg}"
        );
        assert!(
            msg.contains("us") && msg.contains("ns") && msg.contains("ps") && msg.contains("fs"),
            "error message should list valid options, got: {msg}"
        );
    }

    #[test]
    #[serial]
    fn try_non_unicode_returns_err() {
        use std::ffi::OsStr;
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            // A byte sequence that is not valid UTF-8.
            let bad = OsStr::from_bytes(&[0xFF]);
            // SAFETY: serialised by #[serial]; no concurrent env access.
            unsafe { std::env::set_var("RANJID_PRECISION", bad) };
            let result = try_generation_precision();
            unsafe { std::env::remove_var("RANJID_PRECISION") };
            assert!(result.is_err(), "non-UTF-8 value should return Err");
            let msg = result.unwrap_err();
            assert!(
                msg.contains("UTF-8") || msg.contains("not valid"),
                "error should mention UTF-8, got: {msg}"
            );
        }
        #[cfg(not(unix))]
        {
            // Non-Unix platforms (e.g. Windows) cannot set a non-Unicode env
            // var via the std API; skip gracefully.
            let _ = OsStr::new(""); // suppress unused-import warning
        }
    }
}
