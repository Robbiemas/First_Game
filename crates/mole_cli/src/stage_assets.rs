use mole_core::{StageProfile, StageSurface, StageSurfaceKind};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::{StageCommand, StageExtractIsoOptions, StageExtractOptions, SCHEMA_VERSION};

pub(crate) const STAGE_ASSETS_GENERATOR: &str = "crates/mole_cli/src/stage_assets.rs";
pub(crate) const STAGE_ASSETS_COMMAND: &str =
    "cargo run -p mole_cli -- stage extract --stage battlefield --write --json";
pub(crate) const STAGE_ASSET_INPUTS: &[&str] = &["resources/melee/raw/GrNBa.dat"];
pub(crate) const STAGE_ASSET_OUTPUTS: &[&str] = &[
    "resources/melee/extracted/stages/battlefield_stage.json",
    "crates/mole_core/src/generated/stages.rs",
];
const RAW_STAGE_DIR: &str = "resources/melee/raw";
const STAGE_ASSET_DIR: &str = "resources/melee/extracted/stages";
const BATTLEFIELD_RAW_DAT: &str = "resources/melee/raw/GrNBa.dat";
const BATTLEFIELD_STAGE_ASSET: &str = "resources/melee/extracted/stages/battlefield_stage.json";
const ENGINE_STAGE_MODULE: &str = "crates/mole_core/src/generated/stages.rs";
const BATTLEFIELD_PROVENANCE: &str = "rust_baked_stage_asset_pending_grnba_dat_extract";
const MELEE_STAGE_DAT_PROVENANCE: &str = "melee_stage_dat";
const COLL_LINE_FLOOR: u16 = 0x1;
const COLL_LINE_CEILING: u16 = 0x2;
const COLL_LINE_RIGHT_WALL: u16 = 0x4;
const COLL_LINE_LEFT_WALL: u16 = 0x8;
const COLL_LINE_SOFT_FLOOR: u16 = 0x100;
const DAT_DATA_BLOCK_BASE: usize = 0x20;
const GCM_FST_OFFSET_FIELD: u64 = 0x424;
const GCM_FST_SIZE_FIELD: u64 = 0x428;

#[derive(Debug, Clone, Copy)]
struct StageSpec {
    id: &'static str,
    name: &'static str,
    dat_file: &'static str,
    decomp_ref: &'static str,
    competitive_kind: Option<&'static str>,
    related_dat_files: &'static [&'static str],
}

const STAGE_SPECS: &[StageSpec] = &[
    StageSpec {
        id: "battlefield",
        name: "Battlefield",
        dat_file: "GrNBa.dat",
        decomp_ref: ".research/doldecomp-melee/src/melee/gr/grbattle.c",
        competitive_kind: Some("starter"),
        related_dat_files: &[],
    },
    StageSpec {
        id: "final-destination",
        name: "Final Destination",
        dat_file: "GrNLa.dat",
        decomp_ref: ".research/doldecomp-melee/src/melee/gr/grlast.c",
        competitive_kind: Some("starter"),
        related_dat_files: &[],
    },
    StageSpec {
        id: "yoshi-story",
        name: "Yoshi's Story",
        dat_file: "GrSt.dat",
        decomp_ref: ".research/doldecomp-melee/src/melee/gr/grstory.c",
        competitive_kind: Some("starter"),
        related_dat_files: &[],
    },
    StageSpec {
        id: "fountain-of-dreams",
        name: "Fountain of Dreams",
        dat_file: "GrIz.dat",
        decomp_ref: ".research/doldecomp-melee/src/melee/gr/grizumi.c",
        competitive_kind: Some("starter"),
        related_dat_files: &[],
    },
    StageSpec {
        id: "dream-land-64",
        name: "Dream Land 64",
        dat_file: "GrOp.dat",
        decomp_ref: ".research/doldecomp-melee/src/melee/gr/groldpupupu.c",
        competitive_kind: Some("starter"),
        related_dat_files: &[],
    },
    StageSpec {
        id: "pokemon-stadium",
        name: "Pokemon Stadium",
        dat_file: "GrPs.dat",
        decomp_ref: ".research/doldecomp-melee/src/melee/gr/grpstadium.c",
        competitive_kind: Some("counterpick"),
        related_dat_files: &["GrPs1.dat", "GrPs2.dat", "GrPs3.dat", "GrPs4.dat"],
    },
];

const COMPETITIVE_STAGE_IDS: &[&str] = &[
    "battlefield",
    "final-destination",
    "yoshi-story",
    "fountain-of-dreams",
    "dream-land-64",
    "pokemon-stadium",
];

pub(crate) fn stage_report(root: &Path, command: &StageCommand) -> Value {
    match command {
        StageCommand::Inspect { stage } => inspect_stage_report(root, stage),
        StageCommand::Extract(options) => stage_extract_report(root, options),
        StageCommand::ExtractIso(options) => stage_extract_iso_report(root, options),
    }
}

fn inspect_stage_report(root: &Path, stage_id: &str) -> Value {
    let Some(resolved) = resolve_stage_input(stage_id, None, None) else {
        return json!({
            "schema_version": SCHEMA_VERSION,
            "command": "stage inspect",
            "project_root": root.display().to_string(),
            "ok": false,
            "stage_id": stage_id,
            "error": format!("unknown stage id for stage inspect: {stage_id}"),
            "known_stages": known_stage_ids(),
            "dat_override_supported": true,
        });
    };

    let raw_dat_path = root.join(&resolved.raw_dat);
    let stage_asset_path = root.join(&resolved.output_path);
    let engine_stage_path = root.join(ENGINE_STAGE_MODULE);
    let asset = fs::read_to_string(&stage_asset_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let baked_asset = if stage_id == "battlefield" {
        Some(build_battlefield_stage_asset())
    } else {
        None
    };
    let asset_source_kind = asset
        .as_ref()
        .and_then(|value| value.get("source"))
        .and_then(|source| source.get("kind"))
        .and_then(Value::as_str)
        .or_else(|| {
            baked_asset
                .as_ref()
                .and_then(|value| value.get("source"))
                .and_then(|source| source.get("kind"))
                .and_then(Value::as_str)
        })
        .unwrap_or("missing_stage_asset");
    let raw_dat_present = raw_dat_path.exists();
    let asset_present = stage_asset_path.exists();
    let engine_stage_present = stage_id != "battlefield" || engine_stage_path.exists();
    let decomp_parity_ready =
        raw_dat_present && asset_source_kind == MELEE_STAGE_DAT_PROVENANCE && engine_stage_present;
    let collision = asset.as_ref().and_then(|value| value.get("collision"));

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "stage inspect",
        "project_root": root.display().to_string(),
        "ok": asset_present || baked_asset.is_some(),
        "stage_id": resolved.stage_id,
        "stage_name": resolved.stage_name,
        "decomp_parity_ready": decomp_parity_ready,
        "status": if decomp_parity_ready {
            "decomp_extracted"
        } else if raw_dat_present {
            "raw_stage_dat_present_pending_extract"
        } else {
            "raw_stage_dat_missing"
        },
        "engine_boundary": "rust_core_authority",
        "raw_dat": {
            "path": resolved.raw_dat,
            "present": raw_dat_present,
            "required_for_decomp_parity": true,
        },
        "asset": {
            "path": resolved.output_path,
            "present": asset_present,
            "source_kind": asset_source_kind,
        },
        "engine_blob": {
            "path": ENGINE_STAGE_MODULE,
            "present": stage_id == "battlefield" && engine_stage_path.exists(),
            "required_for_runtime": stage_id == "battlefield",
        },
        "collision": collision.map(|value| json!({
            "vertex_count": value.get("vertex_count").cloned().unwrap_or(Value::Null),
            "line_count": value.get("line_count").cloned().unwrap_or(Value::Null),
            "joint_count": value.get("joint_count").cloned().unwrap_or(Value::Null),
            "scale": value.get("scale").cloned().unwrap_or(Value::Null),
        })),
        "current_surfaces": surface_summary(asset.as_ref().or(baked_asset.as_ref())),
        "decomp_refs": decomp_refs(&resolved),
        "required_public_symbols": [
            "map_head",
            "coll_data",
            "grGroundParam"
        ],
        "known_stages": known_stage_ids(),
        "dat_override_supported": true,
        "blocking_notes": if decomp_parity_ready {
            Vec::<String>::new()
        } else if raw_dat_present && asset_source_kind != MELEE_STAGE_DAT_PROVENANCE {
            vec![format!(
                "Raw stage DAT is present but {} has not been regenerated from coll_data.",
                resolved.output_path
            )]
        } else if raw_dat_present {
            vec![format!(
                "Extracted stage asset is present, but {} has not been generated for the engine.",
                ENGINE_STAGE_MODULE
            )]
        } else {
            vec![format!(
                "Place {} at {} or pass --dat <path> to stage extract.",
                resolved.dat_file, RAW_STAGE_DIR
            )]
        },
        "recommended_next": if decomp_parity_ready {
            Vec::<String>::new()
        } else {
            vec![format!(
                "Run mole stage extract --stage {} --write after the raw DAT is available.",
                resolved.stage_id
            )]
        },
    })
}

fn stage_extract_report(root: &Path, options: &StageExtractOptions) -> Value {
    let Some(resolved) = resolve_stage_input(
        &options.stage,
        options.stage_name.as_deref(),
        options.dat.as_deref(),
    ) else {
        return json!({
            "schema_version": SCHEMA_VERSION,
            "command": "stage extract",
            "project_root": root.display().to_string(),
            "mutated": false,
            "ok": false,
            "stage_id": options.stage,
            "error": format!("unknown stage id for stage extract: {}", options.stage),
            "known_stages": known_stage_ids(),
            "dat_override_supported": true,
        });
    };

    let raw_path = root.join(&resolved.raw_dat);
    if !raw_path.exists() {
        return json!({
            "schema_version": SCHEMA_VERSION,
            "command": "stage extract",
            "project_root": root.display().to_string(),
            "mutated": false,
            "ok": false,
            "stage_id": resolved.stage_id,
            "stage_name": resolved.stage_name,
            "raw_dat": {
                "path": resolved.raw_dat,
                "present": false,
            },
            "output_path": resolved.output_path,
            "error": format!("raw stage DAT not found: {}", raw_path.display()),
            "known_stages": known_stage_ids(),
            "dat_override_supported": true,
        });
    }

    let payload = match fs::read(&raw_path) {
        Ok(payload) => payload,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "stage_id": resolved.stage_id,
                "stage_name": resolved.stage_name,
                "raw_dat": {
                    "path": resolved.raw_dat,
                    "present": true,
                },
                "output_path": resolved.output_path,
                "error": format!("failed to read {}: {error}", raw_path.display()),
            });
        }
    };

    let asset = match build_stage_asset_from_dat(&resolved, &payload) {
        Ok(asset) => asset,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "stage_id": resolved.stage_id,
                "stage_name": resolved.stage_name,
                "raw_dat": {
                    "path": resolved.raw_dat,
                    "present": true,
                },
                "output_path": resolved.output_path,
                "error": error,
            });
        }
    };
    let engine_stage_blob = match build_engine_stage_module_for_asset(&asset) {
        Ok(blob) => blob,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "stage_id": resolved.stage_id,
                "stage_name": resolved.stage_name,
                "raw_dat": {
                    "path": resolved.raw_dat,
                    "present": true,
                },
                "output_path": resolved.output_path,
                "engine_blob": {
                    "path": ENGINE_STAGE_MODULE,
                    "present": false,
                },
                "error": error,
            });
        }
    };

    let mut written_paths = Vec::new();
    let mut pending_paths = Vec::new();
    if options.write {
        if let Err(error) = write_json(root, &resolved.output_path, &asset) {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "stage_id": resolved.stage_id,
                "stage_name": resolved.stage_name,
                "raw_dat": {
                    "path": resolved.raw_dat,
                    "present": true,
                },
                "output_path": resolved.output_path,
                "error": error,
            });
        }
        written_paths.push(resolved.output_path.clone());
        if let Some(engine_stage_blob) = &engine_stage_blob {
            if let Err(error) = write_text(root, ENGINE_STAGE_MODULE, engine_stage_blob) {
                return json!({
                    "schema_version": SCHEMA_VERSION,
                    "command": "stage extract",
                    "project_root": root.display().to_string(),
                    "mutated": false,
                    "ok": false,
                    "stage_id": resolved.stage_id,
                    "stage_name": resolved.stage_name,
                    "raw_dat": {
                        "path": resolved.raw_dat,
                        "present": true,
                    },
                    "output_path": resolved.output_path,
                    "engine_blob": {
                        "path": ENGINE_STAGE_MODULE,
                        "present": false,
                    },
                    "error": error,
                });
            }
            written_paths.push(ENGINE_STAGE_MODULE.to_string());
        }
    } else {
        pending_paths.push(resolved.output_path.clone());
        if engine_stage_blob.is_some() {
            pending_paths.push(ENGINE_STAGE_MODULE.to_string());
        }
    }

    let collision = asset.get("collision").unwrap_or(&Value::Null);
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "stage extract",
        "project_root": root.display().to_string(),
        "mutated": options.write,
        "ok": true,
        "stage_id": resolved.stage_id,
        "stage_name": resolved.stage_name,
        "raw_dat": {
            "path": resolved.raw_dat,
            "present": true,
        },
        "output_path": resolved.output_path,
        "engine_blob": engine_stage_blob.as_ref().map(|_| json!({
            "path": ENGINE_STAGE_MODULE,
            "present": options.write || root.join(ENGINE_STAGE_MODULE).exists(),
            "kind": "generated_rust_stage_blob",
            "stage_id": resolved.stage_id,
        })),
        "written_paths": written_paths,
        "pending_paths": pending_paths,
        "collision": {
            "scale": collision.get("scale").cloned().unwrap_or(Value::Null),
            "vertex_count": collision.get("vertex_count").cloned().unwrap_or(Value::Null),
            "line_count": collision.get("line_count").cloned().unwrap_or(Value::Null),
            "joint_count": collision.get("joint_count").cloned().unwrap_or(Value::Null),
        },
    })
}

fn stage_extract_iso_report(root: &Path, options: &StageExtractIsoOptions) -> Value {
    let iso_path = project_path(root, &options.iso);
    let specs = match selected_stage_specs(options) {
        Ok(specs) => specs,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract-iso",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "iso": {
                    "path": options.iso,
                    "present": iso_path.exists(),
                },
                "error": error,
                "known_stages": known_stage_ids(),
                "competitive_stages": COMPETITIVE_STAGE_IDS,
            });
        }
    };

    let mut iso = match fs::File::open(&iso_path) {
        Ok(file) => file,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract-iso",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "iso": {
                    "path": options.iso,
                    "present": false,
                },
                "selected_stage_count": specs.len(),
                "error": format!("failed to open ISO {}: {error}", iso_path.display()),
                "known_stages": known_stage_ids(),
                "competitive_stages": COMPETITIVE_STAGE_IDS,
            });
        }
    };
    let iso_files = match read_gcm_file_entries(&mut iso) {
        Ok(files) => files,
        Err(error) => {
            return json!({
                "schema_version": SCHEMA_VERSION,
                "command": "stage extract-iso",
                "project_root": root.display().to_string(),
                "mutated": false,
                "ok": false,
                "iso": {
                    "path": options.iso,
                    "present": true,
                },
                "selected_stage_count": specs.len(),
                "error": error,
                "known_stages": known_stage_ids(),
                "competitive_stages": COMPETITIVE_STAGE_IDS,
            });
        }
    };

    let single_stage_name =
        if options.stages.len() == 1 && !options.competitive && !options.all_registered {
            options.stage_name.as_deref()
        } else {
            None
        };
    let mut stage_reports = Vec::new();
    let mut written_paths = Vec::new();
    let mut pending_paths = Vec::new();
    let mut errors = Vec::new();
    for spec in specs {
        let Some(resolved) = resolve_stage_input(spec.id, single_stage_name, None) else {
            errors.push(format!("unknown registered stage: {}", spec.id));
            continue;
        };
        let payload = match read_gcm_file_payload(&mut iso, &iso_files, spec.dat_file) {
            Ok(payload) => payload,
            Err(error) => {
                errors.push(format!("{}: {error}", spec.id));
                stage_reports.push(json!({
                    "stage_id": spec.id,
                    "stage_name": spec.name,
                    "dat_file": spec.dat_file,
                    "ok": false,
                    "error": error,
                }));
                continue;
            }
        };
        let asset = match build_stage_asset_from_dat(&resolved, &payload) {
            Ok(asset) => asset,
            Err(error) => {
                errors.push(format!("{}: {error}", spec.id));
                stage_reports.push(json!({
                    "stage_id": spec.id,
                    "stage_name": spec.name,
                    "dat_file": spec.dat_file,
                    "ok": false,
                    "error": error,
                }));
                continue;
            }
        };
        let raw_dat_path = format!("{RAW_STAGE_DIR}/{}", spec.dat_file);
        let engine_stage_blob = match build_engine_stage_module_for_asset(&asset) {
            Ok(blob) => blob,
            Err(error) => {
                errors.push(format!("{} engine blob: {error}", spec.id));
                None
            }
        };
        if options.write {
            if let Err(error) = write_bytes(root, &raw_dat_path, &payload) {
                errors.push(error);
            } else {
                written_paths.push(raw_dat_path.clone());
            }
            if let Err(error) = write_json(root, &resolved.output_path, &asset) {
                errors.push(error);
            } else {
                written_paths.push(resolved.output_path.clone());
            }
            if let Some(engine_stage_blob) = &engine_stage_blob {
                if let Err(error) = write_text(root, ENGINE_STAGE_MODULE, engine_stage_blob) {
                    errors.push(error);
                } else {
                    written_paths.push(ENGINE_STAGE_MODULE.to_string());
                }
            }
        } else {
            pending_paths.push(raw_dat_path.clone());
            pending_paths.push(resolved.output_path.clone());
            if engine_stage_blob.is_some() {
                pending_paths.push(ENGINE_STAGE_MODULE.to_string());
            }
        }

        let mut related_raw_dats = Vec::new();
        for dat_file in spec.related_dat_files {
            match read_gcm_file_payload(&mut iso, &iso_files, dat_file) {
                Ok(bytes) => {
                    let raw_path = format!("{RAW_STAGE_DIR}/{dat_file}");
                    if options.write {
                        if let Err(error) = write_bytes(root, &raw_path, &bytes) {
                            errors.push(error);
                        } else {
                            written_paths.push(raw_path.clone());
                        }
                    } else {
                        pending_paths.push(raw_path.clone());
                    }
                    related_raw_dats.push(json!({
                        "dat_file": dat_file,
                        "raw_dat": raw_path,
                        "size_bytes": bytes.len(),
                    }));
                }
                Err(error) => {
                    errors.push(format!("{} related DAT {dat_file}: {error}", spec.id));
                }
            }
        }

        let collision = asset.get("collision").unwrap_or(&Value::Null);
        stage_reports.push(json!({
            "stage_id": spec.id,
            "stage_name": resolved.stage_name,
            "competitive_kind": spec.competitive_kind,
            "ok": true,
            "dat_file": spec.dat_file,
            "raw_dat": raw_dat_path,
            "asset_path": resolved.output_path,
            "engine_blob": engine_stage_blob.as_ref().map(|_| json!({
                "path": ENGINE_STAGE_MODULE,
                "kind": "generated_rust_stage_blob",
            })),
            "related_raw_dats": related_raw_dats,
            "collision": {
                "scale": collision.get("scale").cloned().unwrap_or(Value::Null),
                "vertex_count": collision.get("vertex_count").cloned().unwrap_or(Value::Null),
                "line_count": collision.get("line_count").cloned().unwrap_or(Value::Null),
                "joint_count": collision.get("joint_count").cloned().unwrap_or(Value::Null),
            },
        }));
    }

    json!({
        "schema_version": SCHEMA_VERSION,
        "command": "stage extract-iso",
        "project_root": root.display().to_string(),
        "mutated": options.write,
        "ok": errors.is_empty(),
        "iso": {
            "path": options.iso,
            "present": true,
        },
        "selection": {
            "competitive": options.competitive,
            "all_registered": options.all_registered,
            "explicit_stages": options.stages,
        },
        "selected_stage_count": stage_reports.len(),
        "raw_output_dir": RAW_STAGE_DIR,
        "stage_asset_output_dir": STAGE_ASSET_DIR,
        "engine_stage_module": ENGINE_STAGE_MODULE,
        "competitive_stages": COMPETITIVE_STAGE_IDS,
        "known_stages": known_stage_ids(),
        "written_paths": written_paths,
        "pending_paths": pending_paths,
        "stages": stage_reports,
        "errors": errors,
    })
}

pub(crate) fn write_stage_asset_report(root: &Path, stage_id: &str, write: bool) -> Value {
    match build_stage_asset_outputs(root, stage_id) {
        Ok(outputs) => {
            let mut written_paths = Vec::new();
            let mut pending_paths = Vec::new();
            let mut assets = Vec::new();

            for output in &outputs {
                let engine_stage_blob = match build_engine_stage_module_for_asset(&output.asset) {
                    Ok(blob) => blob,
                    Err(error) => {
                        return json!({
                            "schema_version": SCHEMA_VERSION,
                            "command": "generated write-stage-asset",
                            "project_root": root.display().to_string(),
                            "mutated": false,
                            "ok": false,
                            "stage_id": stage_id,
                            "error": error,
                        });
                    }
                };

                if write {
                    if let Err(error) = write_json(root, &output.relative_path, &output.asset) {
                        return json!({
                            "schema_version": SCHEMA_VERSION,
                            "command": "generated write-stage-asset",
                            "project_root": root.display().to_string(),
                            "mutated": false,
                            "ok": false,
                            "error": error,
                        });
                    }
                    written_paths.push(output.relative_path.to_string());
                    if let Some(engine_stage_blob) = &engine_stage_blob {
                        if let Err(error) = write_text(root, ENGINE_STAGE_MODULE, engine_stage_blob)
                        {
                            return json!({
                                "schema_version": SCHEMA_VERSION,
                                "command": "generated write-stage-asset",
                                "project_root": root.display().to_string(),
                                "mutated": false,
                                "ok": false,
                                "error": error,
                            });
                        }
                        written_paths.push(ENGINE_STAGE_MODULE.to_string());
                    }
                } else {
                    pending_paths.push(output.relative_path.to_string());
                    if engine_stage_blob.is_some() {
                        pending_paths.push(ENGINE_STAGE_MODULE.to_string());
                    }
                }

                let surfaces = output
                    .asset
                    .get("soft_platforms")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                assets.push(json!({
                    "path": output.relative_path,
                    "stage_id": output.stage_id,
                    "stage_name": output.stage_name,
                    "surface_count": surfaces + 1,
                    "engine_blob": engine_stage_blob.as_ref().map(|_| json!({
                        "path": ENGINE_STAGE_MODULE,
                        "kind": "generated_rust_stage_blob",
                    })),
                }));
            }

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
                "pending_paths": pending_paths,
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

#[derive(Debug, Clone)]
struct ResolvedStageInput {
    stage_id: String,
    stage_name: String,
    dat_file: String,
    raw_dat: String,
    output_path: String,
    decomp_ref: Option<&'static str>,
}

struct StageAssetOutput {
    relative_path: String,
    stage_id: String,
    stage_name: String,
    asset: Value,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageAsset {
    stage_id: String,
    stage_name: String,
    source: ExtractedStageSource,
    collision: ExtractedStageCollision,
    main_floor: Option<ExtractedStageSurface>,
    soft_platforms: Vec<ExtractedStageSurface>,
    blast_zones: Option<ExtractedStageBlastZones>,
    spawn_points: Option<Vec<ExtractedStageSpawnPoint>>,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageSource {
    kind: String,
    dat_file: Option<String>,
    decomp_refs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageCollision {
    scale: f32,
    vertices: Vec<ExtractedStageVertex>,
    lines: Vec<ExtractedStageLine>,
    joints: Vec<ExtractedStageJoint>,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageVertex {
    index: u16,
    source_x: f32,
    source_y: f32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageLine {
    index: u16,
    kind: String,
    passable: bool,
    v0_idx: u16,
    v1_idx: u16,
    prev_id0: i16,
    next_id0: i16,
    prev_id1: i16,
    next_id1: i16,
    hi_flags: u16,
    lo_flags: u16,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageJoint {
    index: u16,
    floor_start: i16,
    floor_count: i16,
    ceiling_start: i16,
    ceiling_count: i16,
    right_wall_start: i16,
    right_wall_count: i16,
    left_wall_start: i16,
    left_wall_count: i16,
    dynamic_start: i16,
    dynamic_count: i16,
    bounds: ExtractedStageBounds,
    vtx_start: i16,
    vtx_count: i16,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageBounds {
    left: i32,
    bottom: i32,
    right: i32,
    top: i32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageSurface {
    name: String,
    kind: String,
    left_x: i32,
    right_x: i32,
    y: i32,
    friction_multiplier: f32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageBlastZones {
    left_x: i32,
    right_x: i32,
    top_y: i32,
    bottom_y: i32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageSpawnPoint {
    x: i32,
    y: i32,
    facing: i8,
}

fn build_engine_stage_module_for_asset(asset: &Value) -> Result<Option<String>, String> {
    let stage_id = asset
        .get("stage_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let source_kind = asset
        .get("source")
        .and_then(|source| source.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if source_kind != MELEE_STAGE_DAT_PROVENANCE || stage_id != "battlefield" {
        return Ok(None);
    }

    let stage: ExtractedStageAsset = serde_json::from_value(asset.clone())
        .map_err(|error| format!("stage asset cannot be compiled into engine blob: {error}"))?;

    let main_floor = stage
        .main_floor
        .as_ref()
        .ok_or_else(|| "Battlefield engine blob requires a main_floor surface".to_string())?;
    let blast_zones = stage
        .blast_zones
        .as_ref()
        .ok_or_else(|| "Battlefield engine blob requires blast_zones".to_string())?;
    let spawn_points = stage
        .spawn_points
        .as_ref()
        .ok_or_else(|| "Battlefield engine blob requires spawn_points".to_string())?;

    let mut text = String::new();
    text.push_str("// @generated by mole_cli stage extract; do not edit by hand.\n");
    text.push_str("use crate::stage::{\n");
    text.push_str(
        "    MeleeStageProfile, StageBlastZones, StageCollisionJoint, StageCollisionLine,\n",
    );
    text.push_str(
        "    StageCollisionLineKind, StageCollisionProfile, StageCollisionVertex, StageSource,\n",
    );
    text.push_str("    StageSpawnPoint, StageSurface, StageSurfaceKind,\n");
    text.push_str("};\n\n");

    writeln!(
        text,
        "pub(crate) const BATTLEFIELD_STAGE: MeleeStageProfile = MeleeStageProfile {{"
    )
    .unwrap();
    writeln!(text, "    id: {},", rust_str(&stage.stage_id)).unwrap();
    writeln!(text, "    name: {},", rust_str(&stage.stage_name)).unwrap();
    writeln!(text, "    source: StageSource {{").unwrap();
    writeln!(text, "        kind: {},", rust_str(&stage.source.kind)).unwrap();
    writeln!(
        text,
        "        dat_file: {},",
        rust_str(stage.source.dat_file.as_deref().unwrap_or(""))
    )
    .unwrap();
    writeln!(
        text,
        "        decomp_ref: {},",
        rust_str(
            stage
                .source
                .decomp_refs
                .as_ref()
                .and_then(|refs| refs.first())
                .map(String::as_str)
                .unwrap_or("")
        )
    )
    .unwrap();
    writeln!(text, "    }},").unwrap();
    writeln!(text, "    collision: StageCollisionProfile {{").unwrap();
    writeln!(text, "        scale: BATTLEFIELD_COLLISION_SCALE,").unwrap();
    writeln!(text, "        vertices: &BATTLEFIELD_COLLISION_VERTICES,").unwrap();
    writeln!(text, "        lines: &BATTLEFIELD_COLLISION_LINES,").unwrap();
    writeln!(text, "        joints: &BATTLEFIELD_COLLISION_JOINTS,").unwrap();
    writeln!(text, "    }},").unwrap();
    writeln!(
        text,
        "    main_floor: {},",
        stage_surface_literal(main_floor)?
    )
    .unwrap();
    writeln!(text, "    soft_platforms: [").unwrap();
    for index in 0..3 {
        if let Some(surface) = stage.soft_platforms.get(index) {
            writeln!(text, "        {},", stage_surface_literal(surface)?).unwrap();
        } else {
            writeln!(text, "        {},", hidden_stage_surface_literal(index)).unwrap();
        }
    }
    writeln!(text, "    ],").unwrap();
    writeln!(
        text,
        "    blast_zones: StageBlastZones {{ left_x: {}, right_x: {}, top_y: {}, bottom_y: {} }},",
        blast_zones.left_x, blast_zones.right_x, blast_zones.top_y, blast_zones.bottom_y
    )
    .unwrap();
    writeln!(text, "    spawn_points: [").unwrap();
    for index in 0..4 {
        if let Some(spawn) = spawn_points.get(index) {
            writeln!(
                text,
                "        StageSpawnPoint {{ x: {}, y: {}, facing: {} }},",
                spawn.x, spawn.y, spawn.facing
            )
            .unwrap();
        } else {
            writeln!(text, "        StageSpawnPoint {{ x: 0, y: 0, facing: 1 }},").unwrap();
        }
    }
    writeln!(text, "    ],").unwrap();
    writeln!(text, "}};\n").unwrap();

    writeln!(
        text,
        "const BATTLEFIELD_COLLISION_SCALE: f32 = {};",
        rust_f32(stage.collision.scale)?
    )
    .unwrap();
    writeln!(
        text,
        "const BATTLEFIELD_COLLISION_VERTICES: [StageCollisionVertex; {}] = [",
        stage.collision.vertices.len()
    )
    .unwrap();
    for vertex in &stage.collision.vertices {
        writeln!(
            text,
            "    StageCollisionVertex {{ index: {}, source_x: {}, source_y: {} }},",
            vertex.index,
            rust_f32(vertex.source_x)?,
            rust_f32(vertex.source_y)?
        )
        .unwrap();
    }
    writeln!(text, "];\n").unwrap();

    writeln!(
        text,
        "const BATTLEFIELD_COLLISION_LINES: [StageCollisionLine; {}] = [",
        stage.collision.lines.len()
    )
    .unwrap();
    for line in &stage.collision.lines {
        writeln!(text, "    StageCollisionLine {{").unwrap();
        writeln!(text, "        index: {},", line.index).unwrap();
        writeln!(
            text,
            "        kind: {},",
            stage_line_kind_literal(&line.kind)?
        )
        .unwrap();
        writeln!(text, "        passable: {},", line.passable).unwrap();
        writeln!(text, "        v0_idx: {},", line.v0_idx).unwrap();
        writeln!(text, "        v1_idx: {},", line.v1_idx).unwrap();
        writeln!(text, "        prev_id0: {},", line.prev_id0).unwrap();
        writeln!(text, "        next_id0: {},", line.next_id0).unwrap();
        writeln!(text, "        prev_id1: {},", line.prev_id1).unwrap();
        writeln!(text, "        next_id1: {},", line.next_id1).unwrap();
        writeln!(text, "        hi_flags: {},", line.hi_flags).unwrap();
        writeln!(text, "        lo_flags: {},", line.lo_flags).unwrap();
        writeln!(text, "    }},").unwrap();
    }
    writeln!(text, "];\n").unwrap();

    writeln!(
        text,
        "const BATTLEFIELD_COLLISION_JOINTS: [StageCollisionJoint; {}] = [",
        stage.collision.joints.len()
    )
    .unwrap();
    for joint in &stage.collision.joints {
        writeln!(text, "    StageCollisionJoint {{").unwrap();
        writeln!(text, "        index: {},", joint.index).unwrap();
        writeln!(text, "        floor_start: {},", joint.floor_start).unwrap();
        writeln!(text, "        floor_count: {},", joint.floor_count).unwrap();
        writeln!(text, "        ceiling_start: {},", joint.ceiling_start).unwrap();
        writeln!(text, "        ceiling_count: {},", joint.ceiling_count).unwrap();
        writeln!(
            text,
            "        right_wall_start: {},",
            joint.right_wall_start
        )
        .unwrap();
        writeln!(
            text,
            "        right_wall_count: {},",
            joint.right_wall_count
        )
        .unwrap();
        writeln!(text, "        left_wall_start: {},", joint.left_wall_start).unwrap();
        writeln!(text, "        left_wall_count: {},", joint.left_wall_count).unwrap();
        writeln!(text, "        dynamic_start: {},", joint.dynamic_start).unwrap();
        writeln!(text, "        dynamic_count: {},", joint.dynamic_count).unwrap();
        writeln!(text, "        left_bound_milli: {},", joint.bounds.left).unwrap();
        writeln!(text, "        bottom_bound_milli: {},", joint.bounds.bottom).unwrap();
        writeln!(text, "        right_bound_milli: {},", joint.bounds.right).unwrap();
        writeln!(text, "        top_bound_milli: {},", joint.bounds.top).unwrap();
        writeln!(text, "        vtx_start: {},", joint.vtx_start).unwrap();
        writeln!(text, "        vtx_count: {},", joint.vtx_count).unwrap();
        writeln!(text, "    }},").unwrap();
    }
    writeln!(text, "];").unwrap();

    Ok(Some(text))
}

fn rust_str(value: &str) -> String {
    serde_json::to_string(value).expect("string literal should serialize")
}

fn rust_f32(value: f32) -> Result<String, String> {
    if value.is_finite() {
        Ok(format!("{value:?}_f32"))
    } else {
        Err("stage engine blob cannot contain non-finite f32 values".to_string())
    }
}

fn stage_surface_literal(surface: &ExtractedStageSurface) -> Result<String, String> {
    Ok(format!(
        "StageSurface {{ name: {}, kind: {}, left_x: {}, right_x: {}, y: {}, friction_multiplier: {} }}",
        rust_str(&surface.name),
        stage_surface_kind_literal(&surface.kind)?,
        surface.left_x,
        surface.right_x,
        surface.y,
        rust_f32(surface.friction_multiplier)?
    ))
}

fn hidden_stage_surface_literal(index: usize) -> String {
    format!(
        "StageSurface {{ name: {}, kind: StageSurfaceKind::Soft, left_x: 10000000, right_x: 10000000, y: 10000000, friction_multiplier: 1.0_f32 }}",
        rust_str(&format!("unused_stage_platform_{index}"))
    )
}

fn stage_surface_kind_literal(kind: &str) -> Result<&'static str, String> {
    match kind {
        "solid" => Ok("StageSurfaceKind::Solid"),
        "soft" => Ok("StageSurfaceKind::Soft"),
        other => Err(format!(
            "unknown stage surface kind for engine blob: {other}"
        )),
    }
}

fn stage_line_kind_literal(kind: &str) -> Result<&'static str, String> {
    match kind {
        "floor" => Ok("StageCollisionLineKind::Floor"),
        "soft_floor" => Ok("StageCollisionLineKind::SoftFloor"),
        "ceiling" => Ok("StageCollisionLineKind::Ceiling"),
        "right_wall" => Ok("StageCollisionLineKind::RightWall"),
        "left_wall" => Ok("StageCollisionLineKind::LeftWall"),
        "dynamic" => Ok("StageCollisionLineKind::Dynamic"),
        other => Err(format!(
            "unknown stage collision line kind for engine blob: {other}"
        )),
    }
}

#[derive(Debug, Clone)]
struct DatRoots {
    data_block_size: usize,
    relocation_count: usize,
    external_count: usize,
    roots: BTreeMap<String, u32>,
}

#[derive(Debug, Clone)]
struct MapCollData {
    verts_offset: u32,
    vert_count: usize,
    lines_offset: u32,
    line_count: usize,
    floor_start: i16,
    floor_count: i16,
    ceiling_start: i16,
    ceiling_count: i16,
    right_wall_start: i16,
    right_wall_count: i16,
    left_wall_start: i16,
    left_wall_count: i16,
    dynamic_start: i16,
    dynamic_count: i16,
    joints_offset: u32,
    joint_count: usize,
    x2c: i32,
}

#[derive(Debug, Clone)]
struct CollVertex {
    source_x: f32,
    source_y: f32,
    x: i32,
    y: i32,
}

#[derive(Debug, Clone)]
struct MapLine {
    v0_idx: u16,
    v1_idx: u16,
    prev_id0: i16,
    next_id0: i16,
    prev_id1: i16,
    next_id1: i16,
    hi_flags: u16,
    lo_flags: u16,
}

#[derive(Debug, Clone)]
struct MapJoint {
    floor_start: i16,
    floor_count: i16,
    ceiling_start: i16,
    ceiling_count: i16,
    right_wall_start: i16,
    right_wall_count: i16,
    left_wall_start: i16,
    left_wall_count: i16,
    dynamic_start: i16,
    dynamic_count: i16,
    left_bound: f32,
    bottom_bound: f32,
    right_bound: f32,
    top_bound: f32,
    vtx_start: i16,
    vtx_count: i16,
}

#[derive(Debug, Clone)]
struct DerivedSurface {
    name: String,
    kind: &'static str,
    left_x: i32,
    right_x: i32,
    y: i32,
}

fn resolve_stage_input(
    stage_id: &str,
    stage_name_override: Option<&str>,
    dat_override: Option<&str>,
) -> Option<ResolvedStageInput> {
    let spec = stage_spec(stage_id);
    let dat_file = dat_override
        .map(stage_dat_file_name)
        .or_else(|| spec.map(|spec| spec.dat_file.to_string()))?;
    let raw_dat = dat_override
        .map(normalize_dat_path)
        .unwrap_or_else(|| format!("{RAW_STAGE_DIR}/{dat_file}"));
    let stage_name = stage_name_override
        .map(str::to_string)
        .or_else(|| spec.map(|spec| spec.name.to_string()))
        .unwrap_or_else(|| humanize_stage_id(stage_id));
    Some(ResolvedStageInput {
        stage_id: stage_id.to_string(),
        stage_name,
        dat_file,
        raw_dat,
        output_path: stage_asset_path(stage_id),
        decomp_ref: spec.map(|spec| spec.decomp_ref),
    })
}

fn stage_spec(stage_id: &str) -> Option<StageSpec> {
    STAGE_SPECS.iter().copied().find(|spec| spec.id == stage_id)
}

fn known_stage_ids() -> Vec<&'static str> {
    STAGE_SPECS.iter().map(|spec| spec.id).collect()
}

fn selected_stage_specs(options: &StageExtractIsoOptions) -> Result<Vec<StageSpec>, String> {
    let mut selected = Vec::new();
    let mut seen = BTreeMap::new();
    let mut push_stage = |stage_id: &str| -> Result<(), String> {
        let Some(spec) = stage_spec(stage_id) else {
            return Err(format!("unknown stage id for ISO extraction: {stage_id}"));
        };
        if !seen.contains_key(spec.id) {
            seen.insert(spec.id, true);
            selected.push(spec);
        }
        Ok(())
    };

    for stage in &options.stages {
        push_stage(stage)?;
    }
    if options.competitive {
        for stage in COMPETITIVE_STAGE_IDS {
            push_stage(stage)?;
        }
    }
    if options.all_registered {
        for spec in STAGE_SPECS {
            push_stage(spec.id)?;
        }
    }
    Ok(selected)
}

fn project_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn normalize_dat_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.contains('/') {
        normalized
    } else {
        format!("{RAW_STAGE_DIR}/{normalized}")
    }
}

fn stage_dat_file_name(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn stage_asset_path(stage_id: &str) -> String {
    format!(
        "{STAGE_ASSET_DIR}/{}_stage.json",
        sanitize_stage_id(stage_id)
    )
}

fn sanitize_stage_id(stage_id: &str) -> String {
    stage_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn humanize_stage_id(stage_id: &str) -> String {
    stage_id
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().collect::<String>();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_stage_asset_outputs(root: &Path, stage_id: &str) -> Result<Vec<StageAssetOutput>, String> {
    match stage_id {
        "battlefield" => {
            let Some(resolved) = resolve_stage_input("battlefield", None, None) else {
                return Err("failed to resolve registered Battlefield stage".to_string());
            };
            let raw_path = root.join(&resolved.raw_dat);
            let asset = if raw_path.exists() {
                let payload = fs::read(&raw_path)
                    .map_err(|error| format!("failed to read {}: {error}", raw_path.display()))?;
                build_stage_asset_from_dat(&resolved, &payload)?
            } else {
                build_battlefield_stage_asset()
            };
            Ok(vec![StageAssetOutput {
                relative_path: BATTLEFIELD_STAGE_ASSET.to_string(),
                stage_id: "battlefield".to_string(),
                stage_name: "Battlefield".to_string(),
                asset,
            }])
        }
        other => Err(format!(
            "unknown stage id for stage asset generation: {other}"
        )),
    }
}

fn write_json(root: &Path, relative_path: &str, value: &Value) -> Result<(), String> {
    let path = root.join(relative_path);
    let Some(parent) = path.parent() else {
        return Err(format!("missing parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {relative_path}: {error}"))?;
    text.push('\n');
    fs::write(&path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn write_text(root: &Path, relative_path: &str, text: &str) -> Result<(), String> {
    let path = root.join(relative_path);
    let Some(parent) = path.parent() else {
        return Err(format!("missing parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    fs::write(&path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn write_bytes(root: &Path, relative_path: &str, bytes: &[u8]) -> Result<(), String> {
    let path = root.join(relative_path);
    let Some(parent) = path.parent() else {
        return Err(format!("missing parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    fs::write(&path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[derive(Debug, Clone)]
struct GcmFileEntry {
    path: String,
    name: String,
    offset: u64,
    size: usize,
}

fn read_gcm_file_entries<R: Read + Seek>(reader: &mut R) -> Result<Vec<GcmFileEntry>, String> {
    let fst_offset = read_u32_be_at(reader, GCM_FST_OFFSET_FIELD, "GCM.fst_offset")? as u64;
    let fst_size = read_u32_be_at(reader, GCM_FST_SIZE_FIELD, "GCM.fst_size")? as usize;
    if fst_size < 12 {
        return Err("GCM FST is too small to contain a root entry".to_string());
    }
    let mut fst = vec![0u8; fst_size];
    reader
        .seek(SeekFrom::Start(fst_offset))
        .map_err(|error| format!("failed to seek to GCM FST at 0x{fst_offset:X}: {error}"))?;
    reader
        .read_exact(&mut fst)
        .map_err(|error| format!("failed to read GCM FST: {error}"))?;

    let entry_count = read_u32_be_slice(&fst, 0x08, "GCM.root_entry_count")? as usize;
    let entry_bytes = entry_count
        .checked_mul(12)
        .ok_or_else(|| "GCM FST entry table size overflowed".to_string())?;
    if entry_count == 0 || entry_bytes > fst.len() {
        return Err("GCM FST root entry count points outside the FST".to_string());
    }
    let string_table = &fst[entry_bytes..];
    let mut files = Vec::new();
    collect_gcm_dir_entries(&fst, string_table, 1, entry_count, "", &mut files)?;
    Ok(files)
}

fn collect_gcm_dir_entries(
    fst: &[u8],
    string_table: &[u8],
    mut index: usize,
    end: usize,
    parent_path: &str,
    files: &mut Vec<GcmFileEntry>,
) -> Result<usize, String> {
    while index < end {
        let entry_offset = index
            .checked_mul(12)
            .ok_or_else(|| "GCM FST entry offset overflowed".to_string())?;
        let type_name = read_u32_be_slice(fst, entry_offset, "GCM.FST.type_name")?;
        let is_dir = (type_name >> 24) != 0;
        let name_offset = (type_name & 0x00FF_FFFF) as usize;
        let name = gcm_string(string_table, name_offset)?;
        let path = if parent_path.is_empty() {
            name.clone()
        } else {
            format!("{parent_path}/{name}")
        };
        if is_dir {
            let next_index =
                read_u32_be_slice(fst, entry_offset + 0x08, "GCM.FST.dir_next_index")? as usize;
            if next_index <= index || next_index > end {
                return Err(format!(
                    "GCM directory {path} has invalid next index {next_index}"
                ));
            }
            index =
                collect_gcm_dir_entries(fst, string_table, index + 1, next_index, &path, files)?;
        } else {
            let offset = read_u32_be_slice(fst, entry_offset + 0x04, "GCM.FST.file_offset")? as u64;
            let size = read_u32_be_slice(fst, entry_offset + 0x08, "GCM.FST.file_size")? as usize;
            files.push(GcmFileEntry {
                path,
                name,
                offset,
                size,
            });
            index += 1;
        }
    }
    Ok(index)
}

fn read_gcm_file_payload<R: Read + Seek>(
    reader: &mut R,
    files: &[GcmFileEntry],
    requested_name: &str,
) -> Result<Vec<u8>, String> {
    let normalized = requested_name.replace('\\', "/");
    let entry = files
        .iter()
        .find(|entry| entry.path.eq_ignore_ascii_case(&normalized))
        .or_else(|| {
            files
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(&normalized))
        })
        .ok_or_else(|| format!("GCM file not found in ISO: {requested_name}"))?;
    let mut payload = vec![0u8; entry.size];
    reader
        .seek(SeekFrom::Start(entry.offset))
        .map_err(|error| {
            format!(
                "failed to seek to {} at 0x{:X} in ISO: {error}",
                entry.path, entry.offset
            )
        })?;
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("failed to read {} from ISO: {error}", entry.path))?;
    Ok(payload)
}

fn read_u32_be_at<R: Read + Seek>(reader: &mut R, offset: u64, label: &str) -> Result<u32, String> {
    let mut bytes = [0u8; 4];
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|error| format!("failed to seek {label} at 0x{offset:X}: {error}"))?;
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("failed to read {label}: {error}"))?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_u32_be_slice(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let range = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{label} at 0x{offset:X} is outside the byte slice"))?;
    Ok(u32::from_be_bytes([range[0], range[1], range[2], range[3]]))
}

fn gcm_string(string_table: &[u8], offset: usize) -> Result<String, String> {
    if offset >= string_table.len() {
        return Err(format!(
            "GCM FST string offset 0x{offset:X} is outside the string table"
        ));
    }
    let end = string_table[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|relative| offset + relative)
        .ok_or_else(|| format!("GCM FST string at 0x{offset:X} is not nul-terminated"))?;
    Ok(String::from_utf8_lossy(&string_table[offset..end]).to_string())
}

fn build_stage_asset_from_dat(stage: &ResolvedStageInput, dat: &[u8]) -> Result<Value, String> {
    let roots = parse_dat_roots(dat)?;
    let coll_offset = *roots
        .roots
        .get("coll_data")
        .ok_or_else(|| "stage DAT is missing public root coll_data".to_string())?;
    let scale = roots
        .roots
        .get("grGroundParam")
        .map(|offset| {
            read_data_f32(
                dat,
                roots.data_block_size,
                *offset as usize,
                "grGroundParam.x0",
            )
        })
        .transpose()?
        .unwrap_or(1.0);
    let coll = parse_map_coll_data(dat, &roots, coll_offset)?;
    let vertices = parse_vertices(dat, &roots, &coll, scale)?;
    let lines = parse_lines(dat, &roots, &coll)?;
    let joints = parse_joints(dat, &roots, &coll, scale)?;
    let mut surfaces = derive_surfaces(&lines, &vertices, &coll);
    canonicalize_stage_surfaces(&stage.stage_id, &mut surfaces);
    let main_floor = surfaces
        .iter()
        .filter(|surface| surface.kind == "solid")
        .max_by_key(|surface| surface.right_x - surface.left_x)
        .or_else(|| {
            surfaces
                .iter()
                .max_by_key(|surface| surface.right_x - surface.left_x)
        })
        .cloned();
    let soft_platforms = surfaces
        .iter()
        .filter(|surface| surface.kind == "soft")
        .cloned()
        .collect::<Vec<_>>();

    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "stage_id": stage.stage_id,
        "stage_name": stage.stage_name,
        "engine_boundary": "rust_core_authority",
        "source": {
            "kind": MELEE_STAGE_DAT_PROVENANCE,
            "raw_dat": stage.raw_dat,
            "dat_file": stage.dat_file,
            "competitive_kind": stage_spec(&stage.stage_id).and_then(|spec| spec.competitive_kind),
            "related_dat_files": stage_spec(&stage.stage_id)
                .map(|spec| spec.related_dat_files.to_vec())
                .unwrap_or_default(),
            "data_block_size": roots.data_block_size,
            "relocation_count": roots.relocation_count,
            "external_count": roots.external_count,
            "public_roots": roots.roots.keys().cloned().collect::<Vec<_>>(),
            "decomp_refs": decomp_refs(stage),
            "struct_refs": [
                ".research/doldecomp-melee/src/melee/mp/types.h::MapCollData",
                ".research/doldecomp-melee/src/melee/mp/types.h::MapLine",
                ".research/doldecomp-melee/src/melee/mp/types.h::MapJoint",
                ".research/doldecomp-melee/src/melee/mp/mplib.c::mpLibLoad"
            ],
            "notes": "Collision vertices preserve raw DAT source coordinates and scaled engine milli-units matching mpLibLoad/Ground_801C0498."
        },
        "collision": {
            "source_root": "coll_data",
            "coll_data_offset": coll_offset,
            "scale": scale,
            "vertex_count": coll.vert_count,
            "line_count": coll.line_count,
            "joint_count": coll.joint_count,
            "line_ranges": {
                "floor": {"start": coll.floor_start, "count": coll.floor_count},
                "ceiling": {"start": coll.ceiling_start, "count": coll.ceiling_count},
                "right_wall": {"start": coll.right_wall_start, "count": coll.right_wall_count},
                "left_wall": {"start": coll.left_wall_start, "count": coll.left_wall_count},
                "dynamic": {"start": coll.dynamic_start, "count": coll.dynamic_count}
            },
            "x2c": coll.x2c,
            "vertices": vertices.iter().enumerate().map(|(index, vertex)| json!({
                "index": index,
                "source_x": vertex.source_x,
                "source_y": vertex.source_y,
                "x": vertex.x,
                "y": vertex.y,
            })).collect::<Vec<_>>(),
            "lines": lines.iter().enumerate().map(|(index, line)| line_to_json(index, line, &vertices, &coll)).collect::<Vec<_>>(),
            "joints": joints.iter().enumerate().map(|(index, joint)| joint_to_json(index, joint, scale)).collect::<Vec<_>>(),
        },
        "main_floor": main_floor.as_ref().map(surface_to_value),
        "soft_platforms": soft_platforms.iter().map(surface_to_value).collect::<Vec<_>>(),
        "blast_zones": if stage.stage_id == "battlefield" {
            Some(build_battlefield_stage_asset()["blast_zones"].clone())
        } else {
            None
        },
        "spawn_points": if stage.stage_id == "battlefield" {
            Some(build_battlefield_stage_asset()["spawn_points"].clone())
        } else {
            None
        },
        "pending_stage_layers": [
            "map_head_object_tree",
            "map_plit_spawn_points",
            "itemdata",
            "camera_bounds",
            "blast_zones"
        ],
    }))
}

fn parse_dat_roots(dat: &[u8]) -> Result<DatRoots, String> {
    if dat.len() < DAT_DATA_BLOCK_BASE {
        return Err("DAT file is shorter than the 0x20-byte header".to_string());
    }
    let data_block_size = read_u32(dat, 0x04, "data_block_size")? as usize;
    let relocation_count = read_u32(dat, 0x08, "relocation_count")? as usize;
    let root_count = read_u32(dat, 0x0C, "root_count")? as usize;
    let external_count = read_u32(dat, 0x10, "external_count")? as usize;
    let root_table_offset = DAT_DATA_BLOCK_BASE + data_block_size + relocation_count * 4;
    let external_table_offset = root_table_offset + root_count * 8;
    let string_table_offset = external_table_offset + external_count * 8;
    if string_table_offset > dat.len() {
        return Err("DAT root/string tables are outside the file".to_string());
    }
    let mut roots = BTreeMap::new();
    for index in 0..root_count {
        let entry = root_table_offset + index * 8;
        let data_offset = read_u32(dat, entry, "root data offset")?;
        let string_offset = read_u32(dat, entry + 4, "root string offset")? as usize;
        let name = dat_string(dat, string_table_offset + string_offset)?;
        roots.insert(name, data_offset);
    }
    Ok(DatRoots {
        data_block_size,
        relocation_count,
        external_count,
        roots,
    })
}

fn parse_map_coll_data(dat: &[u8], roots: &DatRoots, offset: u32) -> Result<MapCollData, String> {
    let offset = offset as usize;
    Ok(MapCollData {
        verts_offset: read_data_u32(dat, roots.data_block_size, offset, "MapCollData.verts")?,
        vert_count: read_data_u32(
            dat,
            roots.data_block_size,
            offset + 0x04,
            "MapCollData.vert_count",
        )? as usize,
        lines_offset: read_data_u32(
            dat,
            roots.data_block_size,
            offset + 0x08,
            "MapCollData.lines",
        )?,
        line_count: read_data_u32(
            dat,
            roots.data_block_size,
            offset + 0x0C,
            "MapCollData.line_count",
        )? as usize,
        floor_start: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x10,
            "MapCollData.floor_start",
        )?,
        floor_count: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x12,
            "MapCollData.floor_count",
        )?,
        ceiling_start: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x14,
            "MapCollData.ceiling_start",
        )?,
        ceiling_count: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x16,
            "MapCollData.ceiling_count",
        )?,
        right_wall_start: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x18,
            "MapCollData.right_wall_start",
        )?,
        right_wall_count: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x1A,
            "MapCollData.right_wall_count",
        )?,
        left_wall_start: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x1C,
            "MapCollData.left_wall_start",
        )?,
        left_wall_count: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x1E,
            "MapCollData.left_wall_count",
        )?,
        dynamic_start: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x20,
            "MapCollData.dynamic_start",
        )?,
        dynamic_count: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x22,
            "MapCollData.dynamic_count",
        )?,
        joints_offset: read_data_u32(
            dat,
            roots.data_block_size,
            offset + 0x24,
            "MapCollData.joints",
        )?,
        joint_count: read_data_u32(
            dat,
            roots.data_block_size,
            offset + 0x28,
            "MapCollData.joint_count",
        )? as usize,
        x2c: read_data_i32(dat, roots.data_block_size, offset + 0x2C, "MapCollData.x2C")?,
    })
}

fn parse_vertices(
    dat: &[u8],
    roots: &DatRoots,
    coll: &MapCollData,
    scale: f32,
) -> Result<Vec<CollVertex>, String> {
    let mut vertices = Vec::with_capacity(coll.vert_count);
    for index in 0..coll.vert_count {
        let offset = coll.verts_offset as usize + index * 0x08;
        let source_x = read_data_f32(dat, roots.data_block_size, offset, "CollVtx.x")?;
        let source_y = read_data_f32(dat, roots.data_block_size, offset + 0x04, "CollVtx.y")?;
        vertices.push(CollVertex {
            source_x,
            source_y,
            x: source_units_to_milli(source_x * scale),
            y: source_units_to_milli(source_y * scale),
        });
    }
    Ok(vertices)
}

fn parse_lines(dat: &[u8], roots: &DatRoots, coll: &MapCollData) -> Result<Vec<MapLine>, String> {
    let mut lines = Vec::with_capacity(coll.line_count);
    for index in 0..coll.line_count {
        let offset = coll.lines_offset as usize + index * 0x10;
        lines.push(MapLine {
            v0_idx: read_data_u16(dat, roots.data_block_size, offset, "MapLine.v0_idx")?,
            v1_idx: read_data_u16(dat, roots.data_block_size, offset + 0x02, "MapLine.v1_idx")?,
            prev_id0: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x04,
                "MapLine.prev_id0",
            )?,
            next_id0: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x06,
                "MapLine.next_id0",
            )?,
            prev_id1: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x08,
                "MapLine.prev_id1",
            )?,
            next_id1: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x0A,
                "MapLine.next_id1",
            )?,
            hi_flags: read_data_u16(
                dat,
                roots.data_block_size,
                offset + 0x0C,
                "MapLine.hi_flags",
            )?,
            lo_flags: read_data_u16(
                dat,
                roots.data_block_size,
                offset + 0x0E,
                "MapLine.lo_flags",
            )?,
        });
    }
    Ok(lines)
}

fn parse_joints(
    dat: &[u8],
    roots: &DatRoots,
    coll: &MapCollData,
    _scale: f32,
) -> Result<Vec<MapJoint>, String> {
    let mut joints = Vec::with_capacity(coll.joint_count);
    for index in 0..coll.joint_count {
        let offset = coll.joints_offset as usize + index * 0x28;
        joints.push(MapJoint {
            floor_start: read_data_i16(dat, roots.data_block_size, offset, "MapJoint.floor_start")?,
            floor_count: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x02,
                "MapJoint.floor_count",
            )?,
            ceiling_start: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x04,
                "MapJoint.ceiling_start",
            )?,
            ceiling_count: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x06,
                "MapJoint.ceiling_count",
            )?,
            right_wall_start: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x08,
                "MapJoint.right_wall_start",
            )?,
            right_wall_count: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x0A,
                "MapJoint.right_wall_count",
            )?,
            left_wall_start: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x0C,
                "MapJoint.left_wall_start",
            )?,
            left_wall_count: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x0E,
                "MapJoint.left_wall_count",
            )?,
            dynamic_start: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x10,
                "MapJoint.dynamic_start",
            )?,
            dynamic_count: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x12,
                "MapJoint.dynamic_count",
            )?,
            left_bound: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x14,
                "MapJoint.left_bound",
            )?,
            bottom_bound: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x18,
                "MapJoint.bottom_bound",
            )?,
            right_bound: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x1C,
                "MapJoint.right_bound",
            )?,
            top_bound: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x20,
                "MapJoint.top_bound",
            )?,
            vtx_start: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x24,
                "MapJoint.vtx_start",
            )?,
            vtx_count: read_data_i16(
                dat,
                roots.data_block_size,
                offset + 0x26,
                "MapJoint.vtx_count",
            )?,
        });
    }
    Ok(joints)
}

fn derive_surfaces(
    lines: &[MapLine],
    vertices: &[CollVertex],
    coll: &MapCollData,
) -> Vec<DerivedSurface> {
    let mut segments = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let kind = line_kind(index, line, coll);
            if kind != "floor" && kind != "soft_floor" {
                return None;
            }
            let v0 = vertices.get(line.v0_idx as usize)?;
            let v1 = vertices.get(line.v1_idx as usize)?;
            if v0.y != v1.y {
                return None;
            }
            Some((kind == "soft_floor", v0.y, v0.x.min(v1.x), v0.x.max(v1.x)))
        })
        .collect::<Vec<_>>();
    segments.sort_by_key(|(soft, y, left, right)| (*soft, *y, *left, *right));

    let mut groups: Vec<(bool, i32, i32, i32)> = Vec::new();
    for (soft, y, left, right) in segments {
        if let Some(last) = groups.last_mut() {
            if last.0 == soft && last.1 == y && left <= last.3 + 2 {
                last.2 = last.2.min(left);
                last.3 = last.3.max(right);
                continue;
            }
        }
        groups.push((soft, y, left, right));
    }

    let main_index = groups
        .iter()
        .enumerate()
        .filter(|(_, (soft, _, _, _))| !*soft)
        .max_by_key(|(_, (_, _, left, right))| right - left)
        .map(|(index, _)| index);

    let mut soft_index = 0;
    groups
        .into_iter()
        .enumerate()
        .filter_map(|(index, (soft, y, left_x, right_x))| {
            if Some(index) == main_index {
                Some(DerivedSurface {
                    name: "main_floor".to_string(),
                    kind: "solid",
                    left_x,
                    right_x,
                    y,
                })
            } else if soft {
                let name = format!("soft_platform_{soft_index}");
                soft_index += 1;
                Some(DerivedSurface {
                    name,
                    kind: "soft",
                    left_x,
                    right_x,
                    y,
                })
            } else {
                None
            }
        })
        .collect()
}

fn canonicalize_stage_surfaces(stage_id: &str, surfaces: &mut [DerivedSurface]) {
    if stage_id != "battlefield" {
        return;
    }

    for surface in surfaces.iter_mut().filter(|surface| surface.kind == "soft") {
        surface.name = if surface.y > 40_000 {
            "top_platform".to_string()
        } else if surface.right_x <= 0 {
            "left_platform".to_string()
        } else if surface.left_x >= 0 {
            "right_platform".to_string()
        } else {
            surface.name.clone()
        };
    }
}

fn line_to_json(
    index: usize,
    line: &MapLine,
    vertices: &[CollVertex],
    coll: &MapCollData,
) -> Value {
    let v0 = vertices.get(line.v0_idx as usize);
    let v1 = vertices.get(line.v1_idx as usize);
    json!({
        "index": index,
        "kind": line_kind(index, line, coll),
        "v0_idx": line.v0_idx,
        "v1_idx": line.v1_idx,
        "prev_id0": line.prev_id0,
        "next_id0": line.next_id0,
        "prev_id1": line.prev_id1,
        "next_id1": line.next_id1,
        "hi_flags": line.hi_flags,
        "lo_flags": line.lo_flags,
        "passable": (line.lo_flags & COLL_LINE_SOFT_FLOOR) != 0,
        "x0": v0.map(|vertex| vertex.x),
        "y0": v0.map(|vertex| vertex.y),
        "x1": v1.map(|vertex| vertex.x),
        "y1": v1.map(|vertex| vertex.y),
    })
}

fn joint_to_json(index: usize, joint: &MapJoint, scale: f32) -> Value {
    json!({
        "index": index,
        "floor_start": joint.floor_start,
        "floor_count": joint.floor_count,
        "ceiling_start": joint.ceiling_start,
        "ceiling_count": joint.ceiling_count,
        "right_wall_start": joint.right_wall_start,
        "right_wall_count": joint.right_wall_count,
        "left_wall_start": joint.left_wall_start,
        "left_wall_count": joint.left_wall_count,
        "dynamic_start": joint.dynamic_start,
        "dynamic_count": joint.dynamic_count,
        "source_bounds": {
            "left": joint.left_bound,
            "bottom": joint.bottom_bound,
            "right": joint.right_bound,
            "top": joint.top_bound,
        },
        "bounds": {
            "left": source_units_to_milli(joint.left_bound * scale),
            "bottom": source_units_to_milli(joint.bottom_bound * scale),
            "right": source_units_to_milli(joint.right_bound * scale),
            "top": source_units_to_milli(joint.top_bound * scale),
        },
        "vtx_start": joint.vtx_start,
        "vtx_count": joint.vtx_count,
    })
}

fn line_kind(index: usize, line: &MapLine, coll: &MapCollData) -> &'static str {
    if in_range(index, coll.floor_start, coll.floor_count) || (line.hi_flags & COLL_LINE_FLOOR) != 0
    {
        if (line.lo_flags & COLL_LINE_SOFT_FLOOR) != 0 {
            "soft_floor"
        } else {
            "floor"
        }
    } else if in_range(index, coll.ceiling_start, coll.ceiling_count)
        || (line.hi_flags & COLL_LINE_CEILING) != 0
    {
        "ceiling"
    } else if in_range(index, coll.right_wall_start, coll.right_wall_count)
        || (line.hi_flags & COLL_LINE_RIGHT_WALL) != 0
    {
        "right_wall"
    } else if in_range(index, coll.left_wall_start, coll.left_wall_count)
        || (line.hi_flags & COLL_LINE_LEFT_WALL) != 0
    {
        "left_wall"
    } else if in_range(index, coll.dynamic_start, coll.dynamic_count) {
        "dynamic"
    } else {
        "unknown"
    }
}

fn in_range(index: usize, start: i16, count: i16) -> bool {
    if start < 0 || count <= 0 {
        return false;
    }
    let start = start as usize;
    index >= start && index < start + count as usize
}

fn surface_summary(asset: Option<&Value>) -> Value {
    let Some(asset) = asset else {
        return json!({
            "surface_count": 0,
        });
    };
    let soft_count = asset
        .get("soft_platforms")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    json!({
        "main_floor": asset.get("main_floor").cloned().unwrap_or(Value::Null),
        "soft_platforms": asset.get("soft_platforms").cloned().unwrap_or_else(|| json!([])),
        "surface_count": soft_count + usize::from(asset.get("main_floor").is_some()),
    })
}

fn surface_to_value(surface: &DerivedSurface) -> Value {
    json!({
        "name": surface.name,
        "kind": surface.kind,
        "left_x": surface.left_x,
        "right_x": surface.right_x,
        "y": surface.y,
        "friction_multiplier": 1.0,
    })
}

fn decomp_refs(stage: &ResolvedStageInput) -> Vec<&'static str> {
    let mut refs = vec![
        ".research/doldecomp-melee/src/melee/gr/grdatfiles.c",
        ".research/doldecomp-melee/src/melee/gr/ground.c",
        ".research/doldecomp-melee/src/melee/mp/types.h",
        ".research/doldecomp-melee/src/melee/mp/mplib.c",
    ];
    if let Some(stage_ref) = stage.decomp_ref {
        refs.insert(0, stage_ref);
    }
    refs
}

fn read_data_u32(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<u32, String> {
    read_u32(
        dat,
        DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 4, field)?,
        field,
    )
}

fn read_data_i32(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<i32, String> {
    read_i32(
        dat,
        DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 4, field)?,
        field,
    )
}

fn read_data_u16(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<u16, String> {
    read_u16(
        dat,
        DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 2, field)?,
        field,
    )
}

fn read_data_i16(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<i16, String> {
    read_i16(
        dat,
        DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 2, field)?,
        field,
    )
}

fn read_data_f32(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<f32, String> {
    read_f32(
        dat,
        DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 4, field)?,
        field,
    )
}

fn checked_data_offset(
    data_block_size: usize,
    offset: usize,
    size: usize,
    field: &str,
) -> Result<usize, String> {
    if offset
        .checked_add(size)
        .is_some_and(|end| end <= data_block_size)
    {
        Ok(offset)
    } else {
        Err(format!(
            "{field} at data offset 0x{offset:x} is outside the DAT data block"
        ))
    }
}

fn read_u32(bytes: &[u8], offset: usize, field: &str) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{field} at 0x{offset:x} is outside the source bytes"))?;
    Ok(u32::from_be_bytes(
        slice.try_into().expect("slice len is 4"),
    ))
}

fn read_i32(bytes: &[u8], offset: usize, field: &str) -> Result<i32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{field} at 0x{offset:x} is outside the source bytes"))?;
    Ok(i32::from_be_bytes(
        slice.try_into().expect("slice len is 4"),
    ))
}

fn read_u16(bytes: &[u8], offset: usize, field: &str) -> Result<u16, String> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("{field} at 0x{offset:x} is outside the source bytes"))?;
    Ok(u16::from_be_bytes(
        slice.try_into().expect("slice len is 2"),
    ))
}

fn read_i16(bytes: &[u8], offset: usize, field: &str) -> Result<i16, String> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("{field} at 0x{offset:x} is outside the source bytes"))?;
    Ok(i16::from_be_bytes(
        slice.try_into().expect("slice len is 2"),
    ))
}

fn read_f32(bytes: &[u8], offset: usize, field: &str) -> Result<f32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("{field} at 0x{offset:x} is outside the source bytes"))?;
    let value = f32::from_be_bytes(slice.try_into().expect("slice len is 4"));
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{field} at 0x{offset:x} is not finite"))
    }
}

fn dat_string(bytes: &[u8], offset: usize) -> Result<String, String> {
    if offset >= bytes.len() {
        return Err(format!(
            "DAT string at 0x{offset:x} starts outside the file"
        ));
    }
    let end = bytes[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|relative| offset + relative)
        .ok_or_else(|| format!("DAT string at 0x{offset:x} is unterminated"))?;
    std::str::from_utf8(&bytes[offset..end])
        .map(str::to_string)
        .map_err(|error| format!("DAT string at 0x{offset:x} is not UTF-8: {error}"))
}

fn source_units_to_milli(value: f32) -> i32 {
    (value * 1000.0).round() as i32
}

fn build_battlefield_stage_asset() -> Value {
    let stage = StageProfile::battlefield();

    json!({
        "schema_version": SCHEMA_VERSION,
        "stage_id": "battlefield",
        "stage_name": "Battlefield",
        "engine_boundary": "rust_core_authority",
        "source": {
            "kind": BATTLEFIELD_PROVENANCE,
            "profile_name": stage.name,
            "reference_stage": "Battlefield",
            "required_raw_dat": BATTLEFIELD_RAW_DAT,
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
        "main_floor": legacy_surface_to_json(stage.main_floor),
        "soft_platforms": stage.soft_platforms.iter().copied().map(legacy_surface_to_json).collect::<Vec<_>>(),
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

fn legacy_surface_to_json(surface: StageSurface) -> Value {
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
