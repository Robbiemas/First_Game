use crate::{LedgerTabTemplate, LedgerTabTemplateRow};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcbCoverageSurface {
    pub id: String,
    pub mapped_action_count: usize,
    pub mapped_motion_state_count: usize,
    pub mapped_motion_states: Vec<EcbCoverageMotionState>,
    pub missing_sampled_mappings: Vec<String>,
    pub source: EcbCoverageSource,
    pub title: String,
    pub unmapped_derived_motion_states: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcbCoverageMotionState {
    pub action_state_id: usize,
    pub motion_state: String,
    pub sample_frames: usize,
    pub source_action: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcbCoverageSource {
    pub generated_rust: String,
    pub samples: String,
}

impl EcbCoverageSurface {
    pub fn empty() -> Self {
        Self {
            id: "ecb_coverage".to_string(),
            mapped_action_count: 0,
            mapped_motion_state_count: 0,
            mapped_motion_states: Vec::new(),
            missing_sampled_mappings: Vec::new(),
            source: EcbCoverageSource {
                generated_rust: String::new(),
                samples: String::new(),
            },
            title: "ECB Coverage".to_string(),
            unmapped_derived_motion_states: Vec::new(),
        }
    }

    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let path = root
            .as_ref()
            .join("docs/state_graphs/parity_reports/falcon_ecb_coverage.json");
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse ECB coverage report: {error}"))
    }

    pub fn summary(&self) -> String {
        format!(
            "Mapped actions: {} | Mapped motion states: {} | Missing sampled mappings: {} | Unmapped derived states: {}",
            self.mapped_action_count,
            self.mapped_motion_state_count,
            self.missing_sampled_mappings.len(),
            self.unmapped_derived_motion_states.len()
        )
    }
}

impl From<&EcbCoverageSurface> for LedgerTabTemplate {
    fn from(surface: &EcbCoverageSurface) -> Self {
        Self {
            title: surface.title.clone(),
            summary: surface.summary(),
            headers: vec![
                "Action State".to_string(),
                "Motion State".to_string(),
                "Sample Frames".to_string(),
                "Source Action".to_string(),
                "Status".to_string(),
            ],
            rows: surface
                .mapped_motion_states
                .iter()
                .map(LedgerTabTemplateRow::from)
                .collect(),
        }
    }
}

impl From<&EcbCoverageMotionState> for LedgerTabTemplateRow {
    fn from(row: &EcbCoverageMotionState) -> Self {
        Self {
            cells: vec![
                row.action_state_id.to_string(),
                row.motion_state.clone(),
                row.sample_frames.to_string(),
                row.source_action.clone(),
                row.status.clone(),
            ],
            detail: format!(
                "action_state_id: {}\nmotion_state: {}\nsample_frames: {}\nsource_action: {}\nstatus: {}",
                row.action_state_id,
                row.motion_state,
                row.sample_frames,
                row.source_action,
                row.status
            ),
            status: Some(row.status.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn ecb_coverage_loads_and_templates_the_mapped_rows() {
        let surface = EcbCoverageSurface::load(workspace_root()).unwrap();
        let template = LedgerTabTemplate::from(&surface);

        assert_eq!(surface.title, "Captain Falcon ECB Coverage");
        assert_eq!(template.title, "Captain Falcon ECB Coverage");
        assert_eq!(template.headers.len(), 5);
        assert_eq!(template.rows.len(), surface.mapped_motion_states.len());
        assert_eq!(
            template.rows.first().unwrap().status.as_deref(),
            Some("mapped_exact_action_table")
        );
    }
}
