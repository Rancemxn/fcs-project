//! RPE (Re:PhiEdit) JSON types. Ref: phichain + lchzh docs.
use serde::Deserialize;
fn default_offset() -> i32 { 0 }
fn default_rpe_version() -> i32 { 81 }
fn default_above() -> i32 { 1 }

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct RpeBeat(pub i32, pub i32, pub i32);
impl RpeBeat {
    pub fn to_f64(&self) -> f64 {
        let d = if self.1 == 0 && self.2 == 0 { 1 } else if self.2 == 0 { return self.0 as f64; } else { self.2 };
        self.0 as f64 + self.1 as f64 / d as f64
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpeChart {
    #[serde(rename = "BPMList", default)] pub bpm_list: Vec<RpeBpmPoint>,
    #[serde(rename = "META", default)] pub meta: RpeMeta,
    #[serde(default)] pub judge_line_list: Vec<RpeJudgeLine>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpeBpmPoint { pub bpm: f32, pub start_time: RpeBeat }

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct RpeMeta {
    #[serde(rename = "RPEVersion", default = "default_rpe_version")] pub rpe_version: i32,
    #[serde(default)] pub name: String, #[serde(default)] pub composer: String,
    #[serde(default)] pub charter: String, #[serde(default)] pub level: String,
    #[serde(default)] pub song: String, #[serde(default)] pub background: String,
    #[serde(default)] pub id: String,
    #[serde(default = "default_offset")] pub offset: i32,
}

fn deser_layers<'de, D>(d: D) -> Result<Vec<RpeEventLayer>, D::Error>
where D: serde::Deserializer<'de> {
    let l: Option<Vec<Option<RpeEventLayer>>> = Option::deserialize(d)?;
    Ok(l.unwrap_or_default().into_iter().flatten().collect())
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RpeJudgeLine {
    #[serde(rename = "Name")] pub name: String,
    #[serde(rename = "Texture")] pub texture: String,
    pub father: i32, #[serde(rename = "zOrder")] pub z_order: i32,
    #[serde(deserialize_with = "deser_layers")] pub event_layers: Vec<RpeEventLayer>,
    pub notes: Vec<RpeNote>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RpeEventLayer {
    pub speed_events: Vec<RpeSpeedEvent>,
    #[serde(rename = "moveXEvents")] pub move_x_events: Vec<RpeCommonEvent>,
    #[serde(rename = "moveYEvents")] pub move_y_events: Vec<RpeCommonEvent>,
    pub rotate_events: Vec<RpeCommonEvent>,
    pub alpha_events: Vec<RpeCommonEvent<i32>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpeCommonEvent<T: Default = f32> {
    pub start_time: RpeBeat, pub end_time: RpeBeat,
    #[serde(default)] pub start: T, #[serde(default)] pub end: T,
    #[serde(default)] pub easing_type: i32, #[serde(default)] pub bezier: i32,
    #[serde(default)] pub bezier_points: [f32; 4],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpeSpeedEvent {
    pub start_time: RpeBeat, pub end_time: RpeBeat,
    #[serde(default)] pub start: f32, #[serde(default)] pub end: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpeNote {
    pub start_time: RpeBeat, pub end_time: RpeBeat,
    pub position_x: f32, pub speed: f32,
    #[serde(rename = "type")] pub kind: i32,
    #[serde(default = "default_above")] pub above: i32,
    #[serde(default, rename = "isFake")] pub is_fake: i32,
    #[serde(default)] pub alpha: i32, #[serde(default)] pub size: f32,
    #[serde(default)] pub y_offset: f32, #[serde(default)] pub visible_time: f32,
}
