//! I3.7 source-to-canonical scroll lowering.

use fcs_model::{CanonicalScrollLine, CanonicalScrollSet, ChartTimeMap, ScrollCoordinateError};

use crate::ast::Document;
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};

impl Document {
    /// Lowers validated Line scroll declarations into the I3.7 constant-speed model.
    pub fn canonical_scroll_set(
        &self,
        time_map: &ChartTimeMap,
    ) -> Result<CanonicalScrollSet, Vec<Diagnostic>> {
        let graph = self.canonical_line_graph()?;
        self.canonical_scroll_set_for_graph(time_map, &graph)
    }

    pub(crate) fn canonical_scroll_set_for_graph(
        &self,
        time_map: &ChartTimeMap,
        graph: &fcs_model::CanonicalLineGraph,
    ) -> Result<CanonicalScrollSet, Vec<Diagnostic>> {
        let lines = graph
            .lines()
            .map(|line| {
                let coordinate = fcs_model::coordinate_for_tempo(line.scroll_tempo(), time_map)
                    .map_err(|error| vec![scroll_diagnostic(error, self)])?;
                CanonicalScrollLine::new(
                    line.id().clone(),
                    coordinate,
                    1.0,
                    line.base().allow_reverse_scroll(),
                    line.base().floor_scale(),
                    line.base().integration_origin(),
                    line.base().initial_floor_position(),
                )
                .map_err(|error| vec![scroll_diagnostic(error, self)])
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        CanonicalScrollSet::new(lines).map_err(|error| vec![scroll_diagnostic(error, self)])
    }
}

fn scroll_diagnostic(error: ScrollCoordinateError, document: &Document) -> Diagnostic {
    let span = document
        .lines
        .first()
        .map(|line| line.name_span)
        .unwrap_or(crate::ast::SourceSpan::new(0, 0));
    Diagnostic::new(
        match error {
            ScrollCoordinateError::NonMonotonic => DiagnosticCode::TEMPO_NON_MONOTONIC,
            ScrollCoordinateError::InvalidBpm | ScrollCoordinateError::FirstPointNotZero => {
                DiagnosticCode::TEMPO_INVALID
            }
            ScrollCoordinateError::Empty
            | ScrollCoordinateError::NonFinite
            | ScrollCoordinateError::InvalidLinePolicy
            | ScrollCoordinateError::DuplicateLine
            | ScrollCoordinateError::WrongLineNamespace => DiagnosticCode::NUMERIC_DOMAIN,
        },
        DiagnosticStage::Canonical,
        error.to_string(),
        span,
    )
}
