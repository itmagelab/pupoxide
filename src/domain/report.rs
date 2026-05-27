use crate::domain::resource::SourceContext;
use serde::{Deserialize, Serialize};

/// The execution outcome of a resource synchronization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceStatus {
    /// The resource was successfully modified to match the desired state.
    Applied,
    /// The resource was already in the desired state, so no action was taken.
    Unchanged,
    /// An error occurred during verification or application.
    Failed,
    /// The execution was skipped because a parent dependency failed.
    Skipped,
    /// The resource would be modified (used during dry-runs).
    WouldApply,
}

/// A detailed report representing the execution outcome of a single resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReport {
    /// The unique identifier of the synchronized resource.
    pub resource_id: String,
    /// The status outcome of the execution.
    pub status: ResourceStatus,
    /// An optional context message (e.g. error details or dry-run info).
    pub message: Option<String>,
    /// Indicates if any state changes were actually applied to the system.
    pub changed: bool,
    /// The time taken to verify and synchronize this resource.
    pub duration: core::time::Duration,
    /// Location context metadata from Rhai compilation source.
    pub source_context: Option<SourceContext>,
}

impl ResourceReport {
    /// Creates a new `ResourceReport` for the given resource and status.
    pub fn new(resource_id: String, status: ResourceStatus, changed: bool) -> Self {
        Self {
            resource_id,
            status,
            message: None,
            changed,
            duration: core::time::Duration::from_secs(0),
            source_context: None,
        }
    }

    /// Builder-style method to attach location metadata context.
    pub fn with_source_context(mut self, source_context: Option<SourceContext>) -> Self {
        self.source_context = source_context;
        self
    }

    /// Builder-style method to set execution duration.
    pub fn with_duration(mut self, duration: core::time::Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Builder-style method to set a context details message.
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }
}
