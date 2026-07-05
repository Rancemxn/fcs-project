//! PGR (official Phigros) JSON types — deserialized with serde.
use serde::Deserialize;

fn default_speed() -> f64 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FormatVersion {
    V1 = 1,
    V3 = 3,
    Unknown = -1,
}
impl FormatVersion {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::V1,
            3 => Self::V3,
            _ => Self::Unknown,
        }
    }
}

/// Deserialize a number from either integer or float, returning as f64.
fn de_as_f64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    use serde::de;
    struct NumVisitor;
    impl<'de> de::Visitor<'de> for NumVisitor {
        type Value = f64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number (integer or float)")
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }
    }
    d.deserialize_any(NumVisitor)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgrChart {
    #[serde(alias = "formatVersion")]
    pub format_version: FormatVersion,
    pub offset: f64,
    #[serde(alias = "judgeLineList")]
    pub judge_line_list: Vec<PgrJudgeLine>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgrJudgeLine {
    pub bpm: f64,
    #[serde(default)]
    pub notes_above: Vec<PgrNote>,
    #[serde(default)]
    pub notes_below: Vec<PgrNote>,
    #[serde(default)]
    pub speed_events: Vec<PgrEvent>,
    #[serde(default, alias = "judgeLineDisappearEvents")]
    pub judge_line_disappear_events: Vec<PgrEvent>,
    #[serde(default, alias = "judgeLineMoveEvents")]
    pub judge_line_move_events: Vec<PgrEvent>,
    #[serde(default, alias = "judgeLineRotateEvents")]
    pub judge_line_rotate_events: Vec<PgrEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgrNote {
    #[serde(alias = "type", deserialize_with = "de_as_f64")]
    pub note_type: f64,
    #[serde(deserialize_with = "de_as_f64")]
    pub time: f64,
    #[serde(alias = "positionX")]
    pub position_x: f64,
    #[serde(default, alias = "holdTime", deserialize_with = "de_as_f64")]
    pub hold_time: f64,
    #[serde(default = "default_speed")]
    pub speed: f64,
    #[serde(default, alias = "floorPosition")]
    pub floor_position: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgrEvent {
    #[serde(alias = "startTime", deserialize_with = "de_as_f64")]
    pub start_time: f64,
    #[serde(alias = "endTime", deserialize_with = "de_as_f64")]
    pub end_time: f64,
    /// start/end are absent on speed events (which use `value` instead)
    #[serde(default, deserialize_with = "de_as_f64")]
    pub start: f64,
    #[serde(default, deserialize_with = "de_as_f64")]
    pub end: f64,
    #[serde(default, deserialize_with = "de_as_f64")]
    pub start2: f64,
    #[serde(default, deserialize_with = "de_as_f64")]
    pub end2: f64,
    #[serde(default, deserialize_with = "de_as_f64")]
    pub value: f64,
}

impl<'de> Deserialize<'de> for FormatVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v: i32 = Deserialize::deserialize(deserializer)?;
        Ok(FormatVersion::from_i32(v))
    }
}
