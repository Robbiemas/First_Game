use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone)]
pub struct ParityLedgerSurface {
    pub summary: String,
    pub tabs: Vec<ParityLedgerSurfaceTab>,
}

#[derive(Debug, Clone)]
pub struct ParityLedgerSurfaceTab {
    pub id: String,
    pub label: String,
    pub summary: String,
    pub headers: Vec<String>,
    pub rows: Vec<ParityLedgerSurfaceRow>,
}

#[derive(Debug, Clone)]
pub struct ParityLedgerSurfaceRow {
    pub cells: Vec<String>,
    pub detail: String,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValueParityDiffReport {
    sections: BTreeMap<String, ValueParityDiffSection>,
}

#[derive(Debug, Deserialize)]
struct ValueParityDiffSection {
    rows: Vec<ValueParityDiffRow>,
}

#[derive(Debug, Deserialize)]
struct ValueParityDiffRow {
    category: String,
    decomp_value: Value,
    field: String,
    kind: String,
    note: String,
    offset: String,
    provenance: String,
    raw: Value,
    rust_field: String,
    rust_value: Value,
    source_field: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ValueSheetFile {
    id: String,
    title: String,
    scope: String,
    engine_boundary: String,
    categories: Vec<ValueSheetCategory>,
}

#[derive(Debug, Deserialize)]
struct ValueSheetCategory {
    id: String,
    #[allow(dead_code)]
    label: String,
    fields: Vec<ValueSheetField>,
}

#[derive(Debug, Deserialize)]
struct ValueSheetField {
    rust_name: String,
    source_name: String,
    offset_hex: Option<String>,
    kind: String,
    raw: Value,
    converted_value: Value,
    comparison_value_kind: String,
    provenance: String,
    owner_scope: Option<String>,
    owner_id: Option<String>,
    notes: Option<String>,
}

impl ParityLedgerSurface {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        let diff_report = load_value_parity_diff_report(
            root.join("docs/state_graphs/parity_reports/value_diffs.json"),
        )?;
        let global_tab = comparison_tab(
            "global_values",
            "Global Values",
            section_rows(&diff_report, "global_values")?,
        );
        let character_tab = comparison_tab(
            "test_character_values",
            "Test Character Values",
            section_rows(&diff_report, "test_character_values")?,
        );
        let physics_sheet = load_value_sheet(
            root.join("docs/state_graphs/value_sheets/physics_engine_values.json"),
        )?;
        let global_combat_sheet = load_value_sheet(
            root.join("docs/state_graphs/value_sheets/global_combat_values.json"),
        )?;
        let falcon_combat_sheet = load_value_sheet(
            root.join("docs/state_graphs/value_sheets/captain_falcon_combat_values.json"),
        )?;
        let stage_sheet = load_value_sheet(
            root.join("docs/state_graphs/value_sheets/battlefield_stage_values.json"),
        )?;

        let tabs = vec![
            global_tab,
            character_tab,
            value_sheet_tab("physics_engine_values", &physics_sheet),
            value_sheet_tab("global_combat_values", &global_combat_sheet),
            value_sheet_tab("captain_falcon_combat_values", &falcon_combat_sheet),
            value_sheet_tab("battlefield_stage_values", &stage_sheet),
        ];

        let summary = build_summary(
            &tabs,
            &physics_sheet,
            &global_combat_sheet,
            &falcon_combat_sheet,
            &stage_sheet,
        );
        Ok(Self { summary, tabs })
    }
}

fn load_value_parity_diff_report(path: impl AsRef<Path>) -> Result<ValueParityDiffReport, String> {
    let text = fs::read_to_string(path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", path.as_ref().display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse value diff report: {error}"))
}

fn load_value_sheet(path: impl AsRef<Path>) -> Result<ValueSheetFile, String> {
    let text = fs::read_to_string(path.as_ref())
        .map_err(|error| format!("failed to read {}: {error}", path.as_ref().display()))?;
    serde_json::from_str(&text).map_err(|error| format!("failed to parse value sheet: {error}"))
}

fn section_rows<'a>(
    report: &'a ValueParityDiffReport,
    section_id: &str,
) -> Result<&'a [ValueParityDiffRow], String> {
    report
        .sections
        .get(section_id)
        .map(|section| section.rows.as_slice())
        .ok_or_else(|| format!("missing parity diff report section: {section_id}"))
}

fn comparison_tab(id: &str, label: &str, rows: &[ValueParityDiffRow]) -> ParityLedgerSurfaceTab {
    let headers = vec![
        "Category".to_string(),
        "Ledger Field".to_string(),
        "Decomp Field".to_string(),
        "Offset".to_string(),
        "Decomp Value".to_string(),
        "Rust Field".to_string(),
        "Rust Value".to_string(),
        "Status".to_string(),
    ];
    let row_values = rows
        .iter()
        .map(|row| {
            let detail = format_value_comparison_details(row);
            ParityLedgerSurfaceRow {
                cells: vec![
                    row.category.clone(),
                    row.field.clone(),
                    row.source_field.clone(),
                    row.offset.clone(),
                    display_value(&row.decomp_value),
                    row.rust_field.clone(),
                    display_value(&row.rust_value),
                    row.status.clone(),
                ],
                detail,
                status: Some(row.status.clone()),
            }
        })
        .collect::<Vec<_>>();

    let summary = comparison_summary(rows);
    ParityLedgerSurfaceTab {
        id: id.to_string(),
        label: label.to_string(),
        summary,
        headers,
        rows: row_values,
    }
}

fn value_sheet_tab(id: &str, sheet: &ValueSheetFile) -> ParityLedgerSurfaceTab {
    let headers = vec![
        "Category".to_string(),
        "Field".to_string(),
        "Source Field".to_string(),
        "Offset".to_string(),
        "Value".to_string(),
        "Kind".to_string(),
        "Provenance".to_string(),
    ];
    let rows = build_value_sheet_rows(sheet)
        .into_iter()
        .map(|row| ParityLedgerSurfaceRow {
            detail: format_value_sheet_details(&row),
            cells: row.cells,
            status: None,
        })
        .collect::<Vec<_>>();

    let summary = format!(
        "{}: {} categories, {} fields | scope: {} | boundary: {}",
        sheet.id,
        sheet.categories.len(),
        rows.len(),
        sheet.scope,
        sheet.engine_boundary
    );
    ParityLedgerSurfaceTab {
        id: id.to_string(),
        label: sheet.title.clone(),
        summary,
        headers,
        rows,
    }
}

#[derive(Debug)]
struct ValueSheetRow {
    cells: Vec<String>,
    category: String,
    field: String,
    source_field: String,
    offset: String,
    value: String,
    raw: String,
    kind: String,
    comparison_value_kind: String,
    provenance: String,
    owner_scope: Option<String>,
    owner_id: Option<String>,
    notes: Option<String>,
}

fn build_value_sheet_rows(sheet: &ValueSheetFile) -> Vec<ValueSheetRow> {
    let mut rows = Vec::new();
    for category in &sheet.categories {
        for field in &category.fields {
            let row = ValueSheetRow {
                cells: vec![
                    category.id.clone(),
                    field.rust_name.clone(),
                    field.source_name.clone(),
                    field.offset_hex.clone().unwrap_or_default(),
                    display_value(&field.converted_value),
                    field.kind.clone(),
                    field.provenance.clone(),
                ],
                category: category.id.clone(),
                field: field.rust_name.clone(),
                source_field: field.source_name.clone(),
                offset: field.offset_hex.clone().unwrap_or_default(),
                value: display_value(&field.converted_value),
                raw: display_value(&field.raw),
                kind: field.kind.clone(),
                comparison_value_kind: field.comparison_value_kind.clone(),
                provenance: field.provenance.clone(),
                owner_scope: field.owner_scope.clone(),
                owner_id: field.owner_id.clone(),
                notes: field.notes.clone(),
            };
            rows.push(row);
        }
    }
    rows
}

fn comparison_summary(rows: &[ValueParityDiffRow]) -> String {
    let total = rows.len();
    let mut match_count = 0;
    let mut diff_count = 0;
    let mut derived_count = 0;
    let mut missing_count = 0;
    let mut actionable_count = 0;
    for row in rows {
        match row.status.as_str() {
            "match" => match_count += 1,
            "diff" => diff_count += 1,
            "derived" => derived_count += 1,
            "missing" => missing_count += 1,
            _ => {}
        }
        if row.status != "match" {
            actionable_count += 1;
        }
    }
    format!(
        "Total: {total} | Match: {match_count} | Diff: {diff_count} | Derived: {derived_count} | Missing: {missing_count} | Actionable: {actionable_count}"
    )
}

fn build_summary(
    tabs: &[ParityLedgerSurfaceTab],
    physics_sheet: &ValueSheetFile,
    global_combat_sheet: &ValueSheetFile,
    falcon_combat_sheet: &ValueSheetFile,
    stage_sheet: &ValueSheetFile,
) -> String {
    let mut lines = vec![
        "Parity Ledger".to_string(),
        "".to_string(),
        "Active sub-tabs:".to_string(),
    ];
    for tab in tabs {
        lines.push(format!("- {}: {}", tab.label, tab.summary));
    }
    lines.push(String::new());
    lines.push(format!(
        "Physics sheet: {} categories, {} fields",
        physics_sheet.categories.len(),
        build_value_sheet_rows(physics_sheet).len()
    ));
    lines.push(format!(
        "Global combat sheet: {} categories, {} fields",
        global_combat_sheet.categories.len(),
        build_value_sheet_rows(global_combat_sheet).len()
    ));
    lines.push(format!(
        "Captain Falcon combat sheet: {} categories, {} fields",
        falcon_combat_sheet.categories.len(),
        build_value_sheet_rows(falcon_combat_sheet).len()
    ));
    lines.push(format!(
        "Stage sheet: {} categories, {} fields",
        stage_sheet.categories.len(),
        build_value_sheet_rows(stage_sheet).len()
    ));
    lines.join("\n")
}

fn format_value_comparison_details(row: &ValueParityDiffRow) -> String {
    let mut lines = vec![
        format!("{} [{}]", row.field, row.status),
        String::new(),
        format!("Category: {}", row.category),
        format!(
            "Decomp: {} @ {} -> {}",
            row.source_field,
            row.offset,
            display_value(&row.decomp_value)
        ),
        format!(
            "Rust: {} -> {}",
            row.rust_field,
            display_value(&row.rust_value)
        ),
        format!("Kind: {}", row.kind),
        format!("Raw decomp value: {}", display_value(&row.raw)),
        format!("Provenance: {}", row.provenance),
    ];
    if !row.note.is_empty() {
        lines.push(String::new());
        lines.push(row.note.clone());
    }
    lines.join("\n")
}

fn format_value_sheet_details(row: &ValueSheetRow) -> String {
    let mut lines = vec![
        row.field.clone(),
        String::new(),
        format!("Category: {}", row.category),
        format!("Source field: {} @ {}", row.source_field, row.offset),
        format!("Value: {}", row.value),
        format!("Raw: {}", row.raw),
        format!("Kind: {}", row.kind),
        format!("Comparison kind: {}", row.comparison_value_kind),
        format!("Provenance: {}", row.provenance),
    ];
    if let Some(owner_scope) = &row.owner_scope {
        lines.push(format!("Owner scope: {}", owner_scope));
    }
    if let Some(owner_id) = &row.owner_id {
        lines.push(format!("Owner id: {}", owner_id));
    }
    if let Some(notes) = &row.notes {
        if !notes.is_empty() {
            lines.push(String::new());
            lines.push(notes.clone());
        }
    }
    lines.join("\n")
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn surface_loads_active_parity_ledger_tabs_and_rows() {
        let surface = ParityLedgerSurface::load(workspace_root()).expect("surface loads");

        assert_eq!(surface.tabs.len(), 6);
        assert_eq!(surface.tabs[0].label, "Global Values");
        assert_eq!(surface.tabs[1].label, "Test Character Values");
        assert_eq!(surface.tabs[2].label, "Physics Engine Values");
        assert_eq!(surface.tabs[3].label, "Global Combat Values");
        assert_eq!(surface.tabs[4].label, "Captain Falcon Combat Values");
        assert!(!surface.tabs[0].rows.is_empty());
        assert!(!surface.tabs[5].rows.is_empty());
        assert!(surface.summary.contains("Parity Ledger"));
    }
}
