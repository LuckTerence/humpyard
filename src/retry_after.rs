//! Reading a stated retry time from a failed upstream attempt (ADR 0018, amended).
//!
//! A rate-limited endpoint usually says when it will serve again. Switchyard 0.3.0 parses the
//! `Retry-After` header but does not put it on the error it returns, so until it does (see
//! NVIDIA-NeMo/Switchyard#904) the same information is read from the error body, where many providers
//! repeat it ("Please try again in 7.66s", a `retryDelay` field). [`from_body`] is the one place that
//! reads it; adopting the header later means trying it first here.
//!
//! The body is provider-controlled text. Only the first [`MAX_BODY_BYTES`] are read, only a few
//! well-known phrasings and fields are recognized, and the only thing that leaves this module is a
//! duration.

use std::time::Duration;

/// How much of an error body is examined.
const MAX_BODY_BYTES: usize = 4096;
/// Never trust a stated time beyond this, whatever the configured cap is.
const SANITY_LIMIT_SECS: f64 = 86_400.0;

/// Phrases that are followed by a duration with units.
const PHRASES: [&str; 3] = ["try again in ", "retry after ", "retry in "];
/// JSON fields whose value is a duration string or a number of seconds.
const FIELDS: [&str; 3] = ["\"retrydelay\"", "\"retry_after\"", "\"retry-after\""];

/// Unit spellings, longest first so `ms` is not read as `m`.
const UNITS: [(&str, f64); 15] = [
    ("milliseconds", 0.001),
    ("millisecond", 0.001),
    ("seconds", 1.0),
    ("second", 1.0),
    ("minutes", 60.0),
    ("minute", 60.0),
    ("hours", 3600.0),
    ("hour", 3600.0),
    ("secs", 1.0),
    ("sec", 1.0),
    ("mins", 60.0),
    ("min", 60.0),
    ("ms", 0.001),
    ("hrs", 3600.0),
    ("hr", 3600.0),
];
/// Single-letter units, tried after the words above.
const SHORT_UNITS: [(&str, f64); 3] = [("s", 1.0), ("m", 60.0), ("h", 3600.0)];

/// The retry time an error body states, if it states one in a recognized form.
#[must_use]
pub fn from_body(body: &str) -> Option<Duration> {
    let end = (0..=MAX_BODY_BYTES.min(body.len()))
        .rev()
        .find(|&i| body.is_char_boundary(i))?;
    let text = body[..end].to_ascii_lowercase();
    for phrase in PHRASES {
        if let Some(at) = text.find(phrase)
            && let Some(secs) = duration_secs(&text[at + phrase.len()..], false)
        {
            return to_duration(secs);
        }
    }
    for field in FIELDS {
        if let Some(at) = text.find(field) {
            let rest = text[at + field.len()..].trim_start();
            let Some(rest) = rest.strip_prefix(':') else {
                continue;
            };
            let value = rest.trim_start().trim_start_matches('"');
            if let Some(secs) = duration_secs(value, true) {
                return to_duration(secs);
            }
        }
    }
    None
}

/// Clamps a stated time to what the gateway will honor: at least one second, at most `max_secs`.
#[must_use]
pub fn clamp(wait: Duration, max_secs: u64) -> Duration {
    wait.clamp(Duration::from_secs(1), Duration::from_secs(max_secs.max(1)))
}

fn to_duration(secs: f64) -> Option<Duration> {
    (secs.is_finite() && secs > 0.0 && secs <= SANITY_LIMIT_SECS)
        .then(|| Duration::from_secs_f64(secs))
}

/// One or more `<number><unit>` terms ("2m3.5s", "20 seconds", "500ms") summed in seconds. A number
/// with no unit counts as seconds only when `bare_seconds` is set (the JSON number fields).
fn duration_secs(input: &str, bare_seconds: bool) -> Option<f64> {
    let mut rest = input;
    let mut total = 0.0;
    let mut terms = 0;
    loop {
        let digits = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        if digits == 0 || digits > 12 {
            break;
        }
        let Ok(number) = rest[..digits].parse::<f64>() else {
            break;
        };
        let after = rest[digits..].trim_start_matches(' ');
        let unit = UNITS.iter().chain(SHORT_UNITS.iter()).find(|(name, _)| {
            after.starts_with(name)
                && !after[name.len()..].starts_with(|c: char| c.is_ascii_alphabetic())
        });
        match unit {
            Some((name, factor)) => {
                total += number * factor;
                terms += 1;
                rest = &after[name.len()..];
            }
            None if bare_seconds && terms == 0 => {
                total = number;
                terms = 1;
                break;
            }
            None => break,
        }
        if !rest.starts_with(|c: char| c.is_ascii_digit()) {
            break;
        }
    }
    (terms > 0).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(body: &str) -> Option<f64> {
        from_body(body).map(|d| d.as_secs_f64())
    }

    #[test]
    fn phrases_with_units_are_read() {
        assert_eq!(
            secs("Rate limit reached. Please try again in 7.66s."),
            Some(7.66)
        );
        assert_eq!(secs("Please try again in 20s"), Some(20.0));
        assert_eq!(secs("limit hit, retry after 30 seconds"), Some(30.0));
        assert_eq!(secs("Please retry in 28.5s"), Some(28.5));
        assert_eq!(secs("try again in 500ms"), Some(0.5));
        assert_eq!(secs("try again in 5 minutes."), Some(300.0));
        assert_eq!(secs("TRY AGAIN IN 2M3.5S"), Some(123.5));
        assert_eq!(secs("try again in 1h2m"), Some(3720.0));
    }

    #[test]
    fn json_fields_are_read() {
        let google = r#"{"error":{"details":[{"@type":"x.RetryInfo","retryDelay":"28s"}]}}"#;
        assert_eq!(secs(google), Some(28.0));
        assert_eq!(secs(r#"{"retry_after": 12}"#), Some(12.0));
        assert_eq!(secs(r#"{"retry-after":"45"}"#), Some(45.0));
        assert_eq!(secs(r#"{ "retry_after" : 1.5 }"#), Some(1.5));
    }

    #[test]
    fn bare_numbers_are_not_seconds_in_prose() {
        assert_eq!(secs("try again in 20"), None);
        assert_eq!(secs("retry after 7"), None);
    }

    #[test]
    fn unrecognized_or_unusable_text_is_ignored() {
        assert_eq!(secs(""), None);
        assert_eq!(secs("something went wrong"), None);
        assert_eq!(secs("please try again in a moment"), None);
        assert_eq!(secs("try again in 0s"), None);
        assert_eq!(secs(r#"{"retry_after": 0}"#), None);
        assert_eq!(secs(r#"{"retry_after": -5}"#), None);
        assert_eq!(secs(r#"{"retry_after": "soon"}"#), None);
        assert_eq!(secs("try again in 20 apples"), None);
        assert_eq!(secs("try again in 99999999999999999999s"), None);
        assert_eq!(
            secs("try again in 999999s"),
            None,
            "beyond a day is not believed"
        );
    }

    #[test]
    fn only_the_start_of_a_long_body_is_read() {
        let padding = "x".repeat(MAX_BODY_BYTES + 10);
        assert_eq!(secs(&format!("{padding} try again in 5s")), None);
        let early = format!("try again in 5s {padding}");
        assert_eq!(secs(&early), Some(5.0));
    }

    #[test]
    fn a_multibyte_character_at_the_limit_does_not_panic() {
        let body = format!("{}é try again in 5s", "x".repeat(MAX_BODY_BYTES - 1));
        let _ = from_body(&body);
    }

    #[test]
    fn clamp_keeps_a_second_to_the_cap() {
        assert_eq!(
            clamp(Duration::from_millis(100), 300),
            Duration::from_secs(1)
        );
        assert_eq!(clamp(Duration::from_secs(20), 300), Duration::from_secs(20));
        assert_eq!(
            clamp(Duration::from_mins(15), 300),
            Duration::from_secs(300)
        );
        assert_eq!(clamp(Duration::from_secs(5), 0), Duration::from_secs(1));
    }
}
