//! Explicit-workspace resource resolution for canonical compilation.

use std::cmp;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use fcs_model::{
    CanonicalBundledResource, CanonicalResource, CanonicalResourceBundle,
    CanonicalResourceBundleError,
};

use crate::ast::{Document, SourceSpan};
use crate::canonical::{LoweredDocument, lower_document_with_sources};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};

/// Public implementation limits for one workspace resource-resolution pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    max_resources: usize,
    max_single_resource_bytes: usize,
    max_total_resource_bytes: usize,
}

impl ResourceLimits {
    pub const DEFAULT_MAX_RESOURCES: usize = 4_096;
    pub const DEFAULT_MAX_SINGLE_RESOURCE_BYTES: usize = 256 * 1024 * 1024;
    pub const DEFAULT_MAX_TOTAL_RESOURCE_BYTES: usize = 1024 * 1024 * 1024;

    pub const fn new(
        max_resources: usize,
        max_single_resource_bytes: usize,
        max_total_resource_bytes: usize,
    ) -> Self {
        Self {
            max_resources,
            max_single_resource_bytes,
            max_total_resource_bytes,
        }
    }

    pub const fn max_resources(self) -> usize {
        self.max_resources
    }

    pub const fn max_single_resource_bytes(self) -> usize {
        self.max_single_resource_bytes
    }

    pub const fn max_total_resource_bytes(self) -> usize {
        self.max_total_resource_bytes
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_MAX_RESOURCES,
            Self::DEFAULT_MAX_SINGLE_RESOURCE_BYTES,
            Self::DEFAULT_MAX_TOTAL_RESOURCE_BYTES,
        )
    }
}

struct ResolvedResource {
    resource: CanonicalResource,
    path: PathBuf,
    span: SourceSpan,
    preflight_bytes: usize,
}

impl Document {
    /// Resolves every declared resource below `workspace_root` and returns its
    /// deterministic, source-free opaque payload closure.
    ///
    /// Parsing and [`Document::canonical_metadata`] remain filesystem-free.
    /// Media bytes are never decoded, transcoded, or normalized here.
    pub fn canonical_resource_bundle(
        &self,
        workspace_root: impl AsRef<Path>,
        limits: ResourceLimits,
    ) -> Result<CanonicalResourceBundle, Vec<Diagnostic>> {
        let resource_span = self
            .resources
            .as_ref()
            .map_or(self.format.span, |block| block.span);
        let declared_resource_count = self
            .resources
            .as_ref()
            .map_or(0, |block| block.resources.len());
        if declared_resource_count > limits.max_resources {
            return Err(vec![resource_limit_diagnostic(
                "resource-count",
                limits.max_resources,
                declared_resource_count,
                resource_span,
            )]);
        }
        let LoweredDocument {
            metadata,
            resource_sources,
        } = lower_document_with_sources(self)?;
        if metadata.resources().is_empty() {
            return Ok(CanonicalResourceBundle::new(Vec::new())
                .expect("an empty canonical resource bundle cannot contain duplicate IDs"));
        }

        let canonical_root = match fs::canonicalize(workspace_root.as_ref()) {
            Ok(path) if path.is_dir() => path,
            Ok(_) | Err(_) => {
                return Err(vec![resource_diagnostic(
                    DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                    "workspace root is unavailable or is not a directory",
                    resource_span,
                )]);
            }
        };

        let mut resolved = Vec::with_capacity(metadata.resources().len());
        let mut diagnostics = Vec::new();
        let mut total_preflight_bytes = 0usize;

        for (id, resource) in metadata.resources() {
            let source = resource_sources
                .get(id)
                .expect("canonical resource lowering retains its authoring source");
            let candidate = canonical_root.join(&source.logical_path);
            let path = match fs::canonicalize(candidate) {
                Ok(path) if path.starts_with(&canonical_root) => path,
                Ok(_) | Err(_) => {
                    diagnostics.push(resource_diagnostic(
                        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                        format!("resource {id} does not resolve to a workspace member"),
                        source.span,
                    ));
                    continue;
                }
            };
            let file_metadata = match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => metadata,
                Ok(_) | Err(_) => {
                    diagnostics.push(resource_diagnostic(
                        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                        format!("resource {id} does not resolve to a regular file"),
                        source.span,
                    ));
                    continue;
                }
            };
            let observed = usize::try_from(file_metadata.len()).unwrap_or(usize::MAX);
            if observed > limits.max_single_resource_bytes {
                diagnostics.push(resource_limit_diagnostic(
                    "single-resource-bytes",
                    limits.max_single_resource_bytes,
                    observed,
                    source.span,
                ));
                continue;
            }
            total_preflight_bytes = total_preflight_bytes.saturating_add(observed);
            resolved.push(ResolvedResource {
                resource: resource.clone(),
                path,
                span: source.span,
                preflight_bytes: observed,
            });
        }

        if total_preflight_bytes > limits.max_total_resource_bytes {
            diagnostics.push(resource_limit_diagnostic(
                "total-resource-bytes",
                limits.max_total_resource_bytes,
                total_preflight_bytes,
                resource_span,
            ));
        }
        if !diagnostics.is_empty() {
            sort_diagnostics(&mut diagnostics);
            return Err(diagnostics);
        }

        let mut bundled = Vec::with_capacity(resolved.len());
        let mut total_read_bytes = 0usize;
        for resolved_resource in resolved {
            let remaining_total = limits
                .max_total_resource_bytes
                .saturating_sub(total_read_bytes);
            let read_limit = cmp::min(limits.max_single_resource_bytes, remaining_total);
            let probe_limit = read_limit.saturating_add(1);
            let file = match File::open(&resolved_resource.path) {
                Ok(file) => file,
                Err(_) => {
                    diagnostics.push(resource_diagnostic(
                        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                        format!(
                            "resource {} could not be opened as a regular workspace member",
                            resolved_resource.resource.id()
                        ),
                        resolved_resource.span,
                    ));
                    continue;
                }
            };
            let mut bytes =
                Vec::with_capacity(cmp::min(resolved_resource.preflight_bytes, read_limit));
            let mut bounded = file.take(u64::try_from(probe_limit).unwrap_or(u64::MAX));
            if bounded.read_to_end(&mut bytes).is_err() {
                diagnostics.push(resource_diagnostic(
                    DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                    format!(
                        "resource {} could not be read as opaque bytes",
                        resolved_resource.resource.id()
                    ),
                    resolved_resource.span,
                ));
                continue;
            }
            if bytes.len() > limits.max_single_resource_bytes {
                diagnostics.push(resource_limit_diagnostic(
                    "single-resource-bytes",
                    limits.max_single_resource_bytes,
                    bytes.len(),
                    resolved_resource.span,
                ));
                continue;
            }
            let observed_total = total_read_bytes.saturating_add(bytes.len());
            if observed_total > limits.max_total_resource_bytes {
                diagnostics.push(resource_limit_diagnostic(
                    "total-resource-bytes",
                    limits.max_total_resource_bytes,
                    observed_total,
                    resolved_resource.span,
                ));
                continue;
            }
            total_read_bytes = observed_total;
            match CanonicalBundledResource::new(resolved_resource.resource, bytes) {
                Ok(resource) => bundled.push(resource),
                Err(_) => diagnostics.push(resource_diagnostic(
                    DiagnosticCode::RESOURCE_HASH_MISMATCH,
                    "declared resource SHA-256 does not match the exact workspace bytes",
                    resolved_resource.span,
                )),
            }
        }

        if !diagnostics.is_empty() {
            sort_diagnostics(&mut diagnostics);
            return Err(diagnostics);
        }
        CanonicalResourceBundle::new(bundled).map_err(|error| {
            let CanonicalResourceBundleError::DuplicateId(id) = error;
            vec![resource_diagnostic(
                DiagnosticCode::NAME_DUPLICATE,
                format!("canonical resource bundle contains duplicate ID {id}"),
                resource_span,
            )]
        })
    }
}

fn resource_diagnostic(
    code: DiagnosticCode,
    message: impl Into<String>,
    span: SourceSpan,
) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Canonical, message, span)
}

fn resource_limit_diagnostic(
    kind: &'static str,
    limit: usize,
    observed: usize,
    span: SourceSpan,
) -> Diagnostic {
    resource_diagnostic(
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
        format!("resource limit {kind} exceeded: limit {limit}, observed {observed}"),
        span,
    )
    .with_budget(kind, limit, observed)
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        left.primary_span()
            .start
            .cmp(&right.primary_span().start)
            .then_with(|| left.primary_span().end.cmp(&right.primary_span().end))
            .then_with(|| left.code().cmp(&right.code()))
            .then_with(|| left.message().cmp(right.message()))
    });
}
