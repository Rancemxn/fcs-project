//! Source-free canonical Render scene owned by Render Profile 1.0.0.
//!
//! I9.3 fixes the scene aggregate that later stages consume: the FCBC
//! RenderSection writer, the semantic draw-list evaluator and the reference
//! rasterizer all read this type and never re-read Render source. Structural
//! facts (forest shape, reference targets, compile-time topology) are validated
//! once here so downstream stages can assume them.

use std::collections::{BTreeMap, BTreeSet};

use crate::{EntityKind, StableId};

/// Output color space of the Render viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalRenderColorSpace {
    LinearSrgb,
    Srgb,
}

impl CanonicalRenderColorSpace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinearSrgb => "linear-srgb",
            Self::Srgb => "srgb",
        }
    }

    /// RenderSection encodes the color space as a fixed enum ordinal.
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::LinearSrgb => 1,
            Self::Srgb => 2,
        }
    }
}

/// Fixed viewport schema: finite, strictly positive logical extent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalViewport {
    width: f64,
    height: f64,
    color_space: CanonicalRenderColorSpace,
}

impl CanonicalViewport {
    pub fn new(
        width: f64,
        height: f64,
        color_space: CanonicalRenderColorSpace,
    ) -> Result<Self, CanonicalRenderError> {
        if !width.is_finite() || width <= 0.0 || !height.is_finite() || height <= 0.0 {
            return Err(CanonicalRenderError::InvalidViewport);
        }
        Ok(Self {
            width,
            height,
            color_space,
        })
    }

    pub const fn width(&self) -> f64 {
        self.width
    }

    pub const fn height(&self) -> f64 {
        self.height
    }

    pub const fn color_space(&self) -> CanonicalRenderColorSpace {
        self.color_space
    }
}

/// The fixed render pass sequence.
///
/// Section 5 fixes the order and section 14.1 pins the ordinals, so ordering
/// is derived from the ordinal rather than from the name: sorting the six
/// names as text would place `aboveNotes` first instead of fifth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalRenderPass {
    Background,
    BehindLines,
    Lines,
    Notes,
    AboveNotes,
    Overlay,
}

impl CanonicalRenderPass {
    /// RenderSection encodes the pass as this fixed ordinal.
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Background => 1,
            Self::BehindLines => 2,
            Self::Lines => 3,
            Self::Notes => 4,
            Self::AboveNotes => 5,
            Self::Overlay => 6,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::BehindLines => "behindLines",
            Self::Lines => "lines",
            Self::Notes => "notes",
            Self::AboveNotes => "aboveNotes",
            Self::Overlay => "overlay",
        }
    }

    /// Resolves a source spelling. The set is closed, and RenderSection cannot
    /// encode anything else, so an unknown name has no representation.
    pub fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "background" => Self::Background,
            "behindLines" => Self::BehindLines,
            "lines" => Self::Lines,
            "notes" => Self::Notes,
            "aboveNotes" => Self::AboveNotes,
            "overlay" => Self::Overlay,
            _ => return None,
        })
    }
}

/// Scene object kinds. Layer is an organizational record, not a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalRenderNodeKind {
    Group,
    ClipGroup,
    Rect,
    RoundedRect,
    Circle,
    Ellipse,
    Line,
    Polyline,
    Polygon,
    Path,
    Image,
    Text,
}

impl CanonicalRenderNodeKind {
    /// RenderSection encodes the node kind as a fixed enum ordinal.
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Group => 1,
            Self::ClipGroup => 2,
            Self::Rect => 3,
            Self::RoundedRect => 4,
            Self::Circle => 5,
            Self::Ellipse => 6,
            Self::Line => 7,
            Self::Polyline => 8,
            Self::Polygon => 9,
            Self::Path => 10,
            Self::Image => 11,
            Self::Text => 12,
        }
    }

    /// Group and ClipGroup carry no geometry and never paint.
    pub const fn is_drawable(self) -> bool {
        !matches!(self, Self::Group | Self::ClipGroup)
    }
}

/// Effective attachment space materialized on every node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRenderAttachment {
    World,
    Screen,
    Line(StableId),
    Note(StableId),
}

impl CanonicalRenderAttachment {
    /// RenderSection encodes the attachment kind as a fixed enum ordinal.
    pub const fn ordinal(&self) -> u16 {
        match self {
            Self::World => 1,
            Self::Screen => 2,
            Self::Line(_) => 3,
            Self::Note(_) => 4,
        }
    }

    pub const fn target(&self) -> Option<&StableId> {
        match self {
            Self::World | Self::Screen => None,
            Self::Line(id) | Self::Note(id) => Some(id),
        }
    }
}

/// Core composite modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalRenderComposite {
    SourceOver,
    Copy,
    Add,
    Multiply,
    Screen,
}

impl CanonicalRenderComposite {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceOver => "sourceOver",
            Self::Copy => "copy",
            Self::Add => "add",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
        }
    }

    pub const fn ordinal(self) -> u16 {
        match self {
            Self::SourceOver => 1,
            Self::Copy => 2,
            Self::Add => 3,
            Self::Multiply => 4,
            Self::Screen => 5,
        }
    }
}

/// Path and clip winding rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalRenderFillRule {
    NonZero,
    EvenOdd,
}

impl CanonicalRenderFillRule {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::NonZero => 1,
            Self::EvenOdd => 2,
        }
    }
}

/// Gradient spread outside `[0,1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalGradientSpread {
    Pad,
    Repeat,
    Reflect,
}

impl CanonicalGradientSpread {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Pad => 1,
            Self::Repeat => 2,
            Self::Reflect => 3,
        }
    }
}

/// Image sampling rule shared by Image nodes and ImagePattern paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalImageSampling {
    Nearest,
    Bilinear,
}

impl CanonicalImageSampling {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Nearest => 1,
            Self::Bilinear => 2,
        }
    }
}

/// ImagePattern tiling axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalImageRepeat {
    None,
    X,
    Y,
    Both,
}

impl CanonicalImageRepeat {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::None => 1,
            Self::X => 2,
            Self::Y => 3,
            Self::Both => 4,
        }
    }
}

/// Stroke end cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalStrokeCap {
    Butt,
    Round,
    Square,
}

impl CanonicalStrokeCap {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Butt => 1,
            Self::Round => 2,
            Self::Square => 3,
        }
    }
}

/// Stroke vertex join.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalStrokeJoin {
    Miter,
    Round,
    Bevel,
}

impl CanonicalStrokeJoin {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Miter => 1,
            Self::Round => 2,
            Self::Bevel => 3,
        }
    }
}

/// Parametric arc sweep direction under FCS Y-up coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalArcDirection {
    Clockwise,
    CounterClockwise,
}

impl CanonicalArcDirection {
    pub const fn ordinal(self) -> u16 {
        match self {
            Self::Clockwise => 1,
            Self::CounterClockwise => 2,
        }
    }
}

/// Half-open chart-time interval during which a node participates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalActiveInterval {
    start: f64,
    end: f64,
    unbounded_before: bool,
    unbounded_after: bool,
}

impl CanonicalActiveInterval {
    /// The default interval: active over the whole chart-time domain.
    pub const fn unbounded() -> Self {
        Self {
            start: 0.0,
            end: 0.0,
            unbounded_before: true,
            unbounded_after: true,
        }
    }

    pub fn bounded(start: f64, end: f64) -> Result<Self, CanonicalRenderError> {
        if !start.is_finite() || !end.is_finite() || start > end {
            return Err(CanonicalRenderError::InvalidActiveInterval);
        }
        Ok(Self {
            start,
            end,
            unbounded_before: false,
            unbounded_after: false,
        })
    }

    pub fn from_bounds(start: Option<f64>, end: Option<f64>) -> Result<Self, CanonicalRenderError> {
        match (start, end) {
            (None, None) => Ok(Self::unbounded()),
            (Some(start), Some(end)) => Self::bounded(start, end),
            (Some(start), None) => {
                if !start.is_finite() {
                    return Err(CanonicalRenderError::InvalidActiveInterval);
                }
                Ok(Self {
                    start,
                    end: 0.0,
                    unbounded_before: false,
                    unbounded_after: true,
                })
            }
            (None, Some(end)) => {
                if !end.is_finite() {
                    return Err(CanonicalRenderError::InvalidActiveInterval);
                }
                Ok(Self {
                    start: 0.0,
                    end,
                    unbounded_before: true,
                    unbounded_after: false,
                })
            }
        }
    }

    pub const fn start(&self) -> f64 {
        self.start
    }

    pub const fn end(&self) -> f64 {
        self.end
    }

    pub const fn unbounded_before(&self) -> bool {
        self.unbounded_before
    }

    pub const fn unbounded_after(&self) -> bool {
        self.unbounded_after
    }

    /// Half-open containment: `start <= t < end` on the bounded sides.
    pub fn contains(&self, chart_time: f64) -> bool {
        if !self.unbounded_before && chart_time < self.start {
            return false;
        }
        if !self.unbounded_after && chart_time >= self.end {
            return false;
        }
        true
    }
}

/// A layer: a pure organizational record with no transform, paint or opacity.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderLayer {
    id: StableId,
    pass: CanonicalRenderPass,
    z_order: i32,
    document_order: u32,
    roots: Vec<usize>,
}

impl CanonicalRenderLayer {
    pub fn new(
        id: StableId,
        pass: CanonicalRenderPass,
        z_order: i32,
        document_order: u32,
        roots: Vec<usize>,
    ) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderLayer {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        Ok(Self {
            id,
            pass,
            z_order,
            document_order,
            roots,
        })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn pass(&self) -> CanonicalRenderPass {
        self.pass
    }

    pub const fn z_order(&self) -> i32 {
        self.z_order
    }

    pub const fn document_order(&self) -> u32 {
        self.document_order
    }

    pub fn roots(&self) -> &[usize] {
        &self.roots
    }
}

/// A scene node with its effective attachment materialized.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderNode {
    id: StableId,
    kind: CanonicalRenderNodeKind,
    parent: Option<usize>,
    layer: usize,
    document_order: u32,
    z_order: i32,
    attachment: CanonicalRenderAttachment,
    active: CanonicalActiveInterval,
    isolate: bool,
    follow_hidden_attachment: bool,
    position: usize,
    origin: usize,
    rotation: usize,
    scale: usize,
    opacity: usize,
    visibility: usize,
    geometry: Option<usize>,
    fill_paint: Option<usize>,
    stroke: Option<usize>,
    clip: Option<usize>,
    composite: CanonicalRenderComposite,
}

/// Constructor input for [`CanonicalRenderNode`], kept separate so the node
/// itself stays immutable and the argument list stays readable.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderNodeSpec {
    pub id: StableId,
    pub kind: CanonicalRenderNodeKind,
    pub parent: Option<usize>,
    pub layer: usize,
    pub document_order: u32,
    pub z_order: i32,
    pub attachment: CanonicalRenderAttachment,
    pub active: CanonicalActiveInterval,
    pub isolate: bool,
    pub follow_hidden_attachment: bool,
    pub position: usize,
    pub origin: usize,
    pub rotation: usize,
    pub scale: usize,
    pub opacity: usize,
    pub visibility: usize,
    pub geometry: Option<usize>,
    pub fill_paint: Option<usize>,
    pub stroke: Option<usize>,
    pub clip: Option<usize>,
    pub composite: CanonicalRenderComposite,
}

impl CanonicalRenderNode {
    pub fn new(spec: CanonicalRenderNodeSpec) -> Result<Self, CanonicalRenderError> {
        if spec.id.namespace() != EntityKind::RenderNode {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if let Some(target) = spec.attachment.target() {
            let expected = match &spec.attachment {
                CanonicalRenderAttachment::Line(_) => EntityKind::Line,
                CanonicalRenderAttachment::Note(_) => EntityKind::Note,
                CanonicalRenderAttachment::World | CanonicalRenderAttachment::Screen => {
                    unreachable!("target() is None for non-target attachments")
                }
            };
            if target.namespace() != expected {
                return Err(CanonicalRenderError::WrongNamespace);
            }
        }
        match spec.kind {
            CanonicalRenderNodeKind::Group => {
                if spec.geometry.is_some() {
                    return Err(CanonicalRenderError::GroupCarriesGeometry);
                }
                if spec.clip.is_some() {
                    return Err(CanonicalRenderError::GroupCarriesClip);
                }
            }
            CanonicalRenderNodeKind::ClipGroup => {
                if spec.geometry.is_some() {
                    return Err(CanonicalRenderError::GroupCarriesGeometry);
                }
                if spec.clip.is_none() {
                    return Err(CanonicalRenderError::ClipGroupWithoutClip);
                }
            }
            _ => {
                if spec.geometry.is_none() {
                    return Err(CanonicalRenderError::DrawableWithoutGeometry);
                }
            }
        }
        if spec.isolate && spec.kind.is_drawable() {
            return Err(CanonicalRenderError::IsolatedDrawable);
        }
        if spec.kind.is_drawable() {
            match spec.kind {
                CanonicalRenderNodeKind::Image
                    if spec.fill_paint.is_some() || spec.stroke.is_some() =>
                {
                    return Err(CanonicalRenderError::ImageCarriesPaint);
                }
                CanonicalRenderNodeKind::Image => {}
                CanonicalRenderNodeKind::Line
                    if spec.fill_paint.is_some() || spec.stroke.is_none() =>
                {
                    return Err(CanonicalRenderError::LinePaintBinding);
                }
                _ if spec.fill_paint.is_none() && spec.stroke.is_none() => {
                    return Err(CanonicalRenderError::DrawableWithoutPaint);
                }
                _ => {}
            }
        } else {
            if spec.fill_paint.is_some() || spec.stroke.is_some() {
                return Err(CanonicalRenderError::GroupCarriesPaint);
            }
            // Section 5: a non-isolated group composites into the parent target
            // directly, so only sourceOver is defined for it; anything else has
            // to declare isolate and go through an offscreen buffer.
            if !spec.isolate && spec.composite != CanonicalRenderComposite::SourceOver {
                return Err(CanonicalRenderError::NonIsolatedGroupComposite);
            }
        }
        // Section 4: the flag exists to ignore the Note render-enabled and
        // visibility gates, which only Note attachment has.
        if spec.follow_hidden_attachment
            && !matches!(spec.attachment, CanonicalRenderAttachment::Note(_))
        {
            return Err(CanonicalRenderError::FollowHiddenWithoutNoteAttachment);
        }
        Ok(Self {
            id: spec.id,
            kind: spec.kind,
            parent: spec.parent,
            layer: spec.layer,
            document_order: spec.document_order,
            z_order: spec.z_order,
            attachment: spec.attachment,
            active: spec.active,
            isolate: spec.isolate,
            follow_hidden_attachment: spec.follow_hidden_attachment,
            position: spec.position,
            origin: spec.origin,
            rotation: spec.rotation,
            scale: spec.scale,
            opacity: spec.opacity,
            visibility: spec.visibility,
            geometry: spec.geometry,
            fill_paint: spec.fill_paint,
            stroke: spec.stroke,
            clip: spec.clip,
            composite: spec.composite,
        })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn kind(&self) -> CanonicalRenderNodeKind {
        self.kind
    }

    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    pub const fn layer(&self) -> usize {
        self.layer
    }

    pub const fn document_order(&self) -> u32 {
        self.document_order
    }

    pub const fn z_order(&self) -> i32 {
        self.z_order
    }

    pub const fn attachment(&self) -> &CanonicalRenderAttachment {
        &self.attachment
    }

    pub const fn active(&self) -> CanonicalActiveInterval {
        self.active
    }

    pub const fn isolate(&self) -> bool {
        self.isolate
    }

    pub const fn follow_hidden_attachment(&self) -> bool {
        self.follow_hidden_attachment
    }

    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn origin(&self) -> usize {
        self.origin
    }

    pub const fn rotation(&self) -> usize {
        self.rotation
    }

    pub const fn scale(&self) -> usize {
        self.scale
    }

    pub const fn opacity(&self) -> usize {
        self.opacity
    }

    pub const fn visibility(&self) -> usize {
        self.visibility
    }

    pub const fn geometry(&self) -> Option<usize> {
        self.geometry
    }

    pub const fn fill_paint(&self) -> Option<usize> {
        self.fill_paint
    }

    pub const fn stroke(&self) -> Option<usize> {
        self.stroke
    }

    pub const fn clip(&self) -> Option<usize> {
        self.clip
    }

    pub const fn composite(&self) -> CanonicalRenderComposite {
        self.composite
    }
}

/// Geometry payloads. Every scalar coordinate is a descriptor index.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalRenderGeometryData {
    Rect {
        origin: usize,
        size: usize,
    },
    RoundedRect {
        origin: usize,
        size: usize,
        radii: [usize; 4],
    },
    Circle {
        center: usize,
        radius: usize,
    },
    Ellipse {
        center: usize,
        radius_x: usize,
        radius_y: usize,
        rotation: usize,
    },
    Line {
        start: usize,
        end: usize,
    },
    Polyline {
        points: Vec<usize>,
    },
    Polygon {
        points: Vec<usize>,
    },
    Path {
        path: usize,
    },
    Image {
        resource: StableId,
        destination: [usize; 4],
        source: Option<[usize; 4]>,
        sampling: CanonicalImageSampling,
    },
    Text {
        glyph_runs: Vec<usize>,
        origin: usize,
    },
}

impl CanonicalRenderGeometryData {
    /// The node kind this payload can be referenced by.
    pub const fn kind(&self) -> CanonicalRenderNodeKind {
        match self {
            Self::Rect { .. } => CanonicalRenderNodeKind::Rect,
            Self::RoundedRect { .. } => CanonicalRenderNodeKind::RoundedRect,
            Self::Circle { .. } => CanonicalRenderNodeKind::Circle,
            Self::Ellipse { .. } => CanonicalRenderNodeKind::Ellipse,
            Self::Line { .. } => CanonicalRenderNodeKind::Line,
            Self::Polyline { .. } => CanonicalRenderNodeKind::Polyline,
            Self::Polygon { .. } => CanonicalRenderNodeKind::Polygon,
            Self::Path { .. } => CanonicalRenderNodeKind::Path,
            Self::Image { .. } => CanonicalRenderNodeKind::Image,
            Self::Text { .. } => CanonicalRenderNodeKind::Text,
        }
    }
}

/// A geometry record referenced by exactly one node.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderGeometry {
    id: StableId,
    data: CanonicalRenderGeometryData,
}

impl CanonicalRenderGeometry {
    pub fn new(
        id: StableId,
        data: CanonicalRenderGeometryData,
    ) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderGeometry {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if let CanonicalRenderGeometryData::Image { resource, .. } = &data
            && resource.namespace() != EntityKind::Resource
        {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        match &data {
            CanonicalRenderGeometryData::Polyline { points } if points.len() < 2 => {
                return Err(CanonicalRenderError::DegeneratePointList);
            }
            CanonicalRenderGeometryData::Polygon { points } if points.len() < 3 => {
                return Err(CanonicalRenderError::DegeneratePointList);
            }
            CanonicalRenderGeometryData::Text { glyph_runs, .. } if glyph_runs.is_empty() => {
                return Err(CanonicalRenderError::EmptyGlyphRunList);
            }
            _ => {}
        }
        Ok(Self { id, data })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn data(&self) -> &CanonicalRenderGeometryData {
        &self.data
    }

    pub const fn kind(&self) -> CanonicalRenderNodeKind {
        self.data.kind()
    }
}

/// One immutable path command. Numeric parameters are descriptor indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalPathCommand {
    MoveTo(usize),
    LineTo(usize),
    QuadraticTo(usize, usize),
    CubicTo(usize, usize, usize),
    Arc {
        center: usize,
        radius: usize,
        start_angle: usize,
        end_angle: usize,
        direction: CanonicalArcDirection,
    },
    EllipseArc {
        center: usize,
        radius_x: usize,
        radius_y: usize,
        rotation: usize,
        start_angle: usize,
        end_angle: usize,
        direction: CanonicalArcDirection,
    },
    Close,
}

/// An immutable path command sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRenderPath {
    id: StableId,
    fill_rule: CanonicalRenderFillRule,
    commands: Vec<CanonicalPathCommand>,
}

impl CanonicalRenderPath {
    pub fn new(
        id: StableId,
        fill_rule: CanonicalRenderFillRule,
        commands: Vec<CanonicalPathCommand>,
    ) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderPath {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if commands.is_empty() {
            return Err(CanonicalRenderError::EmptyPath);
        }
        let mut open = false;
        let mut closed = false;
        let mut has_drawing = false;
        for command in &commands {
            match command {
                CanonicalPathCommand::MoveTo(_) => {
                    open = true;
                    closed = false;
                    has_drawing = false;
                }
                CanonicalPathCommand::Close if !open || closed || !has_drawing => {
                    return Err(CanonicalRenderError::InvalidPathState);
                }
                CanonicalPathCommand::Close => closed = true,
                _ if !open => return Err(CanonicalRenderError::PathWithoutInitialMoveTo),
                _ => {
                    closed = false;
                    has_drawing = true;
                }
            }
        }
        Ok(Self {
            id,
            fill_rule,
            commands,
        })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn fill_rule(&self) -> CanonicalRenderFillRule {
        self.fill_rule
    }

    pub fn commands(&self) -> &[CanonicalPathCommand] {
        &self.commands
    }
}

/// A gradient stop with a compile-time offset and a dynamic color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalGradientStop {
    offset: f64,
    color: usize,
}

impl CanonicalGradientStop {
    pub fn new(offset: f64, color: usize) -> Result<Self, CanonicalRenderError> {
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(CanonicalRenderError::InvalidGradientStop);
        }
        Ok(Self { offset, color })
    }

    pub const fn offset(&self) -> f64 {
        self.offset
    }

    pub const fn color(&self) -> usize {
        self.color
    }
}

/// The fixed four-field ImagePattern transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalPatternTransform {
    pub position: usize,
    pub origin: usize,
    pub rotation: usize,
    pub scale: usize,
}

/// Paint payloads.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalRenderPaintData {
    Solid {
        color: usize,
    },
    LinearGradient {
        start: usize,
        end: usize,
        spread: CanonicalGradientSpread,
        stops: Vec<CanonicalGradientStop>,
    },
    RadialGradient {
        start_center: usize,
        start_radius: usize,
        end_center: usize,
        end_radius: usize,
        spread: CanonicalGradientSpread,
        stops: Vec<CanonicalGradientStop>,
    },
    ImagePattern {
        resource: StableId,
        transform: CanonicalPatternTransform,
        repeat: CanonicalImageRepeat,
        sampling: CanonicalImageSampling,
    },
}

/// A paint record.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderPaint {
    id: StableId,
    data: CanonicalRenderPaintData,
}

impl CanonicalRenderPaint {
    pub fn new(id: StableId, data: CanonicalRenderPaintData) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderPaint {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if let CanonicalRenderPaintData::ImagePattern { resource, .. } = &data
            && resource.namespace() != EntityKind::Resource
        {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        let stops = match &data {
            CanonicalRenderPaintData::LinearGradient { stops, .. }
            | CanonicalRenderPaintData::RadialGradient { stops, .. } => Some(stops),
            _ => None,
        };
        if let Some(stops) = stops {
            if stops.len() < 2 {
                return Err(CanonicalRenderError::InvalidGradientStop);
            }
            // Offsets are compile-time and must be non-decreasing: equal
            // neighbours are an exact color step, a decrease is not ordered.
            if stops
                .windows(2)
                .any(|pair| pair[0].offset() > pair[1].offset())
            {
                return Err(CanonicalRenderError::UnorderedGradientStops);
            }
        }
        Ok(Self { id, data })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn data(&self) -> &CanonicalRenderPaintData {
        &self.data
    }
}

/// A stroke record. `dash` is compile-time and always has an even length.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderStroke {
    id: StableId,
    paint: usize,
    width: usize,
    cap: CanonicalStrokeCap,
    join: CanonicalStrokeJoin,
    miter_limit: f64,
    dash_offset: usize,
    dash: Vec<f64>,
}

impl CanonicalRenderStroke {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: StableId,
        paint: usize,
        width: usize,
        cap: CanonicalStrokeCap,
        join: CanonicalStrokeJoin,
        miter_limit: f64,
        dash_offset: usize,
        dash: Vec<f64>,
    ) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderStroke {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if !miter_limit.is_finite() || miter_limit < 1.0 {
            return Err(CanonicalRenderError::InvalidMiterLimit);
        }
        if !dash.is_empty() {
            if !dash.len().is_multiple_of(2) {
                return Err(CanonicalRenderError::OddDashArray);
            }
            if dash.iter().any(|value| !value.is_finite() || *value < 0.0) {
                return Err(CanonicalRenderError::InvalidDashElement);
            }
            // An all-zero array would make dash-phase advance non-terminating.
            if dash.iter().sum::<f64>() <= 0.0 {
                return Err(CanonicalRenderError::ZeroDashTotal);
            }
        }
        Ok(Self {
            id,
            paint,
            width,
            cap,
            join,
            miter_limit,
            dash_offset,
            dash,
        })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn paint(&self) -> usize {
        self.paint
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn cap(&self) -> CanonicalStrokeCap {
        self.cap
    }

    pub const fn join(&self) -> CanonicalStrokeJoin {
        self.join
    }

    pub const fn miter_limit(&self) -> f64 {
        self.miter_limit
    }

    pub const fn dash_offset(&self) -> usize {
        self.dash_offset
    }

    pub fn dash(&self) -> &[f64] {
        &self.dash
    }
}

/// A clip mask: geometry plus a winding rule, with no paint or composite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalRenderClip {
    id: StableId,
    fill_rule: CanonicalRenderFillRule,
    geometry: usize,
}

impl CanonicalRenderClip {
    pub fn new(
        id: StableId,
        fill_rule: CanonicalRenderFillRule,
        geometry: usize,
    ) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderClip {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        Ok(Self {
            id,
            fill_rule,
            geometry,
        })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn fill_rule(&self) -> CanonicalRenderFillRule {
        self.fill_rule
    }

    pub const fn geometry(&self) -> usize {
        self.geometry
    }
}

/// One positioned glyph inside a compile-time glyph run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalGlyphPlacement {
    pub glyph_id: u32,
    pub x_advance: f64,
    pub y_advance: f64,
    pub x_offset: f64,
    pub y_offset: f64,
}

impl CanonicalGlyphPlacement {
    fn is_finite(&self) -> bool {
        self.x_advance.is_finite()
            && self.y_advance.is_finite()
            && self.x_offset.is_finite()
            && self.y_offset.is_finite()
    }
}

/// A compile-time glyph run bound to an embedded font resource.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalGlyphRun {
    id: StableId,
    font: StableId,
    face_index: u32,
    size: usize,
    run_offset: [f64; 2],
    glyphs: Vec<CanonicalGlyphPlacement>,
}

impl CanonicalGlyphRun {
    pub fn new(
        id: StableId,
        font: StableId,
        face_index: u32,
        size: usize,
        run_offset: [f64; 2],
        glyphs: Vec<CanonicalGlyphPlacement>,
    ) -> Result<Self, CanonicalRenderError> {
        if id.namespace() != EntityKind::RenderGlyphRun {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if font.namespace() != EntityKind::Resource {
            return Err(CanonicalRenderError::WrongNamespace);
        }
        if !run_offset[0].is_finite() || !run_offset[1].is_finite() {
            return Err(CanonicalRenderError::NonFiniteGlyphMetric);
        }
        // Section 14.7 allows a zero glyph count for empty source content, so
        // an empty run is legal and must stay representable.
        if !glyphs.iter().all(CanonicalGlyphPlacement::is_finite) {
            return Err(CanonicalRenderError::NonFiniteGlyphMetric);
        }
        Ok(Self {
            id,
            font,
            face_index,
            size,
            run_offset,
            glyphs,
        })
    }

    pub const fn id(&self) -> &StableId {
        &self.id
    }

    pub const fn font(&self) -> &StableId {
        &self.font
    }

    pub const fn face_index(&self) -> u32 {
        self.face_index
    }

    pub const fn size(&self) -> usize {
        self.size
    }

    pub const fn run_offset(&self) -> [f64; 2] {
        self.run_offset
    }

    pub fn glyphs(&self) -> &[CanonicalGlyphPlacement] {
        &self.glyphs
    }
}

/// The canonical Render scene aggregate.
///
/// Construction validates every structural invariant later stages rely on:
/// the node graph is a forest in parent-before-child order, every reference
/// resolves, node and record kinds agree, single ownership holds, and stable
/// IDs are unique across all Render namespaces.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderScene {
    viewport: CanonicalViewport,
    layers: Vec<CanonicalRenderLayer>,
    nodes: Vec<CanonicalRenderNode>,
    geometries: Vec<CanonicalRenderGeometry>,
    paths: Vec<CanonicalRenderPath>,
    paints: Vec<CanonicalRenderPaint>,
    strokes: Vec<CanonicalRenderStroke>,
    clips: Vec<CanonicalRenderClip>,
    glyph_runs: Vec<CanonicalGlyphRun>,
}

/// Constructor input for [`CanonicalRenderScene`].
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalRenderSceneSpec {
    pub viewport: CanonicalViewport,
    pub layers: Vec<CanonicalRenderLayer>,
    pub nodes: Vec<CanonicalRenderNode>,
    pub geometries: Vec<CanonicalRenderGeometry>,
    pub paths: Vec<CanonicalRenderPath>,
    pub paints: Vec<CanonicalRenderPaint>,
    pub strokes: Vec<CanonicalRenderStroke>,
    pub clips: Vec<CanonicalRenderClip>,
    pub glyph_runs: Vec<CanonicalGlyphRun>,
}

impl CanonicalRenderScene {
    pub fn new(spec: CanonicalRenderSceneSpec) -> Result<Self, CanonicalRenderError> {
        let scene = Self {
            viewport: spec.viewport,
            layers: spec.layers,
            nodes: spec.nodes,
            geometries: spec.geometries,
            paths: spec.paths,
            paints: spec.paints,
            strokes: spec.strokes,
            clips: spec.clips,
            glyph_runs: spec.glyph_runs,
        };
        scene.validate_unique_ids()?;
        scene.validate_layers()?;
        scene.validate_nodes()?;
        scene.validate_records()?;
        scene.validate_single_ownership()?;
        Ok(scene)
    }

    fn validate_unique_ids(&self) -> Result<(), CanonicalRenderError> {
        // Section 14 rejects stable ID 0 and any u64 collision between two
        // Render records "even when the namespace or table differs", so this
        // is one set across every namespace, not one set per namespace.
        let mut seen = BTreeSet::new();
        let ids = self
            .layers
            .iter()
            .map(CanonicalRenderLayer::id)
            .chain(self.nodes.iter().map(CanonicalRenderNode::id))
            .chain(self.geometries.iter().map(CanonicalRenderGeometry::id))
            .chain(self.paths.iter().map(CanonicalRenderPath::id))
            .chain(self.paints.iter().map(CanonicalRenderPaint::id))
            .chain(self.strokes.iter().map(CanonicalRenderStroke::id))
            .chain(self.clips.iter().map(CanonicalRenderClip::id))
            .chain(self.glyph_runs.iter().map(CanonicalGlyphRun::id));
        for id in ids {
            if id.value() == 0 {
                return Err(CanonicalRenderError::ZeroStableId);
            }
            if !seen.insert(id.value()) {
                return Err(CanonicalRenderError::DuplicateStableId);
            }
        }
        Ok(())
    }

    fn validate_layers(&self) -> Result<(), CanonicalRenderError> {
        let mut roots = Owners::new(self.nodes.len());
        for (index, layer) in self.layers.iter().enumerate() {
            if layer.document_order() != index as u32 {
                return Err(CanonicalRenderError::InvalidDocumentOrder);
            }
            for root in layer.roots() {
                roots.claim(*root)?;
                let node = self
                    .nodes
                    .get(*root)
                    .ok_or(CanonicalRenderError::UnresolvedReference)?;
                if node.parent().is_some() {
                    return Err(CanonicalRenderError::LayerRootHasParent);
                }
                if node.layer() != index {
                    return Err(CanonicalRenderError::LayerMembershipMismatch);
                }
            }
        }
        Ok(())
    }

    fn validate_nodes(&self) -> Result<(), CanonicalRenderError> {
        let mut sibling_orders = BTreeMap::<(usize, Option<usize>), Vec<u32>>::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if self.layers.get(node.layer()).is_none() {
                return Err(CanonicalRenderError::UnresolvedReference);
            }
            match node.parent() {
                None => {
                    // A root must be listed by exactly the layer it claims.
                    let layer = &self.layers[node.layer()];
                    if !layer.roots().contains(&index) {
                        return Err(CanonicalRenderError::UnlistedLayerRoot);
                    }
                }
                Some(parent) => {
                    // Parent-before-child ordering makes the graph acyclic by
                    // construction, so no separate cycle walk is needed.
                    if parent >= index {
                        return Err(CanonicalRenderError::ParentNotBeforeChild);
                    }
                    let parent = &self.nodes[parent];
                    if parent.layer() != node.layer() {
                        return Err(CanonicalRenderError::LayerMembershipMismatch);
                    }
                    // Core 1.0 only lets a root override attachment; every
                    // descendant materializes the parent's effective value.
                    if parent.attachment() != node.attachment() {
                        return Err(CanonicalRenderError::AttachmentOverrideBelowRoot);
                    }
                }
            }
            if let Some(geometry) = node.geometry() {
                let record = self
                    .geometries
                    .get(geometry)
                    .ok_or(CanonicalRenderError::UnresolvedReference)?;
                if record.kind() != node.kind() {
                    return Err(CanonicalRenderError::GeometryKindMismatch);
                }
            }
            if let Some(paint) = node.fill_paint()
                && self.paints.get(paint).is_none()
            {
                return Err(CanonicalRenderError::UnresolvedReference);
            }
            if let Some(stroke) = node.stroke()
                && self.strokes.get(stroke).is_none()
            {
                return Err(CanonicalRenderError::UnresolvedReference);
            }
            if let Some(clip) = node.clip()
                && self.clips.get(clip).is_none()
            {
                return Err(CanonicalRenderError::UnresolvedReference);
            }
            sibling_orders
                .entry((node.layer(), node.parent()))
                .or_default()
                .push(node.document_order());
        }
        for orders in sibling_orders.values_mut() {
            orders.sort_unstable();
            if orders
                .iter()
                .enumerate()
                .any(|(index, order)| *order != index as u32)
            {
                return Err(CanonicalRenderError::InvalidDocumentOrder);
            }
        }
        Ok(())
    }

    fn validate_records(&self) -> Result<(), CanonicalRenderError> {
        for geometry in &self.geometries {
            match geometry.data() {
                CanonicalRenderGeometryData::Path { path } => {
                    if self.paths.get(*path).is_none() {
                        return Err(CanonicalRenderError::UnresolvedReference);
                    }
                }
                CanonicalRenderGeometryData::Text { glyph_runs, .. } => {
                    for run in glyph_runs {
                        if self.glyph_runs.get(*run).is_none() {
                            return Err(CanonicalRenderError::UnresolvedReference);
                        }
                    }
                }
                _ => {}
            }
        }
        for stroke in &self.strokes {
            if self.paints.get(stroke.paint()).is_none() {
                return Err(CanonicalRenderError::UnresolvedReference);
            }
        }
        for clip in &self.clips {
            let geometry = self
                .geometries
                .get(clip.geometry())
                .ok_or(CanonicalRenderError::UnresolvedReference)?;
            // Section 14.6 gives a closed allowlist: a clip is a coverage mask,
            // so open and non-fillable kinds are excluded, not just Image/Text.
            if !matches!(
                geometry.kind(),
                CanonicalRenderNodeKind::Rect
                    | CanonicalRenderNodeKind::RoundedRect
                    | CanonicalRenderNodeKind::Circle
                    | CanonicalRenderNodeKind::Ellipse
                    | CanonicalRenderNodeKind::Polygon
                    | CanonicalRenderNodeKind::Path
            ) {
                return Err(CanonicalRenderError::ClipGeometryKindNotAllowed);
            }
            // A Path clip carries its own fill rule, which section 14.6
            // requires to equal the referenced path's.
            if let CanonicalRenderGeometryData::Path { path } = geometry.data() {
                let record = self
                    .paths
                    .get(*path)
                    .ok_or(CanonicalRenderError::UnresolvedReference)?;
                if record.fill_rule() != clip.fill_rule() {
                    return Err(CanonicalRenderError::ClipFillRuleMismatch);
                }
            }
        }
        Ok(())
    }

    /// Section 14.8 fixes one owner per auxiliary record and forbids orphans
    /// and cross-owner sharing. The ownership edges are exactly: a drawable
    /// Node owns its Geometry; `fillPaint` owns a Paint; `strokeRef` owns a
    /// Stroke whose `paintRef` owns another Paint; `clipRef` owns a Clip which
    /// owns a Geometry; a Path Geometry owns its Path; a Text Geometry owns
    /// each GlyphRun it lists.
    fn validate_single_ownership(&self) -> Result<(), CanonicalRenderError> {
        let mut geometries = Owners::new(self.geometries.len());
        let mut paths = Owners::new(self.paths.len());
        let mut paints = Owners::new(self.paints.len());
        let mut strokes = Owners::new(self.strokes.len());
        let mut clips = Owners::new(self.clips.len());
        let mut glyph_runs = Owners::new(self.glyph_runs.len());

        for node in &self.nodes {
            if let Some(geometry) = node.geometry() {
                geometries.claim(geometry)?;
            }
            if let Some(paint) = node.fill_paint() {
                paints.claim(paint)?;
            }
            if let Some(stroke) = node.stroke() {
                strokes.claim(stroke)?;
            }
            if let Some(clip) = node.clip() {
                clips.claim(clip)?;
            }
        }
        for stroke in &self.strokes {
            paints.claim(stroke.paint())?;
        }
        for clip in &self.clips {
            geometries.claim(clip.geometry())?;
        }
        for geometry in &self.geometries {
            match geometry.data() {
                CanonicalRenderGeometryData::Path { path } => paths.claim(*path)?,
                CanonicalRenderGeometryData::Text {
                    glyph_runs: runs, ..
                } => {
                    for run in runs {
                        glyph_runs.claim(*run)?;
                    }
                }
                _ => {}
            }
        }

        geometries.require_all_owned()?;
        paths.require_all_owned()?;
        paints.require_all_owned()?;
        strokes.require_all_owned()?;
        clips.require_all_owned()?;
        glyph_runs.require_all_owned()
    }

    pub const fn viewport(&self) -> CanonicalViewport {
        self.viewport
    }

    pub fn layers(&self) -> &[CanonicalRenderLayer] {
        &self.layers
    }

    pub fn nodes(&self) -> &[CanonicalRenderNode] {
        &self.nodes
    }

    pub fn geometries(&self) -> &[CanonicalRenderGeometry] {
        &self.geometries
    }

    pub fn paths(&self) -> &[CanonicalRenderPath] {
        &self.paths
    }

    pub fn paints(&self) -> &[CanonicalRenderPaint] {
        &self.paints
    }

    pub fn strokes(&self) -> &[CanonicalRenderStroke] {
        &self.strokes
    }

    pub fn clips(&self) -> &[CanonicalRenderClip] {
        &self.clips
    }

    pub fn glyph_runs(&self) -> &[CanonicalGlyphRun] {
        &self.glyph_runs
    }

    /// Rewrites descriptor references after the canonical descriptor table has
    /// interned and reordered its entries.
    pub fn remap_descriptors(&mut self, mapping: &[usize]) -> Result<(), CanonicalRenderError> {
        let remap = |index: &mut usize| {
            *index = *mapping
                .get(*index)
                .ok_or(CanonicalRenderError::UnresolvedReference)?;
            Ok::<_, CanonicalRenderError>(())
        };
        for node in &mut self.nodes {
            for index in [
                &mut node.position,
                &mut node.origin,
                &mut node.rotation,
                &mut node.scale,
                &mut node.opacity,
                &mut node.visibility,
            ] {
                remap(index)?;
            }
        }
        for geometry in &mut self.geometries {
            match &mut geometry.data {
                CanonicalRenderGeometryData::Rect { origin, size }
                | CanonicalRenderGeometryData::RoundedRect { origin, size, .. } => {
                    remap(origin)?;
                    remap(size)?;
                    if let CanonicalRenderGeometryData::RoundedRect { radii, .. } =
                        &mut geometry.data
                    {
                        for index in radii {
                            remap(index)?;
                        }
                    }
                }
                CanonicalRenderGeometryData::Circle { center, radius } => {
                    remap(center)?;
                    remap(radius)?;
                }
                CanonicalRenderGeometryData::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                    rotation,
                } => {
                    remap(center)?;
                    remap(radius_x)?;
                    remap(radius_y)?;
                    remap(rotation)?;
                }
                CanonicalRenderGeometryData::Line { start, end } => {
                    remap(start)?;
                    remap(end)?;
                }
                CanonicalRenderGeometryData::Polyline { points }
                | CanonicalRenderGeometryData::Polygon { points } => {
                    for index in points {
                        remap(index)?;
                    }
                }
                CanonicalRenderGeometryData::Path { .. } => {}
                CanonicalRenderGeometryData::Image {
                    destination,
                    source,
                    ..
                } => {
                    for index in destination {
                        remap(index)?;
                    }
                    if let Some(source) = source {
                        for index in source {
                            remap(index)?;
                        }
                    }
                }
                CanonicalRenderGeometryData::Text { origin, .. } => remap(origin)?,
            }
        }
        for path in &mut self.paths {
            for command in &mut path.commands {
                match command {
                    CanonicalPathCommand::MoveTo(index) | CanonicalPathCommand::LineTo(index) => {
                        remap(index)?
                    }
                    CanonicalPathCommand::QuadraticTo(first, second) => {
                        remap(first)?;
                        remap(second)?;
                    }
                    CanonicalPathCommand::CubicTo(first, second, third) => {
                        remap(first)?;
                        remap(second)?;
                        remap(third)?;
                    }
                    CanonicalPathCommand::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        ..
                    } => {
                        remap(center)?;
                        remap(radius)?;
                        remap(start_angle)?;
                        remap(end_angle)?;
                    }
                    CanonicalPathCommand::EllipseArc {
                        center,
                        radius_x,
                        radius_y,
                        rotation,
                        start_angle,
                        end_angle,
                        ..
                    } => {
                        remap(center)?;
                        remap(radius_x)?;
                        remap(radius_y)?;
                        remap(rotation)?;
                        remap(start_angle)?;
                        remap(end_angle)?;
                    }
                    CanonicalPathCommand::Close => {}
                }
            }
        }
        for paint in &mut self.paints {
            match &mut paint.data {
                CanonicalRenderPaintData::Solid { color } => remap(color)?,
                CanonicalRenderPaintData::LinearGradient {
                    start, end, stops, ..
                } => {
                    remap(start)?;
                    remap(end)?;
                    for stop in stops {
                        remap(&mut stop.color)?;
                    }
                }
                CanonicalRenderPaintData::RadialGradient {
                    start_center,
                    start_radius,
                    end_center,
                    end_radius,
                    stops,
                    ..
                } => {
                    for index in [start_center, start_radius, end_center, end_radius] {
                        remap(index)?;
                    }
                    for stop in stops {
                        remap(&mut stop.color)?;
                    }
                }
                CanonicalRenderPaintData::ImagePattern { transform, .. } => {
                    remap(&mut transform.position)?;
                    remap(&mut transform.origin)?;
                    remap(&mut transform.rotation)?;
                    remap(&mut transform.scale)?;
                }
            }
        }
        for stroke in &mut self.strokes {
            // `paint` is a Render paint-table index, not a descriptor index.
            remap(&mut stroke.width)?;
            remap(&mut stroke.dash_offset)?;
        }
        for glyph_run in &mut self.glyph_runs {
            remap(&mut glyph_run.size)?;
        }
        Ok(())
    }

    /// Layer indices in the fixed draw order `(pass, zOrder, documentOrder, id)`.
    ///
    /// Pass ordering uses the section 14.1 ordinal, which is the sequence
    /// section 5 fixes. It is deliberately not the name's text order: those
    /// two disagree on `aboveNotes`, which sorts first as text and fifth in
    /// the specification.
    pub fn layer_draw_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.layers.len()).collect();
        order.sort_by(|left, right| {
            let left = &self.layers[*left];
            let right = &self.layers[*right];
            left.pass()
                .ordinal()
                .cmp(&right.pass().ordinal())
                .then(left.z_order().cmp(&right.z_order()))
                .then(left.document_order().cmp(&right.document_order()))
                .then(left.id().value().cmp(&right.id().value()))
        });
        order
    }
}

/// Tracks the single owner of one auxiliary record table.
struct Owners {
    owned: Vec<bool>,
}

impl Owners {
    fn new(len: usize) -> Self {
        Self {
            owned: vec![false; len],
        }
    }

    fn claim(&mut self, index: usize) -> Result<(), CanonicalRenderError> {
        let owned = self
            .owned
            .get_mut(index)
            .ok_or(CanonicalRenderError::UnresolvedReference)?;
        if *owned {
            return Err(CanonicalRenderError::SharedRecord);
        }
        *owned = true;
        Ok(())
    }

    fn require_all_owned(&self) -> Result<(), CanonicalRenderError> {
        if self.owned.iter().any(|owned| !owned) {
            return Err(CanonicalRenderError::UnreachableRecord);
        }
        Ok(())
    }
}

/// Structural rejection reasons for canonical Render scene construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanonicalRenderError {
    InvalidViewport,
    InvalidActiveInterval,
    InvalidDocumentOrder,
    WrongNamespace,
    ZeroStableId,
    DuplicateStableId,
    UnresolvedReference,
    ParentNotBeforeChild,
    LayerMembershipMismatch,
    LayerRootHasParent,
    UnlistedLayerRoot,
    AttachmentOverrideBelowRoot,
    GroupCarriesGeometry,
    GroupCarriesPaint,
    GroupCarriesClip,
    ClipGroupWithoutClip,
    DrawableWithoutGeometry,
    DrawableWithoutPaint,
    ImageCarriesPaint,
    LinePaintBinding,
    GeometryKindMismatch,
    ClipGeometryKindNotAllowed,
    ClipFillRuleMismatch,
    IsolatedDrawable,
    NonIsolatedGroupComposite,
    FollowHiddenWithoutNoteAttachment,
    SharedRecord,
    UnreachableRecord,
    DegeneratePointList,
    EmptyGlyphRunList,
    NonFiniteGlyphMetric,
    EmptyPath,
    PathWithoutInitialMoveTo,
    InvalidPathState,
    InvalidGradientStop,
    UnorderedGradientStops,
    InvalidMiterLimit,
    OddDashArray,
    InvalidDashElement,
    ZeroDashTotal,
}

// Stable Render diagnostic categories fixed by `fcs-render.md` section 16.
pub const RENDER_DIAGNOSTIC_UNSUPPORTED_PROFILE: &str = "render.unsupported-profile";
pub const RENDER_DIAGNOSTIC_INVALID_SECTION: &str = "render.invalid-section";
pub const RENDER_DIAGNOSTIC_INVALID_RECORD: &str = "render.invalid-record";
pub const RENDER_DIAGNOSTIC_RESOURCE_NOT_FOUND: &str = "render.resource-not-found";
pub const RENDER_DIAGNOSTIC_RESOURCE_TYPE_MISMATCH: &str = "render.resource-type-mismatch";
pub const RENDER_DIAGNOSTIC_RESOURCE_DECODE_FAILED: &str = "render.resource-decode-failed";
pub const RENDER_DIAGNOSTIC_RESOURCE_CAPABILITY_MISSING: &str =
    "render.resource-capability-missing";
pub const RENDER_DIAGNOSTIC_INVALID_REFERENCE: &str = "render.invalid-reference";
pub const RENDER_DIAGNOSTIC_INVALID_GEOMETRY: &str = "render.invalid-geometry";
pub const RENDER_DIAGNOSTIC_INVALID_PAINT: &str = "render.invalid-paint";
pub const RENDER_DIAGNOSTIC_INVALID_STROKE: &str = "render.invalid-stroke";
pub const RENDER_DIAGNOSTIC_INVALID_CLIP: &str = "render.invalid-clip";
pub const RENDER_DIAGNOSTIC_INVALID_COMPOSITE: &str = "render.invalid-composite";
pub const RENDER_DIAGNOSTIC_INVALID_GRAPH: &str = "render.invalid-graph";
pub const RENDER_DIAGNOSTIC_INVALID_DESCRIPTOR: &str = "render.invalid-descriptor";
pub const RENDER_DIAGNOSTIC_LIMIT_EXCEEDED: &str = "render.limit-exceeded";

/// The closed list of Render diagnostic categories.
pub const RENDER_DIAGNOSTIC_CATEGORIES: [&str; 16] = [
    RENDER_DIAGNOSTIC_UNSUPPORTED_PROFILE,
    RENDER_DIAGNOSTIC_INVALID_SECTION,
    RENDER_DIAGNOSTIC_INVALID_RECORD,
    RENDER_DIAGNOSTIC_RESOURCE_NOT_FOUND,
    RENDER_DIAGNOSTIC_RESOURCE_TYPE_MISMATCH,
    RENDER_DIAGNOSTIC_RESOURCE_DECODE_FAILED,
    RENDER_DIAGNOSTIC_RESOURCE_CAPABILITY_MISSING,
    RENDER_DIAGNOSTIC_INVALID_REFERENCE,
    RENDER_DIAGNOSTIC_INVALID_GEOMETRY,
    RENDER_DIAGNOSTIC_INVALID_PAINT,
    RENDER_DIAGNOSTIC_INVALID_STROKE,
    RENDER_DIAGNOSTIC_INVALID_CLIP,
    RENDER_DIAGNOSTIC_INVALID_COMPOSITE,
    RENDER_DIAGNOSTIC_INVALID_GRAPH,
    RENDER_DIAGNOSTIC_INVALID_DESCRIPTOR,
    RENDER_DIAGNOSTIC_LIMIT_EXCEEDED,
];

impl CanonicalRenderError {
    /// The stable Render diagnostic category this rejection maps to.
    ///
    /// The mapping follows the ordered failure-surface table in
    /// `fcs-render.md` section 16, so a rejection reports the same category the
    /// FCBC Render validator would report for the same fact.
    pub const fn code(self) -> &'static str {
        match self {
            // Row 3: viewport width/height/colorSpace belong to the section header.
            Self::InvalidViewport => RENDER_DIAGNOSTIC_INVALID_SECTION,
            // Row 5: duplicate or zero ID, Layer pass and root range, Node
            // active interval, parent, layer and order, cycles, orphan
            // ownership and cross-owner sharing.
            Self::InvalidActiveInterval
            | Self::InvalidDocumentOrder
            | Self::WrongNamespace
            | Self::ZeroStableId
            | Self::DuplicateStableId
            | Self::ParentNotBeforeChild
            | Self::LayerMembershipMismatch
            | Self::LayerRootHasParent
            | Self::UnlistedLayerRoot
            | Self::SharedRecord
            | Self::UnreachableRecord => RENDER_DIAGNOSTIC_INVALID_GRAPH,
            // Row 6: table reference bounds, nullability and kind
            // incompatibility, and attachment target/kind.
            Self::UnresolvedReference
            | Self::AttachmentOverrideBelowRoot
            | Self::GroupCarriesGeometry
            | Self::GroupCarriesPaint
            | Self::GroupCarriesClip
            | Self::ClipGroupWithoutClip
            | Self::DrawableWithoutGeometry
            | Self::DrawableWithoutPaint
            | Self::ImageCarriesPaint
            | Self::LinePaintBinding
            | Self::FollowHiddenWithoutNoteAttachment => RENDER_DIAGNOSTIC_INVALID_REFERENCE,
            // Row 7: Node/Geometry kind, path state and compile-time geometry
            // ranges. Glyph run problems are geometry too once the font
            // decoded, and must not fall back to a resource category.
            Self::GeometryKindMismatch
            | Self::DegeneratePointList
            | Self::EmptyGlyphRunList
            | Self::NonFiniteGlyphMetric
            | Self::EmptyPath
            | Self::PathWithoutInitialMoveTo
            | Self::InvalidPathState => RENDER_DIAGNOSTIC_INVALID_GEOMETRY,
            // Row 8 and row 9.
            Self::InvalidGradientStop | Self::UnorderedGradientStops => {
                RENDER_DIAGNOSTIC_INVALID_PAINT
            }
            Self::InvalidMiterLimit
            | Self::OddDashArray
            | Self::InvalidDashElement
            | Self::ZeroDashTotal => RENDER_DIAGNOSTIC_INVALID_STROKE,
            // Row 11: composite enum and isolate applicability.
            Self::IsolatedDrawable | Self::NonIsolatedGroupComposite => {
                RENDER_DIAGNOSTIC_INVALID_COMPOSITE
            }
            // Row 10: clip fill rule, the geometry kinds a clip may use, and
            // Path fill-rule consistency.
            Self::ClipGeometryKindNotAllowed | Self::ClipFillRuleMismatch => {
                RENDER_DIAGNOSTIC_INVALID_CLIP
            }
        }
    }

    /// The next variant in declaration order, or `None` at the end.
    ///
    /// This match has no wildcard arm, so adding a variant without extending
    /// the chain fails to compile. That is what makes the category guard
    /// exhaustive by construction instead of by hand.
    const fn next_variant(self) -> Option<Self> {
        Some(match self {
            Self::InvalidViewport => Self::InvalidActiveInterval,
            Self::InvalidActiveInterval => Self::InvalidDocumentOrder,
            Self::InvalidDocumentOrder => Self::WrongNamespace,
            Self::WrongNamespace => Self::ZeroStableId,
            Self::ZeroStableId => Self::DuplicateStableId,
            Self::DuplicateStableId => Self::UnresolvedReference,
            Self::UnresolvedReference => Self::ParentNotBeforeChild,
            Self::ParentNotBeforeChild => Self::LayerMembershipMismatch,
            Self::LayerMembershipMismatch => Self::LayerRootHasParent,
            Self::LayerRootHasParent => Self::UnlistedLayerRoot,
            Self::UnlistedLayerRoot => Self::AttachmentOverrideBelowRoot,
            Self::AttachmentOverrideBelowRoot => Self::GroupCarriesGeometry,
            Self::GroupCarriesGeometry => Self::GroupCarriesPaint,
            Self::GroupCarriesPaint => Self::GroupCarriesClip,
            Self::GroupCarriesClip => Self::ClipGroupWithoutClip,
            Self::ClipGroupWithoutClip => Self::DrawableWithoutGeometry,
            Self::DrawableWithoutGeometry => Self::DrawableWithoutPaint,
            Self::DrawableWithoutPaint => Self::ImageCarriesPaint,
            Self::ImageCarriesPaint => Self::LinePaintBinding,
            Self::LinePaintBinding => Self::GeometryKindMismatch,
            Self::GeometryKindMismatch => Self::ClipGeometryKindNotAllowed,
            Self::ClipGeometryKindNotAllowed => Self::ClipFillRuleMismatch,
            Self::ClipFillRuleMismatch => Self::IsolatedDrawable,
            Self::IsolatedDrawable => Self::NonIsolatedGroupComposite,
            Self::NonIsolatedGroupComposite => Self::FollowHiddenWithoutNoteAttachment,
            Self::FollowHiddenWithoutNoteAttachment => Self::SharedRecord,
            Self::SharedRecord => Self::UnreachableRecord,
            Self::UnreachableRecord => Self::DegeneratePointList,
            Self::DegeneratePointList => Self::EmptyGlyphRunList,
            Self::EmptyGlyphRunList => Self::NonFiniteGlyphMetric,
            Self::NonFiniteGlyphMetric => Self::EmptyPath,
            Self::EmptyPath => Self::PathWithoutInitialMoveTo,
            Self::PathWithoutInitialMoveTo => Self::InvalidPathState,
            Self::InvalidPathState => Self::InvalidGradientStop,
            Self::InvalidGradientStop => Self::UnorderedGradientStops,
            Self::UnorderedGradientStops => Self::InvalidMiterLimit,
            Self::InvalidMiterLimit => Self::OddDashArray,
            Self::OddDashArray => Self::InvalidDashElement,
            Self::InvalidDashElement => Self::ZeroDashTotal,
            Self::ZeroDashTotal => return None,
        })
    }

    /// Every variant, walked through [`Self::next_variant`].
    pub fn all() -> Vec<Self> {
        let mut variants = Vec::new();
        let mut current = Some(Self::InvalidViewport);
        while let Some(variant) = current {
            variants.push(variant);
            current = variant.next_variant();
        }
        variants
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalTextualId, StableIdRegistry};

    fn id(registry: &mut StableIdRegistry, kind: EntityKind, textual: &str) -> StableId {
        registry
            .insert(
                kind,
                CanonicalTextualId::explicit(textual).expect("textual id"),
            )
            .expect("stable id")
    }

    struct Fixture {
        registry: StableIdRegistry,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                registry: StableIdRegistry::new(),
            }
        }

        fn id(&mut self, kind: EntityKind, textual: &str) -> StableId {
            id(&mut self.registry, kind, textual)
        }

        /// One layer holding one Rect root, the smallest complete scene.
        fn solid_rect(&mut self) -> CanonicalRenderSceneSpec {
            let layer_id = self.id(EntityKind::RenderLayer, "layer/main");
            let node_id = self.id(EntityKind::RenderNode, "layer/main/full");
            let geometry_id = self.id(EntityKind::RenderGeometry, "layer/main/full/geometry");
            let paint_id = self.id(EntityKind::RenderPaint, "layer/main/full/fill");

            let layer =
                CanonicalRenderLayer::new(layer_id, CanonicalRenderPass::Overlay, 0, 0, vec![0])
                    .expect("layer");
            let geometry = CanonicalRenderGeometry::new(
                geometry_id,
                CanonicalRenderGeometryData::Rect { origin: 0, size: 1 },
            )
            .expect("geometry");
            let paint =
                CanonicalRenderPaint::new(paint_id, CanonicalRenderPaintData::Solid { color: 2 })
                    .expect("paint");
            let node = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                id: node_id,
                kind: CanonicalRenderNodeKind::Rect,
                parent: None,
                layer: 0,
                document_order: 0,
                z_order: 0,
                attachment: CanonicalRenderAttachment::Screen,
                active: CanonicalActiveInterval::unbounded(),
                isolate: false,
                follow_hidden_attachment: false,
                position: 3,
                origin: 4,
                rotation: 5,
                scale: 6,
                opacity: 7,
                visibility: 8,
                geometry: Some(0),
                fill_paint: Some(0),
                stroke: None,
                clip: None,
                composite: CanonicalRenderComposite::SourceOver,
            })
            .expect("node");

            CanonicalRenderSceneSpec {
                viewport: CanonicalViewport::new(4.0, 4.0, CanonicalRenderColorSpace::LinearSrgb)
                    .expect("viewport"),
                layers: vec![layer],
                nodes: vec![node],
                geometries: vec![geometry],
                paths: Vec::new(),
                paints: vec![paint],
                strokes: Vec::new(),
                clips: Vec::new(),
                glyph_runs: Vec::new(),
            }
        }
    }

    #[test]
    fn minimal_scene_validates() {
        let mut fixture = Fixture::new();
        let spec = fixture.solid_rect();
        let scene = CanonicalRenderScene::new(spec).expect("scene");
        assert_eq!(scene.layers().len(), 1);
        assert_eq!(scene.nodes().len(), 1);
        assert_eq!(scene.layer_draw_order(), vec![0]);
        assert_eq!(scene.viewport().width(), 4.0);
    }

    #[test]
    fn viewport_rejects_non_positive_and_non_finite_extent() {
        for (width, height) in [
            (0.0, 4.0),
            (4.0, 0.0),
            (-1.0, 4.0),
            (f64::NAN, 4.0),
            (f64::INFINITY, 4.0),
        ] {
            assert_eq!(
                CanonicalViewport::new(width, height, CanonicalRenderColorSpace::Srgb),
                Err(CanonicalRenderError::InvalidViewport),
                "width={width} height={height}"
            );
        }
    }

    #[test]
    fn active_interval_is_half_open() {
        let interval = CanonicalActiveInterval::bounded(1.0, 2.0).expect("interval");
        assert!(!interval.contains(0.999));
        assert!(interval.contains(1.0));
        assert!(interval.contains(1.999));
        assert!(!interval.contains(2.0));
        assert!(CanonicalActiveInterval::unbounded().contains(f64::MIN));
        let empty = CanonicalActiveInterval::bounded(2.0, 2.0).expect("empty interval");
        assert!(!empty.contains(1.999));
        assert!(!empty.contains(2.0));
        assert!(!empty.contains(2.001));
        assert_eq!(
            CanonicalActiveInterval::bounded(2.0, 1.0),
            Err(CanonicalRenderError::InvalidActiveInterval)
        );
    }

    #[test]
    fn group_cannot_carry_geometry_or_paint() {
        let mut fixture = Fixture::new();
        let node_id = fixture.id(EntityKind::RenderNode, "layer/main/group");
        let spec = CanonicalRenderNodeSpec {
            id: node_id,
            kind: CanonicalRenderNodeKind::Group,
            parent: None,
            layer: 0,
            document_order: 0,
            z_order: 0,
            attachment: CanonicalRenderAttachment::World,
            active: CanonicalActiveInterval::unbounded(),
            isolate: false,
            follow_hidden_attachment: false,
            position: 0,
            origin: 1,
            rotation: 2,
            scale: 3,
            opacity: 4,
            visibility: 5,
            geometry: Some(0),
            fill_paint: None,
            stroke: None,
            clip: None,
            composite: CanonicalRenderComposite::SourceOver,
        };
        assert_eq!(
            CanonicalRenderNode::new(spec.clone()),
            Err(CanonicalRenderError::GroupCarriesGeometry)
        );
        let painted = CanonicalRenderNodeSpec {
            geometry: None,
            fill_paint: Some(0),
            ..spec.clone()
        };
        assert_eq!(
            CanonicalRenderNode::new(painted),
            Err(CanonicalRenderError::GroupCarriesPaint)
        );
        let clipped = CanonicalRenderNodeSpec {
            clip: Some(0),
            ..spec
        };
        assert_eq!(
            CanonicalRenderNode::new(clipped),
            Err(CanonicalRenderError::GroupCarriesClip)
        );
    }

    #[test]
    fn drawable_paint_bindings_match_loader_rules() {
        let mut fixture = Fixture::new();
        let node_id = fixture.id(EntityKind::RenderNode, "layer/main/drawable");
        let base = CanonicalRenderNodeSpec {
            id: node_id,
            kind: CanonicalRenderNodeKind::Rect,
            parent: None,
            layer: 0,
            document_order: 0,
            z_order: 0,
            attachment: CanonicalRenderAttachment::World,
            active: CanonicalActiveInterval::unbounded(),
            isolate: false,
            follow_hidden_attachment: false,
            position: 0,
            origin: 1,
            rotation: 2,
            scale: 3,
            opacity: 4,
            visibility: 5,
            geometry: Some(0),
            fill_paint: Some(0),
            stroke: None,
            clip: None,
            composite: CanonicalRenderComposite::SourceOver,
        };
        assert_eq!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                isolate: true,
                ..base.clone()
            }),
            Err(CanonicalRenderError::IsolatedDrawable)
        );
        assert!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                kind: CanonicalRenderNodeKind::Image,
                fill_paint: None,
                ..base.clone()
            })
            .is_ok()
        );
        assert_eq!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                kind: CanonicalRenderNodeKind::Image,
                ..base.clone()
            }),
            Err(CanonicalRenderError::ImageCarriesPaint)
        );
        assert_eq!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                kind: CanonicalRenderNodeKind::Line,
                fill_paint: None,
                ..base.clone()
            }),
            Err(CanonicalRenderError::LinePaintBinding)
        );
        assert_eq!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                fill_paint: None,
                ..base
            }),
            Err(CanonicalRenderError::DrawableWithoutPaint)
        );
    }

    #[test]
    fn clip_group_requires_a_clip() {
        let mut fixture = Fixture::new();
        let node_id = fixture.id(EntityKind::RenderNode, "layer/main/clip");
        assert_eq!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                id: node_id,
                kind: CanonicalRenderNodeKind::ClipGroup,
                parent: None,
                layer: 0,
                document_order: 0,
                z_order: 0,
                attachment: CanonicalRenderAttachment::World,
                active: CanonicalActiveInterval::unbounded(),
                isolate: false,
                follow_hidden_attachment: false,
                position: 0,
                origin: 1,
                rotation: 2,
                scale: 3,
                opacity: 4,
                visibility: 5,
                geometry: None,
                fill_paint: None,
                stroke: None,
                clip: None,
                composite: CanonicalRenderComposite::SourceOver,
            }),
            Err(CanonicalRenderError::ClipGroupWithoutClip)
        );
    }

    #[test]
    fn drawable_without_geometry_is_rejected() {
        let mut fixture = Fixture::new();
        let node_id = fixture.id(EntityKind::RenderNode, "layer/main/rect");
        assert_eq!(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                id: node_id,
                kind: CanonicalRenderNodeKind::Rect,
                parent: None,
                layer: 0,
                document_order: 0,
                z_order: 0,
                attachment: CanonicalRenderAttachment::World,
                active: CanonicalActiveInterval::unbounded(),
                isolate: false,
                follow_hidden_attachment: false,
                position: 0,
                origin: 1,
                rotation: 2,
                scale: 3,
                opacity: 4,
                visibility: 5,
                geometry: None,
                fill_paint: None,
                stroke: None,
                clip: None,
                composite: CanonicalRenderComposite::SourceOver,
            }),
            Err(CanonicalRenderError::DrawableWithoutGeometry)
        );
    }

    #[test]
    fn child_cannot_precede_its_parent() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let child_id = fixture.id(EntityKind::RenderNode, "layer/main/child");
        let child = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
            id: child_id,
            kind: CanonicalRenderNodeKind::Group,
            parent: Some(1),
            layer: 0,
            document_order: 1,
            z_order: 0,
            attachment: CanonicalRenderAttachment::Screen,
            active: CanonicalActiveInterval::unbounded(),
            isolate: false,
            follow_hidden_attachment: false,
            position: 3,
            origin: 4,
            rotation: 5,
            scale: 6,
            opacity: 7,
            visibility: 8,
            geometry: None,
            fill_paint: None,
            stroke: None,
            clip: None,
            composite: CanonicalRenderComposite::SourceOver,
        })
        .expect("child");
        // Index 1 refers to itself, which is exactly the self-parent cycle the
        // parent-before-child rule exists to reject.
        spec.nodes.push(child);
        assert_eq!(
            CanonicalRenderScene::new(spec),
            Err(CanonicalRenderError::ParentNotBeforeChild)
        );
    }

    #[test]
    fn descendant_cannot_override_attachment() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let child_id = fixture.id(EntityKind::RenderNode, "layer/main/child");
        let child = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
            id: child_id,
            kind: CanonicalRenderNodeKind::Group,
            parent: Some(0),
            layer: 0,
            document_order: 1,
            z_order: 0,
            attachment: CanonicalRenderAttachment::World,
            active: CanonicalActiveInterval::unbounded(),
            isolate: false,
            follow_hidden_attachment: false,
            position: 3,
            origin: 4,
            rotation: 5,
            scale: 6,
            opacity: 7,
            visibility: 8,
            geometry: None,
            fill_paint: None,
            stroke: None,
            clip: None,
            composite: CanonicalRenderComposite::SourceOver,
        })
        .expect("child");
        spec.nodes.push(child);
        assert_eq!(
            CanonicalRenderScene::new(spec),
            Err(CanonicalRenderError::AttachmentOverrideBelowRoot)
        );
    }

    #[test]
    fn geometry_kind_must_match_the_node() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let geometry_id = fixture.id(EntityKind::RenderGeometry, "layer/main/other");
        spec.geometries[0] = CanonicalRenderGeometry::new(
            geometry_id,
            CanonicalRenderGeometryData::Circle {
                center: 0,
                radius: 1,
            },
        )
        .expect("geometry");
        assert_eq!(
            CanonicalRenderScene::new(spec),
            Err(CanonicalRenderError::GeometryKindMismatch)
        );
    }

    #[test]
    fn repeated_layer_roots_are_rejected() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let layer = spec.layers[0].clone();
        spec.layers[0] = CanonicalRenderLayer::new(
            layer.id().clone(),
            layer.pass(),
            layer.z_order(),
            layer.document_order(),
            vec![0, 0],
        )
        .expect("layer");
        assert_eq!(
            CanonicalRenderScene::new(spec).expect_err("repeated root"),
            CanonicalRenderError::SharedRecord
        );
    }

    #[test]
    fn unreferenced_geometry_is_rejected() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let geometry_id = fixture.id(EntityKind::RenderGeometry, "layer/main/orphan");
        spec.geometries.push(
            CanonicalRenderGeometry::new(
                geometry_id,
                CanonicalRenderGeometryData::Circle {
                    center: 0,
                    radius: 1,
                },
            )
            .expect("geometry"),
        );
        assert_eq!(
            CanonicalRenderScene::new(spec),
            Err(CanonicalRenderError::UnreachableRecord)
        );
    }

    #[test]
    fn gradient_stops_must_be_ordered_and_at_least_two() {
        let mut fixture = Fixture::new();
        let paint_id = fixture.id(EntityKind::RenderPaint, "paint/gradient");
        let single = vec![CanonicalGradientStop::new(0.0, 0).expect("stop")];
        assert_eq!(
            CanonicalRenderPaint::new(
                paint_id.clone(),
                CanonicalRenderPaintData::LinearGradient {
                    start: 0,
                    end: 1,
                    spread: CanonicalGradientSpread::Pad,
                    stops: single,
                },
            ),
            Err(CanonicalRenderError::InvalidGradientStop)
        );
        let unordered = vec![
            CanonicalGradientStop::new(0.75, 0).expect("stop"),
            CanonicalGradientStop::new(0.25, 1).expect("stop"),
        ];
        assert_eq!(
            CanonicalRenderPaint::new(
                paint_id,
                CanonicalRenderPaintData::LinearGradient {
                    start: 0,
                    end: 1,
                    spread: CanonicalGradientSpread::Pad,
                    stops: unordered,
                },
            ),
            Err(CanonicalRenderError::UnorderedGradientStops)
        );
        assert_eq!(
            CanonicalGradientStop::new(1.5, 0),
            Err(CanonicalRenderError::InvalidGradientStop)
        );
    }

    #[test]
    fn equal_gradient_offsets_are_an_exact_color_step() {
        let mut fixture = Fixture::new();
        let paint_id = fixture.id(EntityKind::RenderPaint, "paint/step");
        let stops = vec![
            CanonicalGradientStop::new(0.0, 0).expect("stop"),
            CanonicalGradientStop::new(0.5, 1).expect("stop"),
            CanonicalGradientStop::new(0.5, 2).expect("stop"),
            CanonicalGradientStop::new(1.0, 3).expect("stop"),
        ];
        assert!(
            CanonicalRenderPaint::new(
                paint_id,
                CanonicalRenderPaintData::LinearGradient {
                    start: 0,
                    end: 1,
                    spread: CanonicalGradientSpread::Repeat,
                    stops,
                },
            )
            .is_ok()
        );
    }

    #[test]
    fn stroke_rejects_odd_zero_and_negative_dash_arrays() {
        let mut fixture = Fixture::new();
        let stroke_id = fixture.id(EntityKind::RenderStroke, "stroke/main");
        let build = |dash: Vec<f64>, miter: f64| {
            CanonicalRenderStroke::new(
                stroke_id.clone(),
                0,
                1,
                CanonicalStrokeCap::Butt,
                CanonicalStrokeJoin::Miter,
                miter,
                2,
                dash,
            )
        };
        assert_eq!(
            build(vec![1.0], 4.0),
            Err(CanonicalRenderError::OddDashArray)
        );
        assert_eq!(
            build(vec![0.0, 0.0], 4.0),
            Err(CanonicalRenderError::ZeroDashTotal)
        );
        assert_eq!(
            build(vec![-1.0, 2.0], 4.0),
            Err(CanonicalRenderError::InvalidDashElement)
        );
        assert_eq!(
            build(vec![1.0, 1.0], 0.5),
            Err(CanonicalRenderError::InvalidMiterLimit)
        );
        // A zero-length element is legal as long as the total is positive.
        assert!(build(vec![0.0, 2.0], 1.0).is_ok());
        assert!(build(Vec::new(), 1.0).is_ok());
    }

    #[test]
    fn path_requires_an_initial_move_to() {
        let mut fixture = Fixture::new();
        let path_id = fixture.id(EntityKind::RenderPath, "path/main");
        assert_eq!(
            CanonicalRenderPath::new(
                path_id.clone(),
                CanonicalRenderFillRule::NonZero,
                vec![CanonicalPathCommand::LineTo(0)],
            ),
            Err(CanonicalRenderError::PathWithoutInitialMoveTo)
        );
        assert_eq!(
            CanonicalRenderPath::new(
                path_id.clone(),
                CanonicalRenderFillRule::NonZero,
                Vec::new()
            ),
            Err(CanonicalRenderError::EmptyPath)
        );
        assert_eq!(
            CanonicalRenderPath::new(
                path_id.clone(),
                CanonicalRenderFillRule::NonZero,
                vec![CanonicalPathCommand::MoveTo(0), CanonicalPathCommand::Close],
            ),
            Err(CanonicalRenderError::InvalidPathState)
        );
        assert_eq!(
            CanonicalRenderPath::new(
                path_id.clone(),
                CanonicalRenderFillRule::NonZero,
                vec![
                    CanonicalPathCommand::MoveTo(0),
                    CanonicalPathCommand::LineTo(1),
                    CanonicalPathCommand::Close,
                    CanonicalPathCommand::Close,
                ],
            ),
            Err(CanonicalRenderError::InvalidPathState)
        );
        assert_eq!(
            CanonicalRenderPath::new(
                path_id.clone(),
                CanonicalRenderFillRule::NonZero,
                vec![CanonicalPathCommand::Close],
            ),
            Err(CanonicalRenderError::InvalidPathState)
        );
        assert!(
            CanonicalRenderPath::new(
                path_id,
                CanonicalRenderFillRule::EvenOdd,
                vec![
                    CanonicalPathCommand::MoveTo(0),
                    CanonicalPathCommand::LineTo(1),
                    CanonicalPathCommand::Close,
                ],
            )
            .is_ok()
        );
    }

    #[test]
    fn degenerate_point_lists_are_rejected() {
        let mut fixture = Fixture::new();
        let polyline = fixture.id(EntityKind::RenderGeometry, "geometry/polyline");
        assert_eq!(
            CanonicalRenderGeometry::new(
                polyline,
                CanonicalRenderGeometryData::Polyline { points: vec![0] },
            ),
            Err(CanonicalRenderError::DegeneratePointList)
        );
        let polygon = fixture.id(EntityKind::RenderGeometry, "geometry/polygon");
        assert_eq!(
            CanonicalRenderGeometry::new(
                polygon,
                CanonicalRenderGeometryData::Polygon { points: vec![0, 1] },
            ),
            Err(CanonicalRenderError::DegeneratePointList)
        );
    }

    #[test]
    fn wrong_namespace_is_rejected_for_every_record() {
        let mut fixture = Fixture::new();
        let node_ns = fixture.id(EntityKind::RenderNode, "wrong/namespace");
        assert_eq!(
            CanonicalRenderLayer::new(
                node_ns.clone(),
                CanonicalRenderPass::Overlay,
                0,
                0,
                Vec::new()
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderClip::new(node_ns.clone(), CanonicalRenderFillRule::NonZero, 0),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderPath::new(
                node_ns.clone(),
                CanonicalRenderFillRule::NonZero,
                vec![CanonicalPathCommand::MoveTo(0)]
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderGeometry::new(
                node_ns.clone(),
                CanonicalRenderGeometryData::Rect { origin: 0, size: 1 }
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderPaint::new(
                node_ns.clone(),
                CanonicalRenderPaintData::Solid { color: 0 }
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderStroke::new(
                node_ns.clone(),
                0,
                1,
                CanonicalStrokeCap::Butt,
                CanonicalStrokeJoin::Miter,
                4.0,
                2,
                Vec::new(),
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalGlyphRun::new(
                node_ns,
                fixture.id(EntityKind::Resource, "font/main"),
                0,
                0,
                [0.0, 0.0],
                Vec::new()
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
    }

    #[test]
    fn wrong_namespace_is_rejected_for_every_embedded_stable_id() {
        let mut fixture = Fixture::new();
        let geometry_id = fixture.id(EntityKind::RenderGeometry, "geometry/image");
        let paint_id = fixture.id(EntityKind::RenderPaint, "paint/pattern");
        let node_id = fixture.id(EntityKind::RenderNode, "node/attached");
        let not_resource = fixture.id(EntityKind::Line, "line/not-resource");
        assert_eq!(
            CanonicalRenderGeometry::new(
                geometry_id,
                CanonicalRenderGeometryData::Image {
                    resource: not_resource.clone(),
                    destination: [0, 1, 2, 3],
                    source: None,
                    sampling: CanonicalImageSampling::Bilinear,
                },
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderPaint::new(
                paint_id,
                CanonicalRenderPaintData::ImagePattern {
                    resource: not_resource,
                    transform: CanonicalPatternTransform {
                        position: 0,
                        origin: 1,
                        rotation: 2,
                        scale: 3,
                    },
                    repeat: CanonicalImageRepeat::Both,
                    sampling: CanonicalImageSampling::Nearest,
                },
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
        let spec = |attachment: CanonicalRenderAttachment| CanonicalRenderNodeSpec {
            id: node_id.clone(),
            kind: CanonicalRenderNodeKind::Group,
            parent: None,
            layer: 0,
            document_order: 0,
            z_order: 0,
            attachment,
            active: CanonicalActiveInterval::unbounded(),
            isolate: false,
            follow_hidden_attachment: false,
            position: 0,
            origin: 1,
            rotation: 2,
            scale: 3,
            opacity: 4,
            visibility: 5,
            geometry: None,
            fill_paint: None,
            stroke: None,
            clip: None,
            composite: CanonicalRenderComposite::SourceOver,
        };
        assert_eq!(
            CanonicalRenderNode::new(spec(CanonicalRenderAttachment::Line(
                fixture.id(EntityKind::Note, "note/not-line"),
            ))),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalRenderNode::new(spec(CanonicalRenderAttachment::Note(
                fixture.id(EntityKind::Line, "line/not-note"),
            ))),
            Err(CanonicalRenderError::WrongNamespace)
        );
        assert_eq!(
            CanonicalGlyphRun::new(
                fixture.id(EntityKind::RenderGlyphRun, "glyph/run"),
                fixture.id(EntityKind::Line, "line/not-font"),
                0,
                0,
                [0.0, 0.0],
                Vec::new(),
            ),
            Err(CanonicalRenderError::WrongNamespace)
        );
    }

    #[test]
    fn pass_order_is_the_specification_sequence_not_the_name_order() {
        // Sorting these six names as text yields aboveNotes first; the
        // specification puts it fifth. Every pass appears so the two orders
        // cannot coincide by accident.
        let declaration = [
            CanonicalRenderPass::Overlay,
            CanonicalRenderPass::AboveNotes,
            CanonicalRenderPass::Background,
            CanonicalRenderPass::Notes,
            CanonicalRenderPass::BehindLines,
            CanonicalRenderPass::Lines,
        ];
        let mut by_ordinal = declaration;
        by_ordinal.sort_by_key(|pass| pass.ordinal());
        assert_eq!(
            by_ordinal.map(CanonicalRenderPass::as_str),
            [
                "background",
                "behindLines",
                "lines",
                "notes",
                "aboveNotes",
                "overlay"
            ]
        );
        let mut by_name = declaration;
        by_name.sort_by_key(|pass| pass.as_str());
        assert_ne!(
            by_name.map(CanonicalRenderPass::as_str),
            by_ordinal.map(CanonicalRenderPass::as_str),
            "the name order must not be mistaken for the specification order"
        );
        for pass in declaration {
            assert_eq!(
                CanonicalRenderPass::from_spelling(pass.as_str()),
                Some(pass)
            );
        }
        assert_eq!(CanonicalRenderPass::from_spelling("Background"), None);
        assert_eq!(CanonicalRenderPass::from_spelling("hud"), None);
    }

    #[test]
    fn layer_draw_order_is_pass_then_z_then_document_then_id() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let above = fixture.id(EntityKind::RenderLayer, "layer/above");
        let background = fixture.id(EntityKind::RenderLayer, "layer/background");
        let same_pass = fixture.id(EntityKind::RenderLayer, "layer/overlay-earlier");
        spec.layers[0] = CanonicalRenderLayer::new(
            spec.layers[0].id().clone(),
            CanonicalRenderPass::Overlay,
            0,
            0,
            vec![0],
        )
        .expect("layer");
        // aboveNotes sorts before overlay by ordinal, and after background;
        // as text it would sort before both.
        spec.layers.push(
            CanonicalRenderLayer::new(above, CanonicalRenderPass::AboveNotes, 100, 1, Vec::new())
                .expect("layer"),
        );
        spec.layers.push(
            CanonicalRenderLayer::new(
                background,
                CanonicalRenderPass::Background,
                500,
                2,
                Vec::new(),
            )
            .expect("layer"),
        );
        spec.layers.push(
            CanonicalRenderLayer::new(same_pass, CanonicalRenderPass::Overlay, -5, 3, Vec::new())
                .expect("layer"),
        );
        let scene = CanonicalRenderScene::new(spec).expect("scene");
        // background, then aboveNotes, then the two overlay layers by zOrder.
        assert_eq!(scene.layer_draw_order(), vec![2, 1, 3, 0]);
    }

    #[test]
    fn document_order_is_dense_within_each_expanded_collection() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        spec.layers.push(
            CanonicalRenderLayer::new(
                fixture.id(EntityKind::RenderLayer, "layer/second"),
                CanonicalRenderPass::Overlay,
                0,
                2,
                Vec::new(),
            )
            .expect("layer"),
        );
        assert_eq!(
            CanonicalRenderScene::new(spec).expect_err("layer order gap"),
            CanonicalRenderError::InvalidDocumentOrder
        );

        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        spec.nodes[0].document_order = 1;
        assert_eq!(
            CanonicalRenderScene::new(spec).expect_err("root order gap"),
            CanonicalRenderError::InvalidDocumentOrder
        );
    }

    #[test]
    fn every_auxiliary_record_needs_exactly_one_owner() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        // An orphan paint has no owner.
        let orphan = fixture.id(EntityKind::RenderPaint, "paint/orphan");
        spec.paints.push(
            CanonicalRenderPaint::new(orphan, CanonicalRenderPaintData::Solid { color: 9 })
                .expect("paint"),
        );
        assert_eq!(
            CanonicalRenderScene::new(spec).expect_err("orphan paint"),
            CanonicalRenderError::UnreachableRecord
        );

        // Two nodes claiming one paint is cross-owner sharing.
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        let sibling_id = fixture.id(EntityKind::RenderNode, "layer/main/sibling");
        let geometry_id = fixture.id(EntityKind::RenderGeometry, "layer/main/sibling/geometry");
        spec.geometries.push(
            CanonicalRenderGeometry::new(
                geometry_id,
                CanonicalRenderGeometryData::Rect { origin: 0, size: 1 },
            )
            .expect("geometry"),
        );
        spec.nodes.push(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                id: sibling_id,
                kind: CanonicalRenderNodeKind::Rect,
                parent: Some(0),
                layer: 0,
                document_order: 1,
                z_order: 0,
                attachment: CanonicalRenderAttachment::Screen,
                active: CanonicalActiveInterval::unbounded(),
                isolate: false,
                follow_hidden_attachment: false,
                position: 3,
                origin: 4,
                rotation: 5,
                scale: 6,
                opacity: 7,
                visibility: 8,
                geometry: Some(1),
                fill_paint: Some(0),
                stroke: None,
                clip: None,
                composite: CanonicalRenderComposite::SourceOver,
            })
            .expect("node"),
        );
        assert_eq!(
            CanonicalRenderScene::new(spec).expect_err("shared paint"),
            CanonicalRenderError::SharedRecord
        );
    }

    #[test]
    fn stable_ids_collide_across_render_namespaces() {
        let mut fixture = Fixture::new();
        let mut spec = fixture.solid_rect();
        // Section 14 rejects a u64 collision even when the namespace differs,
        // so reuse the layer's value under the node namespace.
        let colliding = StableId {
            namespace: EntityKind::RenderNode,
            value: spec.layers[0].id().value(),
            textual: spec.nodes[0].id().textual().clone(),
        };
        let node = spec.nodes.remove(0);
        spec.nodes.push(
            CanonicalRenderNode::new(CanonicalRenderNodeSpec {
                id: colliding,
                kind: node.kind(),
                parent: node.parent(),
                layer: node.layer(),
                document_order: node.document_order(),
                z_order: node.z_order(),
                attachment: node.attachment().clone(),
                active: node.active(),
                isolate: node.isolate(),
                follow_hidden_attachment: node.follow_hidden_attachment(),
                position: node.position(),
                origin: node.origin(),
                rotation: node.rotation(),
                scale: node.scale(),
                opacity: node.opacity(),
                visibility: node.visibility(),
                geometry: node.geometry(),
                fill_paint: node.fill_paint(),
                stroke: node.stroke(),
                clip: node.clip(),
                composite: node.composite(),
            })
            .expect("node"),
        );
        assert_eq!(
            CanonicalRenderScene::new(spec).expect_err("cross-namespace collision"),
            CanonicalRenderError::DuplicateStableId
        );
    }

    #[test]
    fn a_non_isolated_group_must_composite_source_over() {
        let mut fixture = Fixture::new();
        let group_id = fixture.id(EntityKind::RenderNode, "layer/main/group");
        let spec = |isolate: bool, composite: CanonicalRenderComposite| CanonicalRenderNodeSpec {
            id: group_id.clone(),
            kind: CanonicalRenderNodeKind::Group,
            parent: None,
            layer: 0,
            document_order: 0,
            z_order: 0,
            attachment: CanonicalRenderAttachment::World,
            active: CanonicalActiveInterval::unbounded(),
            isolate,
            follow_hidden_attachment: false,
            position: 0,
            origin: 1,
            rotation: 2,
            scale: 3,
            opacity: 4,
            visibility: 5,
            geometry: None,
            fill_paint: None,
            stroke: None,
            clip: None,
            composite,
        };
        assert_eq!(
            CanonicalRenderNode::new(spec(false, CanonicalRenderComposite::Multiply)),
            Err(CanonicalRenderError::NonIsolatedGroupComposite)
        );
        assert!(CanonicalRenderNode::new(spec(true, CanonicalRenderComposite::Multiply)).is_ok());
        assert!(
            CanonicalRenderNode::new(spec(false, CanonicalRenderComposite::SourceOver)).is_ok()
        );
    }

    #[test]
    fn follow_hidden_attachment_requires_note_attachment() {
        let mut fixture = Fixture::new();
        let node_id = fixture.id(EntityKind::RenderNode, "layer/main/follower");
        let note_id = fixture.id(EntityKind::Note, "note/1");
        let spec = |attachment: CanonicalRenderAttachment| CanonicalRenderNodeSpec {
            id: node_id.clone(),
            kind: CanonicalRenderNodeKind::Group,
            parent: None,
            layer: 0,
            document_order: 0,
            z_order: 0,
            attachment,
            active: CanonicalActiveInterval::unbounded(),
            isolate: false,
            follow_hidden_attachment: true,
            position: 0,
            origin: 1,
            rotation: 2,
            scale: 3,
            opacity: 4,
            visibility: 5,
            geometry: None,
            fill_paint: None,
            stroke: None,
            clip: None,
            composite: CanonicalRenderComposite::SourceOver,
        };
        for attachment in [
            CanonicalRenderAttachment::World,
            CanonicalRenderAttachment::Screen,
        ] {
            assert_eq!(
                CanonicalRenderNode::new(spec(attachment)),
                Err(CanonicalRenderError::FollowHiddenWithoutNoteAttachment)
            );
        }
        assert!(CanonicalRenderNode::new(spec(CanonicalRenderAttachment::Note(note_id))).is_ok());
    }

    #[test]
    fn an_empty_glyph_run_is_legal() {
        let mut fixture = Fixture::new();
        let run_id = fixture.id(EntityKind::RenderGlyphRun, "run/empty");
        let font_id = fixture.id(EntityKind::Resource, "resource/font");
        // Section 14.7 allows a zero glyph count for empty source content.
        assert!(CanonicalGlyphRun::new(run_id, font_id, 0, 0, [0.0, 0.0], Vec::new()).is_ok());
    }

    #[test]
    fn no_rejection_invents_a_diagnostic_category() {
        let all = CanonicalRenderError::all();
        // `next_variant` has no wildcard arm, so a new variant cannot compile
        // without an arm. This count catches the remaining hole: a chain that
        // skips a variant whose arm exists but that nothing points at. A stale
        // number fails loudly here rather than silently shrinking coverage.
        assert_eq!(all.len(), 40, "variant walk skips a variant");
        assert_eq!(
            all.iter().collect::<BTreeSet<_>>().len(),
            all.len(),
            "variant walk repeats a variant"
        );
        for error in all {
            let code = error.code();
            assert!(
                RENDER_DIAGNOSTIC_CATEGORIES.contains(&code),
                "{error:?} reports {code}, which fcs-render.md section 16 does not define"
            );
        }
    }

    #[test]
    fn error_codes_follow_the_section_16_failure_surface_order() {
        // Row 3: the viewport lives in the RenderSection header.
        assert_eq!(
            CanonicalRenderError::InvalidViewport.code(),
            "render.invalid-section"
        );
        // Row 5: duplicate identity, layer pass, node graph shape, orphans.
        for error in [
            CanonicalRenderError::DuplicateStableId,
            CanonicalRenderError::ParentNotBeforeChild,
            CanonicalRenderError::InvalidActiveInterval,
            CanonicalRenderError::InvalidDocumentOrder,
            CanonicalRenderError::SharedRecord,
            CanonicalRenderError::UnreachableRecord,
        ] {
            assert_eq!(error.code(), "render.invalid-graph", "{error:?}");
        }
        // Row 6: reference bounds, nullability and attachment.
        for error in [
            CanonicalRenderError::UnresolvedReference,
            CanonicalRenderError::GroupCarriesGeometry,
            CanonicalRenderError::DrawableWithoutPaint,
            CanonicalRenderError::ImageCarriesPaint,
            CanonicalRenderError::LinePaintBinding,
            CanonicalRenderError::ClipGroupWithoutClip,
            CanonicalRenderError::AttachmentOverrideBelowRoot,
        ] {
            assert_eq!(error.code(), "render.invalid-reference", "{error:?}");
        }
        // Row 7: a decoded font's glyph problems stay geometry and must not
        // fall back to render.resource-decode-failed.
        for error in [
            CanonicalRenderError::GeometryKindMismatch,
            CanonicalRenderError::NonFiniteGlyphMetric,
        ] {
            assert_eq!(error.code(), "render.invalid-geometry", "{error:?}");
        }
        // Rows 9 and 10.
        assert_eq!(
            CanonicalRenderError::ZeroDashTotal.code(),
            "render.invalid-stroke"
        );
        assert_eq!(
            CanonicalRenderError::ClipGeometryKindNotAllowed.code(),
            "render.invalid-clip"
        );
        assert_eq!(
            CanonicalRenderError::InvalidPathState.code(),
            "render.invalid-geometry"
        );
        assert_eq!(
            CanonicalRenderError::GroupCarriesClip.code(),
            "render.invalid-reference"
        );
        assert_eq!(
            CanonicalRenderError::IsolatedDrawable.code(),
            "render.invalid-composite"
        );
    }
}
