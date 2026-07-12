# FCS 5.0 Front-End Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a strict, versioned FCS 5 parser foundation with independent FCS/FCBC/Execution ABI versions, document profiles, exact rational beat keys, and a validated tempo map while preserving the buildability of the existing v4 converter.

**Architecture:** Add a temporary `fcs_core::v5` module with focused `version`, `ast`, `parser`, and `validation` units. Keep the existing v4 `ast`, `parser`, compiler and converter untouched during this phase; expose FCS 5 through explicit `fcs_core::v5::parse_document` and promote it during the final roadmap phase.

**Tech Stack:** Rust 2024, `nom` 8, `thiserror` 2, existing workspace Clippy and cargo-nextest tooling.

---

## File map

**Create:**

- `crates/fcs-core/src/v5/mod.rs` — public FCS 5 namespace and re-exports.
- `crates/fcs-core/src/v5/version.rs` — semantic versions and compatibility rules.
- `crates/fcs-core/src/v5/ast/mod.rs` — FCS 5 document/profile/tempo AST.
- `crates/fcs-core/src/v5/ast/time.rs` — checked rational beat and BPM value types.
- `crates/fcs-core/src/v5/parser/mod.rs` — public strict FCS 5 parse entry point.
- `crates/fcs-core/src/v5/parser/header.rs` — `#fcs major.minor.patch` parser.
- `crates/fcs-core/src/v5/parser/document.rs` — format/profile and minimal top-level parser.
- `crates/fcs-core/src/v5/parser/tempo.rs` — exact beat and tempo map parser.
- `crates/fcs-core/src/v5/validation.rs` — profile and tempo invariants.
- `crates/fcs-core/tests/fcs5_frontend.rs` — public API integration tests.
- `examples/fcs/fcs5-fragment.fcs` — minimal fragment fixture.
- `examples/fcs/fcs5-chart.fcs` — minimal chart fixture with tempo map.

**Modify:**

- `crates/fcs-core/src/lib.rs` — export the temporary `v5` namespace and update crate-level version wording.
- `fcs.md` — add an implementation-status note linking the approved FCS 5 design and recording the initial version triplet without replacing the v4 body during this phase.

## Task 1: Add independent version types and constants

**Files:**

- Create: `crates/fcs-core/src/v5/version.rs`
- Create: `crates/fcs-core/src/v5/mod.rs`
- Modify: `crates/fcs-core/src/lib.rs`
- Test: `crates/fcs-core/tests/fcs5_frontend.rs`

- [ ] **Step 1: Write the failing public version test**

Create `crates/fcs-core/tests/fcs5_frontend.rs` with:

```rust
use fcs_core::v5::version::{
    EXECUTION_ABI_VERSION, FCBC_FORMAT_VERSION, FCS_SOURCE_VERSION, Version,
};

#[test]
fn exposes_independent_fcs_fcbc_and_abi_versions() {
    assert_eq!(FCS_SOURCE_VERSION, Version::new(5, 0, 0));
    assert_eq!(FCBC_FORMAT_VERSION, Version::new(2, 0, 0));
    assert_eq!(EXECUTION_ABI_VERSION, Version::new(1, 0, 0));
    assert_eq!(FCS_SOURCE_VERSION.to_string(), "5.0.0");
}
```

- [ ] **Step 2: Run Clippy, then run the targeted test and verify it fails**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
```

Expected: FAIL because `fcs_core::v5` does not exist.

Then run only after the compile failure is observed:

```text
cargo nextest run -p fcs-core
```

Expected: FAIL to compile with an unresolved `fcs_core::v5` import.

- [ ] **Step 3: Add the minimal version implementation**

Create `crates/fcs-core/src/v5/version.rs`:

```rust
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }

    pub const fn supports_source(self, source: Self) -> bool {
        self.major == source.major && self.minor >= source.minor
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionParseError;

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('.');
        let major = parts.next().ok_or(VersionParseError)?.parse().map_err(|_| VersionParseError)?;
        let minor = parts.next().ok_or(VersionParseError)?.parse().map_err(|_| VersionParseError)?;
        let patch = parts.next().ok_or(VersionParseError)?.parse().map_err(|_| VersionParseError)?;
        if parts.next().is_some() {
            return Err(VersionParseError);
        }
        Ok(Self::new(major, minor, patch))
    }
}

pub const FCS_SOURCE_VERSION: Version = Version::new(5, 0, 0);
pub const FCBC_FORMAT_VERSION: Version = Version::new(2, 0, 0);
pub const EXECUTION_ABI_VERSION: Version = Version::new(1, 0, 0);
```

Create `crates/fcs-core/src/v5/mod.rs`:

```rust
pub mod version;
```

Add this exact module declaration to `crates/fcs-core/src/lib.rs`:

```rust
pub mod v5;
```

Replace the crate-level description with:

```rust
//! FCS (Functional Chart Specification) core library.
//!
//! The existing public modules implement the current FCS v4 toolchain. The
//! versioned `v5` module contains the staged FCS 5 front end until final cutover.
```

- [ ] **Step 4: Run Clippy and the targeted test**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: both commands PASS.

- [ ] **Step 5: Commit the version foundation**

```text
git add crates/fcs-core/src/lib.rs crates/fcs-core/src/v5/mod.rs crates/fcs-core/src/v5/version.rs crates/fcs-core/tests/fcs5_frontend.rs
git commit -m "feat(core): add FCS 5 version foundation"
```

## Task 2: Add exact rational beat and validated BPM types

**Files:**

- Create: `crates/fcs-core/src/v5/ast/mod.rs`
- Create: `crates/fcs-core/src/v5/ast/time.rs`
- Modify: `crates/fcs-core/src/v5/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_frontend.rs`

- [ ] **Step 1: Add failing exact-beat tests**

Append to `crates/fcs-core/tests/fcs5_frontend.rs`:

```rust
use fcs_core::v5::ast::{Beat, Bpm};

#[test]
fn beat_arithmetic_is_exact_and_normalized() {
    let one_third = Beat::new(1, 3).unwrap();
    let two_thirds = Beat::new(2, 3).unwrap();
    assert_eq!(one_third.checked_add(two_thirds).unwrap(), Beat::new(1, 1).unwrap());
    assert_eq!(Beat::new(2, 6).unwrap(), one_third);
}

#[test]
fn rejects_zero_denominator_and_invalid_bpm() {
    assert!(Beat::new(1, 0).is_err());
    assert!(Bpm::new(0.0).is_err());
    assert!(Bpm::new(f64::NAN).is_err());
    assert_eq!(Bpm::new(180.0).unwrap().get(), 180.0);
}
```

- [ ] **Step 2: Run Clippy and verify the tests fail to compile**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: FAIL because `v5::ast::{Beat, Bpm}` does not exist.

- [ ] **Step 3: Implement checked rational beat and BPM wrappers**

Create `crates/fcs-core/src/v5/ast/time.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Beat {
    numerator: i64,
    denominator: i64,
}

impl PartialOrd for Beat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Beat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let left = self.numerator as i128 * other.denominator as i128;
        let right = other.numerator as i128 * self.denominator as i128;
        left.cmp(&right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatError {
    ZeroDenominator,
    Overflow,
}

impl Beat {
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, BeatError> {
        if denominator == 0 {
            return Err(BeatError::ZeroDenominator);
        }
        let sign = if denominator < 0 { -1 } else { 1 };
        let numerator = numerator.checked_mul(sign).ok_or(BeatError::Overflow)?;
        let denominator = denominator.checked_mul(sign).ok_or(BeatError::Overflow)?;
        let divisor = gcd(numerator.unsigned_abs(), denominator as u64) as i64;
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    pub const fn denominator(self) -> i64 {
        self.denominator
    }

    pub fn checked_add(self, other: Self) -> Result<Self, BeatError> {
        let left = self
            .numerator
            .checked_mul(other.denominator)
            .ok_or(BeatError::Overflow)?;
        let right = other
            .numerator
            .checked_mul(self.denominator)
            .ok_or(BeatError::Overflow)?;
        let denominator = self
            .denominator
            .checked_mul(other.denominator)
            .ok_or(BeatError::Overflow)?;
        Self::new(left.checked_add(right).ok_or(BeatError::Overflow)?, denominator)
    }
}

const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    if a == 0 { 1 } else { a }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bpm(f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBpm;

impl Bpm {
    pub fn new(value: f64) -> Result<Self, InvalidBpm> {
        if value.is_finite() && value > 0.0 {
            Ok(Self(value))
        } else {
            Err(InvalidBpm)
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}
```

Create `crates/fcs-core/src/v5/ast/mod.rs`:

```rust
mod time;

pub use time::{Beat, BeatError, Bpm, InvalidBpm};
```

Add this exact declaration to `crates/fcs-core/src/v5/mod.rs`:

```rust
pub mod ast;
```

- [ ] **Step 4: Run Clippy and exact-value tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: PASS.

- [ ] **Step 5: Commit exact time primitives**

```text
git add crates/fcs-core/src/v5/ast crates/fcs-core/src/v5/mod.rs crates/fcs-core/tests/fcs5_frontend.rs
git commit -m "feat(core): add exact FCS 5 beat primitives"
```

## Task 3: Parse and validate the mandatory FCS 5 source header

**Files:**

- Create: `crates/fcs-core/src/v5/parser/mod.rs`
- Create: `crates/fcs-core/src/v5/parser/header.rs`
- Modify: `crates/fcs-core/src/v5/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_frontend.rs`

- [ ] **Step 1: Add failing header tests**

Append:

```rust
use fcs_core::v5::parser::{parse_header, ParseError};

#[test]
fn parses_exact_fcs5_header() {
    let (rest, version) = parse_header("#fcs 5.0.0\nformat { profile: fragment; }").unwrap();
    assert_eq!(version, FCS_SOURCE_VERSION);
    assert_eq!(rest, "format { profile: fragment; }");
}

#[test]
fn rejects_missing_or_wrong_major_header() {
    assert_eq!(parse_header("format { profile: fragment; }"), Err(ParseError::MissingHeader));
    assert_eq!(parse_header("#fcs 4.1.0\n"), Err(ParseError::UnsupportedSourceVersion(Version::new(4, 1, 0))));
    assert_eq!(parse_header("#fcs 5.1.0\n"), Err(ParseError::UnsupportedSourceVersion(Version::new(5, 1, 0))));
}
```

- [ ] **Step 2: Run Clippy and verify failure**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: FAIL because the FCS 5 parser module does not exist.

- [ ] **Step 3: Implement a strict header parser**

Create `crates/fcs-core/src/v5/parser/header.rs`:

```rust
use std::str::FromStr;

use crate::v5::version::{FCS_SOURCE_VERSION, Version};

use super::ParseError;

pub fn parse_header(input: &str) -> Result<(&str, Version), ParseError> {
    let line_end = input.find('\n').unwrap_or(input.len());
    let line = input[..line_end].trim_end_matches('\r');
    let version_text = line
        .strip_prefix("#fcs ")
        .ok_or(ParseError::MissingHeader)?;
    let version = Version::from_str(version_text).map_err(|_| ParseError::InvalidVersion)?;
    if !FCS_SOURCE_VERSION.supports_source(version) {
        return Err(ParseError::UnsupportedSourceVersion(version));
    }
    let rest = if line_end == input.len() {
        ""
    } else {
        &input[line_end + 1..]
    };
    Ok((rest, version))
}
```

Create `crates/fcs-core/src/v5/parser/mod.rs`:

```rust
mod header;

use crate::v5::version::Version;

pub use header::parse_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingHeader,
    InvalidVersion,
    UnsupportedSourceVersion(Version),
    InvalidSyntax(&'static str),
}
```

Add this exact declaration to `crates/fcs-core/src/v5/mod.rs`:

```rust
pub mod parser;
```

- [ ] **Step 4: Run Clippy and header tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: PASS.

- [ ] **Step 5: Commit strict header parsing**

```text
git add crates/fcs-core/src/v5/parser crates/fcs-core/src/v5/mod.rs crates/fcs-core/tests/fcs5_frontend.rs
git commit -m "feat(parser): require an FCS 5 source version header"
```

## Task 4: Add document profiles and the minimal format block

**Files:**

- Modify: `crates/fcs-core/src/v5/ast/mod.rs`
- Create: `crates/fcs-core/src/v5/parser/document.rs`
- Modify: `crates/fcs-core/src/v5/parser/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_frontend.rs`

- [ ] **Step 1: Add failing format/profile tests**

Append:

```rust
use fcs_core::v5::{
    ast::DocumentProfile,
    parser::parse_document,
};

#[test]
fn parses_fragment_profile() {
    let document = parse_document("#fcs 5.0.0\nformat { profile: fragment; }").unwrap();
    assert_eq!(document.profile, DocumentProfile::Fragment);
    assert_eq!(document.source_version, FCS_SOURCE_VERSION);
    assert!(document.tempo_map.is_none());
}

#[test]
fn rejects_unknown_profile() {
    assert!(matches!(
        parse_document("#fcs 5.0.0\nformat { profile: unknown; }"),
        Err(ParseError::InvalidSyntax("document profile"))
    ));
}
```

- [ ] **Step 2: Run Clippy and verify failure**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: FAIL because `Document`, `DocumentProfile` and `parse_document` do not exist.

- [ ] **Step 3: Add the minimal document AST and parser**

Append to `crates/fcs-core/src/v5/ast/mod.rs`:

```rust
use crate::v5::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentProfile {
    Fragment,
    Chart,
    Playable,
    Renderable,
    Publishable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub source_version: Version,
    pub profile: DocumentProfile,
    pub tempo_map: Option<TempoMap>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempoMap {
    pub points: Vec<TempoPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoPoint {
    pub beat: Beat,
    pub bpm: Bpm,
}
```

Create `crates/fcs-core/src/v5/parser/document.rs`:

```rust
use crate::v5::ast::{Document, DocumentProfile};

use super::{parse_header, ParseError};

pub fn parse_document(input: &str) -> Result<Document, ParseError> {
    let (rest, source_version) = parse_header(input)?;
    let (format_body, rest) = take_named_block(rest, "format")?;
    if !rest.trim().is_empty() {
        return Err(ParseError::InvalidSyntax("trailing document input"));
    }
    let profile = parse_format_body(format_body)?;
    Ok(Document {
        source_version,
        profile,
        tempo_map: None,
    })
}

fn parse_format_body(body: &str) -> Result<DocumentProfile, ParseError> {
    let body = body.trim();
    let profile = body
        .strip_prefix("profile:")
        .and_then(|value| value.trim().strip_suffix(';'))
        .ok_or(ParseError::InvalidSyntax("format profile"))?
        .trim();
    match profile {
        "fragment" => Ok(DocumentProfile::Fragment),
        "chart" => Ok(DocumentProfile::Chart),
        "playable" => Ok(DocumentProfile::Playable),
        "renderable" => Ok(DocumentProfile::Renderable),
        "publishable" => Ok(DocumentProfile::Publishable),
        _ => Err(ParseError::InvalidSyntax("document profile")),
    }
}

pub(super) fn take_named_block<'a>(
    input: &'a str,
    name: &str,
) -> Result<(&'a str, &'a str), ParseError> {
    let input = input.trim_start();
    let name_end = input
        .strip_prefix(name)
        .ok_or(ParseError::InvalidSyntax("top-level block"))?;
    let leading = input.len() - name_end.len();
    let after_name = name_end.trim_start();
    let whitespace = name_end.len() - after_name.len();
    let after_open = after_name
        .strip_prefix('{')
        .ok_or(ParseError::InvalidSyntax("top-level block"))?;
    let body_start = leading + whitespace + 1;
    let mut depth = 1_u32;
    for (offset, character) in after_open.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let body_end = body_start + offset;
                    let rest_start = body_end + character.len_utf8();
                    return Ok((&input[body_start..body_end], &input[rest_start..]));
                }
            }
            _ => {}
        }
    }
    Err(ParseError::InvalidSyntax("unterminated top-level block"))
}
```

Re-export it from `crates/fcs-core/src/v5/parser/mod.rs`:

```rust
mod document;
pub use document::parse_document;
```

- [ ] **Step 4: Run Clippy and profile tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: PASS.

- [ ] **Step 5: Commit the profile parser**

```text
git add crates/fcs-core/src/v5/ast/mod.rs crates/fcs-core/src/v5/parser crates/fcs-core/tests/fcs5_frontend.rs
git commit -m "feat(parser): parse FCS 5 document profiles"
```

## Task 5: Parse exact beat literals and tempo maps

**Files:**

- Create: `crates/fcs-core/src/v5/parser/tempo.rs`
- Modify: `crates/fcs-core/src/v5/parser/document.rs`
- Modify: `crates/fcs-core/src/v5/parser/mod.rs`
- Test: `crates/fcs-core/tests/fcs5_frontend.rs`

- [ ] **Step 1: Add failing tempo parser tests**

Append:

```rust
#[test]
fn parses_exact_decimal_and_fractional_beats() {
    let source = r#"#fcs 5.0.0
format { profile: chart; }
tempoMap {
    0beat -> 180bpm;
    4.5beat -> 200bpm;
    [8, 1, 3]beat -> 220bpm;
}
"#;
    let document = parse_document(source).unwrap();
    let points = &document.tempo_map.unwrap().points;
    assert_eq!(points[0].beat, Beat::new(0, 1).unwrap());
    assert_eq!(points[1].beat, Beat::new(9, 2).unwrap());
    assert_eq!(points[2].beat, Beat::new(25, 3).unwrap());
}

#[test]
fn rejects_zero_denominator_and_non_positive_bpm_literals() {
    let bad_fraction = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { [1, 1, 0]beat -> 120bpm; }";
    let bad_bpm = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 0bpm; }";
    assert!(parse_document(bad_fraction).is_err());
    assert!(parse_document(bad_bpm).is_err());
}
```

- [ ] **Step 2: Run Clippy and verify failure**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: FAIL because the document parser does not accept a tempo block.

- [ ] **Step 3: Implement exact decimal-to-rational conversion and tempo parsing**

Create `crates/fcs-core/src/v5/parser/tempo.rs` with these public functions and exact decimal conversion:

```rust
use crate::v5::ast::{Beat, Bpm, TempoMap, TempoPoint};

use super::ParseError;

pub(super) fn parse_tempo_map(body: &str) -> Result<TempoMap, ParseError> {
    let mut points = Vec::new();
    for statement in body.split(';').map(str::trim).filter(|value| !value.is_empty()) {
        let (beat, bpm) = statement
            .split_once("->")
            .ok_or(ParseError::InvalidSyntax("tempo point"))?;
        points.push(TempoPoint {
            beat: parse_beat(beat.trim())?,
            bpm: parse_bpm(bpm.trim())?,
        });
    }
    Ok(TempoMap { points })
}

fn parse_beat(value: &str) -> Result<Beat, ParseError> {
    let value = value
        .strip_suffix("beat")
        .ok_or(ParseError::InvalidSyntax("beat unit"))?
        .trim();
    if let Some(array) = value.strip_prefix('[').and_then(|value| value.strip_suffix(']')) {
        let values = array
            .split(',')
            .map(|part| part.trim().parse::<i64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ParseError::InvalidSyntax("fractional beat"))?;
        if values.len() != 3 {
            return Err(ParseError::InvalidSyntax("fractional beat"));
        }
        let fraction = Beat::new(values[1], values[2])
            .map_err(|_| ParseError::InvalidSyntax("fractional beat"))?;
        return Beat::new(values[0], 1)
            .and_then(|whole| whole.checked_add(fraction))
            .map_err(|_| ParseError::InvalidSyntax("fractional beat"));
    }
    parse_decimal_beat(value)
}

fn parse_decimal_beat(value: &str) -> Result<Beat, ParseError> {
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let scale = 10_i64
        .checked_pow(fraction.len() as u32)
        .ok_or(ParseError::InvalidSyntax("beat overflow"))?;
    let whole = whole
        .parse::<i64>()
        .map_err(|_| ParseError::InvalidSyntax("beat"))?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<i64>()
            .map_err(|_| ParseError::InvalidSyntax("beat"))?
    };
    let numerator = whole
        .checked_mul(scale)
        .and_then(|value| value.checked_add(fraction))
        .ok_or(ParseError::InvalidSyntax("beat overflow"))?;
    Beat::new(if negative { -numerator } else { numerator }, scale)
        .map_err(|_| ParseError::InvalidSyntax("beat"))
}

fn parse_bpm(value: &str) -> Result<Bpm, ParseError> {
    let number = value
        .strip_suffix("bpm")
        .ok_or(ParseError::InvalidSyntax("bpm unit"))?
        .trim()
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidSyntax("bpm"))?;
    Bpm::new(number).map_err(|_| ParseError::InvalidSyntax("bpm"))
}
```

Replace `parse_document` in `crates/fcs-core/src/v5/parser/document.rs` with:

```rust
use crate::v5::ast::{Document, DocumentProfile};

use super::{parse_header, tempo::parse_tempo_map, ParseError};

pub fn parse_document(input: &str) -> Result<Document, ParseError> {
    let (rest, source_version) = parse_header(input)?;
    let (format_body, rest) = take_named_block(rest, "format")?;
    let profile = parse_format_body(format_body)?;
    let rest = rest.trim_start();
    let (tempo_map, rest) = if rest.starts_with("tempoMap") {
        let (tempo_body, rest) = take_named_block(rest, "tempoMap")?;
        (Some(parse_tempo_map(tempo_body)?), rest)
    } else {
        (None, rest)
    };
    if !rest.trim().is_empty() {
        return Err(ParseError::InvalidSyntax("trailing document input"));
    }
    Ok(Document {
        source_version,
        profile,
        tempo_map,
    })
}
```

Keep `parse_format_body` and `take_named_block` from Task 4 unchanged. In `crates/fcs-core/src/v5/parser/mod.rs`, add:

```rust
mod tempo;
```

- [ ] **Step 4: Run Clippy and tempo tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: PASS.

- [ ] **Step 5: Commit tempo parsing**

```text
git add crates/fcs-core/src/v5/parser crates/fcs-core/tests/fcs5_frontend.rs
git commit -m "feat(parser): parse exact FCS 5 tempo maps"
```

## Task 6: Validate tempo and profile invariants

**Files:**

- Create: `crates/fcs-core/src/v5/validation.rs`
- Modify: `crates/fcs-core/src/v5/mod.rs`
- Modify: `crates/fcs-core/src/v5/parser/document.rs`
- Test: `crates/fcs-core/tests/fcs5_frontend.rs`

- [ ] **Step 1: Add failing validation tests**

Append:

```rust
#[test]
fn chart_profile_requires_tempo_starting_at_zero() {
    let missing = "#fcs 5.0.0\nformat { profile: chart; }";
    let non_zero = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 1beat -> 120bpm; }";
    assert!(matches!(parse_document(missing), Err(ParseError::MissingRequiredBlock("tempoMap"))));
    assert!(matches!(parse_document(non_zero), Err(ParseError::InvalidTempoMap("first beat must be zero"))));
}

#[test]
fn tempo_points_must_be_non_decreasing() {
    let source = "#fcs 5.0.0\nformat { profile: chart; }\ntempoMap { 0beat -> 120bpm; 4beat -> 180bpm; 3beat -> 200bpm; }";
    assert!(matches!(parse_document(source), Err(ParseError::InvalidTempoMap("beats must be non-decreasing"))));
}
```

- [ ] **Step 2: Run Clippy and verify validation failure**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: FAIL because the new diagnostic variants and validation do not exist.

- [ ] **Step 3: Add validation and diagnostics**

Add these exact variants to `ParseError`:

```rust
MissingRequiredBlock(&'static str),
InvalidTempoMap(&'static str),
```

Create `crates/fcs-core/src/v5/validation.rs`:

```rust
use crate::v5::ast::{Beat, DocumentProfile, TempoMap};

use super::parser::ParseError;

pub fn validate_profile(
    profile: DocumentProfile,
    tempo_map: Option<&TempoMap>,
) -> Result<(), ParseError> {
    if matches!(
        profile,
        DocumentProfile::Chart | DocumentProfile::Playable | DocumentProfile::Publishable
    ) && tempo_map.is_none()
    {
        return Err(ParseError::MissingRequiredBlock("tempoMap"));
    }
    if let Some(tempo_map) = tempo_map {
        validate_tempo_map(tempo_map)?;
    }
    Ok(())
}

fn validate_tempo_map(tempo_map: &TempoMap) -> Result<(), ParseError> {
    let zero = Beat::new(0, 1).expect("constant zero beat is valid");
    if tempo_map.points.first().map(|point| point.beat) != Some(zero) {
        return Err(ParseError::InvalidTempoMap("first beat must be zero"));
    }
    if tempo_map
        .points
        .windows(2)
        .any(|points| points[0].beat > points[1].beat)
    {
        return Err(ParseError::InvalidTempoMap("beats must be non-decreasing"));
    }
    Ok(())
}
```

Add this exact declaration to `crates/fcs-core/src/v5/mod.rs`:

```rust
pub(crate) mod validation;
```

Import validation in `crates/fcs-core/src/v5/parser/document.rs`:

```rust
use crate::v5::validation::validate_profile;
```

Immediately before constructing and returning `Document`, add:

```rust
validate_profile(profile, tempo_map.as_ref())?;
```

- [ ] **Step 4: Run Clippy and validation tests**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: PASS.

- [ ] **Step 5: Commit profile validation**

```text
git add crates/fcs-core/src/v5 crates/fcs-core/tests/fcs5_frontend.rs
git commit -m "feat(core): validate FCS 5 tempo profiles"
```

## Task 7: Add public fixtures and document the staged implementation

**Files:**

- Create: `examples/fcs/fcs5-fragment.fcs`
- Create: `examples/fcs/fcs5-chart.fcs`
- Modify: `crates/fcs-core/tests/fcs5_frontend.rs`
- Modify: `fcs.md`

- [ ] **Step 1: Add failing fixture-loading tests**

Append:

```rust
use std::{fs, path::PathBuf};

fn example(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/fcs")
        .join(name);
    fs::read_to_string(path).unwrap()
}

#[test]
fn parses_public_fcs5_fixtures() {
    let fragment = parse_document(&example("fcs5-fragment.fcs")).unwrap();
    let chart = parse_document(&example("fcs5-chart.fcs")).unwrap();
    assert_eq!(fragment.profile, DocumentProfile::Fragment);
    assert_eq!(chart.profile, DocumentProfile::Chart);
    assert_eq!(chart.tempo_map.unwrap().points.len(), 2);
}
```

- [ ] **Step 2: Run Clippy and verify failure because fixtures are missing**

Run:

```text
cargo clippy -p fcs-core --all-targets -- -D warnings
cargo nextest run -p fcs-core
```

Expected: FAIL with a file-not-found error.

- [ ] **Step 3: Add the fixtures**

Create `examples/fcs/fcs5-fragment.fcs`:

```fcs
#fcs 5.0.0
format { profile: fragment; }
```

Create `examples/fcs/fcs5-chart.fcs`:

```fcs
#fcs 5.0.0
format { profile: chart; }
tempoMap {
    0beat -> 180bpm;
    64beat -> 200bpm;
}
```

At the top of `fcs.md`, immediately after the title, add this exact staged-implementation note:

```markdown
> **FCS 5 implementation status:** The approved FCS 5 design is recorded in
> `docs/superpowers/specs/2026-07-13-fcs5-spec-redesign-design.md`.
> During the staged implementation, the body below still documents the current
> v4 implementation. FCS 5 source files declare `#fcs 5.0.0`; the initial FCBC
> container version is `2.0.0`, and the initial Execution ABI version is `1.0.0`.
```

- [ ] **Step 4: Run fixture test and complete workspace verification**

Run in this order:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo fmt --all -- --check
```

Expected: all commands PASS.

- [ ] **Step 5: Commit fixtures and status documentation**

```text
git add examples/fcs/fcs5-fragment.fcs examples/fcs/fcs5-chart.fcs crates/fcs-core/tests/fcs5_frontend.rs fcs.md
git commit -m "docs: add FCS 5 front-end fixtures"
```

## Phase 1 completion checklist

- [ ] `fcs_core::v5` is public without changing the existing v4 parser API.
- [ ] FCS, FCBC and Execution ABI versions are independent constants.
- [ ] `#fcs 5.0.0` is mandatory for the new parser.
- [ ] Beat keys retain exact rational values.
- [ ] BPM values reject zero, negative and non-finite input.
- [ ] Fragment and chart profiles enforce their Phase 1 invariants.
- [ ] Tempo points start at zero and are non-decreasing; equal-beat step entries remain legal.
- [ ] Public fixtures parse through the public FCS 5 API.
- [ ] Existing v4 converter and tests still compile and pass.
- [ ] Workspace Clippy, nextest and rustfmt checks pass.
