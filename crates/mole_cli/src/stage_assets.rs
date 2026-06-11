use mole_core::{StageProfile, StageSurface, StageSurfaceKind};
use serde_json::{json, Value};
use std::{fs, path::Path};

use crate::SCHEMA_VERSION;

pub(crate) const STAGE_ASSETS_GENERATOR: &str = "crates/mole_cli/src/stage_assets.rs";
pub(crate) const STAGE_ASSETS_COMMAND: &str =
    "cargo run -p mole_cli -- generated write-stage-asset --stage battlefield --write --json";
pub(crate) const STAGE_ASSET_INPUTS: &[&str] = &["crates/mole_core/src/stage.rs"];
pub(crate) const STAGE_ASSET_OUTPUTS: &[&str] =
    &["resources/melee/extracted/stages/battlefield_stage.json"];

pub(crate) fn write_stage_asset_report(root: &Path, stage_id: &str, write: bool) -> Value {
    match build_stage_asset_outputs(stage_id) {
        Ok(outputs) => {
            if write {
                if let Err(error) = write_outputs(root, &outputs) {
                    return json!({
                        "schema_version": SCHEMA_VERSION,
                        "command": "generated write-stage-asset",
                        "project_root": root.display().to_string(),
                        "mutated": false,
                        "ok": false,
                        "error": error,
                    });
                }
            }

            let assets = outputs
                .iter()
                .map(|output| {
                    let surfaces = output
                        .asset
                        .get("soft_platforms")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len);
                    json!({
                        "path": output.relative_path,
                        "stage_id": output.stage_id,
                        "stage_name": output.stage_name,
                        "surface_count": surfaces + 1,
                    })
                })
                .collect::<Vec<_>>();
            let written_paths = if write {
                outputs
                    .iter()
                    .map(|output| output.relative_path.to_string())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            json!({
                "schema_version": SCHEMA_VERSION,
                "command": "generated write-stage-asset",
                "project_root": root.display().to_string(),
                "mutated": write,
                "ok": true,
                "write": write,
                "generator": STAGE_ASSETS_GENERATOR,
                "recommended_command": STAGE_ASSETS_COMMAND,
                "stage_id": stage_id,
                "inputs": STAGE_ASSET_INPUTS,
                "outputs": STAGE_ASSET_OUTPUTS,
                "written_paths": written_paths,
                "pending_paths": if write { Vec::<String>::new() } else {
                    STAGE_ASSET_OUTPUTS.iter().map(|path| path.to_string()).collect::<Vec<_>>()
                },
                "assets": assets,
            })
        }
        Err(error) => json!({
            "schema_version": SCHEMA_VERSION,
            "command": "generated write-stage-asset",
            "project_root": root.display().to_string(),
            "mutated": false,
            "ok": false,
            "stage_id": stage_id,
            "error": error,
        }),
    }
}

struct StageAssetOutput {
    relative_path: &'static str,
    stage_id: &'static str,
    stage_name: &'static str,
    asset: Value,
}

fn build_stage_asset_outputs(stage_id: &str) -> Result<Vec<StageAssetOutput>, String> {
    match stage_id {
        "battlefield" => Ok(vec![StageAssetOutput {
            relative_path: STAGE_ASSET_OUTPUTS[0],
            stage_id: "battlefield",
            stage_name: "Battlefield",
            asset: build_battlefield_stage_asset(),
        }]),
        other => Err(format!(
            "unknown stage id for stage asset generation: {other}"
        )),
    }
}

fn write_outputs(root: &Path, outputs: &[StageAssetOutput]) -> Result<(), String> {
    for output in outputs {
        let path = root.join(output.relative_path);
        let Some(parent) = path.parent() else {
            return Err(format!("missing parent directory for {}", path.display()));
        };
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        let mut text = serde_json::to_string_pretty(&output.asset)
            .map_err(|error| format!("failed to serialize {}: {error}", output.relative_path))?;
        text.push('\n');
        fs::write(&path, text)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn build_battlefield_stage_asset() -> Value {
    let stage = StageProfile::battlefield_test();

    json!({
        "schema_version": SCHEMA_VERSION,
        "stage_id": "battlefield",
        "stage_name": "Battlefield",
        "engine_boundary": "rust_core_authority",
        "source": {
            "kind": "rust_baked_stage_asset_pending_grnba_dat_extract",
            "profile_name": stage.name,
            "reference_stage": "Battlefield",
            "required_raw_dat": "resources/melee/raw/GrNBa.dat",
            "decomp_refs": [
                ".research/doldecomp-melee/src/melee/gr/grbattle.c",
                ".research/doldecomp-melee/src/melee/gr/grdatfiles.c",
                ".research/doldecomp-melee/src/melee/gr/ground.c",
                ".research/doldecomp-melee/src/melee/mp/types.h"
            ],
            "required_public_symbols": [
                "map_head",
                "coll_data",
                "grGroundParam"
            ],
            "notes": "Current numeric values are the Rust baked Battlefield profile. True stage parity requires extracting /GrNBa.dat coll_data into the MapCollData shape used by mpLibLoad."
        },
        "main_floor": surface_to_json(stage.main_floor),
        "soft_platforms": stage.soft_platforms.iter().copied().map(surface_to_json).collect::<Vec<_>>(),
        "blast_zones": {
            "left_x": stage.blast_zones.left_x,
            "right_x": stage.blast_zones.right_x,
            "top_y": stage.blast_zones.top_y,
            "bottom_y": stage.blast_zones.bottom_y,
        },
        "spawn_points": stage.spawn_points.iter().map(|spawn_point| json!({
            "x": spawn_point.x,
            "y": spawn_point.y,
            "facing": spawn_point.facing,
        })).collect::<Vec<_>>(),
    })
}

fn surface_to_json(surface: StageSurface) -> Value {
    json!({
        "name": surface.name,
        "kind": surface_kind_label(surface.kind),
        "left_x": surface.left_x,
        "right_x": surface.right_x,
        "y": surface.y,
        "friction_multiplier": surface.friction_multiplier,
    })
}

fn surface_kind_label(kind: StageSurfaceKind) -> &'static str {
    match kind {
        StageSurfaceKind::Solid => "solid",
        StageSurfaceKind::Soft => "soft",
    }
}
