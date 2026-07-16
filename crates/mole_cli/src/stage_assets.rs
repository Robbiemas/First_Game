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
const COLL_LINE_KIND_MASK: u16 = 0x000F;
const COLL_LINE_EMPTY: u16 = 0x0080;
const COLL_LINE_PLATFORM: u16 = 0x0100;
const COLL_LINE_SOFT_FLOOR: u16 = COLL_LINE_PLATFORM;
const COLL_LINE_LEDGE: u16 = 0x0200;
const COLL_LINE_ENABLED: u32 = 0x0001_0000;
const COLL_LINE_HIDDEN: u32 = 0x0004_0000;
const HSD_JOBJ_INSTANCE: u32 = 1 << 12;
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
            "line_ranges": value.get("line_ranges").cloned().unwrap_or(Value::Null),
            "line_flag_refs": value.get("line_flag_refs").cloned().unwrap_or(Value::Null),
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
    #[serde(default)]
    ledges: Vec<ExtractedStageLedge>,
    #[serde(default)]
    dynamic_collision: ExtractedStageDynamicCollision,
    camera: ExtractedStageCameraInfo,
    source_blast_zones: ExtractedStageFloatBounds,
    map_head_object_tree: Option<ExtractedStageMapHead>,
    #[serde(default)]
    callbacks: ExtractedStageCallbackProfile,
    main_floor: Option<ExtractedStageSurface>,
    soft_platforms: Vec<ExtractedStageSurface>,
    blast_zones: Option<ExtractedStageBlastZones>,
    spawn_points: Option<Vec<ExtractedStageSpawnPoint>>,
    respawn_platforms: Option<Vec<ExtractedStageRespawnPlatform>>,
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
    pos_x: Option<f32>,
    pos_y: Option<f32>,
    x10: Option<f32>,
    x14: Option<f32>,
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
struct ExtractedStageLedge {
    index: u16,
    line_index: u16,
    side: String,
    x_milli: i32,
    y_milli: i32,
}

#[derive(Debug, Default, Deserialize)]
struct ExtractedStageDynamicCollision {
    line_start: i16,
    line_count: i16,
    joint_count_with_dynamic_lines: u16,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageFloatBounds {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageCameraInfo {
    cam_bounds: ExtractedStageFloatBounds,
    cam_x_offset: f32,
    cam_y_offset: f32,
    cam_vertical_tilt: f32,
    cam_pan_degrees: f32,
    x20: f32,
    x24: f32,
    cam_track_ratio: f32,
    cam_fixed_zoom: f32,
    cam_track_smooth: f32,
    cam_zoom_rate: f32,
    cam_max_depth: f32,
    x3c: f32,
    pausecam_zpos_min: f32,
    pausecam_zpos_init: f32,
    pausecam_zpos_max: f32,
    cam_angle_up: f32,
    cam_angle_down: f32,
    cam_angle_left: f32,
    cam_angle_right: f32,
    fixed_cam_pos: ExtractedStageVec3,
    fixed_cam_fov: f32,
    fixed_cam_vert_angle: f32,
    fixed_cam_horz_angle: f32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageMapHead {
    stage_dat_offset: u32,
    unk0_offset: u32,
    unk4: i32,
    entries_offset: u32,
    entry_count: i32,
    splines_offset: u32,
    spline_count: i32,
    unk18_offset: u32,
    unk1c: i32,
    unk20_offset: u32,
    unk24: i32,
    internals_offset: u32,
    internal_count: i32,
    entries: Vec<ExtractedStageMapHeadEntry>,
    joints: Vec<ExtractedStageMapHeadJoint>,
    #[serde(default)]
    point_mappings: Vec<ExtractedStageMapHeadPointMapping>,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageMapHeadPointMapping {
    tree_index: u16,
    stage_info_index: u16,
    joint_index: Option<u16>,
    source_position: ExtractedStageVec3,
    scaled_x: i32,
    scaled_y: i32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageMapHeadEntry {
    index: u16,
    joint_root_offset: u32,
    joint_root_index: Option<u16>,
    camera_desc_offset: u32,
    x14_offset: u32,
    x18_offset: u32,
    fog_desc_offset: u32,
    vector_offset: u32,
    vector_count: i32,
    x28_offset: u32,
    x2c_offset: u32,
    x30: i32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageMapHeadJoint {
    index: u16,
    node_offset: u32,
    class_name_offset: u32,
    flags: u32,
    child_offset: u32,
    next_offset: u32,
    child_index: Option<u16>,
    next_index: Option<u16>,
    dobjdesc_offset: u32,
    rotation: ExtractedStageVec3,
    scale: ExtractedStageVec3,
    position: ExtractedStageVec3,
    mtx_offset: u32,
    robjdesc_offset: u32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageVec3 {
    x: f32,
    y: f32,
    z: f32,
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

#[derive(Debug, Deserialize)]
struct ExtractedStageRespawnPlatform {
    platform_index: u16,
    stage_point_index: u16,
    final_x: i32,
    final_y: i32,
    top_y: i32,
    facing: i8,
    offset_x: i32,
    offset_y: i32,
}

#[derive(Debug, Default, Deserialize)]
struct ExtractedStageCallbackProfile {
    #[serde(default)]
    stage_data_symbol: String,
    #[serde(default)]
    callback_table_symbol: String,
    #[serde(default)]
    object_callbacks: Vec<ExtractedStageObjectCallbacks>,
    #[serde(default)]
    on_init: String,
    #[serde(default)]
    on_demo_init: String,
    #[serde(default)]
    on_load: String,
    #[serde(default)]
    on_start: String,
    #[serde(default)]
    callback4: String,
    #[serde(default)]
    on_touch_line: String,
    #[serde(default)]
    on_check_shadow_render: String,
    #[serde(default)]
    flags2: u32,
    #[serde(default)]
    spawn_table_symbol: String,
    #[serde(default)]
    spawn_table: Vec<ExtractedStageSpawnMapping>,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageObjectCallbacks {
    object_id: u16,
    #[serde(default)]
    callback0: String,
    #[serde(default)]
    callback1: String,
    #[serde(default)]
    callback2: String,
    #[serde(default)]
    callback3: String,
    #[serde(default)]
    flags: u32,
}

#[derive(Debug, Deserialize)]
struct ExtractedStageSpawnMapping {
    index: u16,
    x: i16,
    y: i16,
    z: i16,
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
    let respawn_platforms = stage
        .respawn_platforms
        .as_ref()
        .ok_or_else(|| "Battlefield engine blob requires respawn_platforms".to_string())?;
    let map_head = stage
        .map_head_object_tree
        .as_ref()
        .ok_or_else(|| "Battlefield engine blob requires map_head_object_tree".to_string())?;

    let mut text = String::new();
    text.push_str("// @generated by mole_cli stage extract; do not edit by hand.\n");
    text.push_str("use crate::stage::{\n");
    text.push_str(
        "    MeleeStageProfile, StageBlastZones, StageCallbackProfile, StageCollisionJoint,\n",
    );
    text.push_str(
        "    StageCollisionLine, StageCollisionLineKind, StageCollisionProfile, StageCollisionVertex,\n",
    );
    text.push_str(
        "    StageCameraInfo, StageDynamicCollisionProfile, StageFloatBounds, StageLedge,\n",
    );
    text.push_str("    StageLedgeSide, StageMapHeadEntry,\n");
    text.push_str(
        "    StageMapHeadJoint, StageMapHeadPointMapping, StageMapHeadProfile, StageObjectCallbacks,\n",
    );
    text.push_str(
        "    StageRespawnPlatform, StageSource, StageSpawnMapping, StageSpawnPoint, StageSurface,\n",
    );
    text.push_str("    StageSurfaceKind, StageVec3,\n");
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
    writeln!(text, "    ledges: &BATTLEFIELD_LEDGES,").unwrap();
    writeln!(
        text,
        "    dynamic_collision: StageDynamicCollisionProfile {{ line_start: {}, line_count: {}, joint_count_with_dynamic_lines: {} }},",
        stage.dynamic_collision.line_start,
        stage.dynamic_collision.line_count,
        stage.dynamic_collision.joint_count_with_dynamic_lines
    )
    .unwrap();
    writeln!(
        text,
        "    camera: {},",
        stage_camera_info_literal(&stage.camera)?
    )
    .unwrap();
    writeln!(
        text,
        "    source_blast_zones: {},",
        stage_float_bounds_literal(&stage.source_blast_zones)?
    )
    .unwrap();
    writeln!(text, "    map_head: StageMapHeadProfile {{").unwrap();
    writeln!(
        text,
        "        stage_dat_offset: {},",
        map_head.stage_dat_offset
    )
    .unwrap();
    writeln!(text, "        unk0_offset: {},", map_head.unk0_offset).unwrap();
    writeln!(text, "        unk4: {},", map_head.unk4).unwrap();
    writeln!(text, "        entries_offset: {},", map_head.entries_offset).unwrap();
    writeln!(text, "        entry_count: {},", map_head.entry_count).unwrap();
    writeln!(text, "        splines_offset: {},", map_head.splines_offset).unwrap();
    writeln!(text, "        spline_count: {},", map_head.spline_count).unwrap();
    writeln!(text, "        unk18_offset: {},", map_head.unk18_offset).unwrap();
    writeln!(text, "        unk1c: {},", map_head.unk1c).unwrap();
    writeln!(text, "        unk20_offset: {},", map_head.unk20_offset).unwrap();
    writeln!(text, "        unk24: {},", map_head.unk24).unwrap();
    writeln!(
        text,
        "        internals_offset: {},",
        map_head.internals_offset
    )
    .unwrap();
    writeln!(text, "        internal_count: {},", map_head.internal_count).unwrap();
    writeln!(text, "        entries: &BATTLEFIELD_MAP_HEAD_ENTRIES,").unwrap();
    writeln!(text, "        joints: &BATTLEFIELD_MAP_HEAD_JOINTS,").unwrap();
    writeln!(
        text,
        "        point_mappings: &BATTLEFIELD_MAP_HEAD_POINT_MAPPINGS,"
    )
    .unwrap();
    writeln!(text, "    }},").unwrap();
    writeln!(text, "    callbacks: StageCallbackProfile {{").unwrap();
    writeln!(
        text,
        "        stage_data_symbol: {},",
        rust_str(&stage.callbacks.stage_data_symbol)
    )
    .unwrap();
    writeln!(
        text,
        "        callback_table_symbol: {},",
        rust_str(&stage.callbacks.callback_table_symbol)
    )
    .unwrap();
    writeln!(
        text,
        "        object_callbacks: &BATTLEFIELD_OBJECT_CALLBACKS,"
    )
    .unwrap();
    writeln!(
        text,
        "        on_init: {},",
        rust_str(&stage.callbacks.on_init)
    )
    .unwrap();
    writeln!(
        text,
        "        on_demo_init: {},",
        rust_str(&stage.callbacks.on_demo_init)
    )
    .unwrap();
    writeln!(
        text,
        "        on_load: {},",
        rust_str(&stage.callbacks.on_load)
    )
    .unwrap();
    writeln!(
        text,
        "        on_start: {},",
        rust_str(&stage.callbacks.on_start)
    )
    .unwrap();
    writeln!(
        text,
        "        callback4: {},",
        rust_str(&stage.callbacks.callback4)
    )
    .unwrap();
    writeln!(
        text,
        "        on_touch_line: {},",
        rust_str(&stage.callbacks.on_touch_line)
    )
    .unwrap();
    writeln!(
        text,
        "        on_check_shadow_render: {},",
        rust_str(&stage.callbacks.on_check_shadow_render)
    )
    .unwrap();
    writeln!(text, "        flags2: {},", stage.callbacks.flags2).unwrap();
    writeln!(
        text,
        "        spawn_table_symbol: {},",
        rust_str(&stage.callbacks.spawn_table_symbol)
    )
    .unwrap();
    writeln!(text, "        spawn_table: &BATTLEFIELD_SPAWN_MAPPINGS,").unwrap();
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
    writeln!(text, "    respawn_platforms: [").unwrap();
    for index in 0..4 {
        if let Some(platform) = respawn_platforms.get(index) {
            writeln!(text, "        StageRespawnPlatform {{").unwrap();
            writeln!(
                text,
                "            platform_index: {},",
                platform.platform_index
            )
            .unwrap();
            writeln!(
                text,
                "            stage_point_index: {},",
                platform.stage_point_index
            )
            .unwrap();
            writeln!(text, "            final_x: {},", platform.final_x).unwrap();
            writeln!(text, "            final_y: {},", platform.final_y).unwrap();
            writeln!(text, "            top_y: {},", platform.top_y).unwrap();
            writeln!(text, "            facing: {},", platform.facing).unwrap();
            writeln!(text, "            offset_x: {},", platform.offset_x).unwrap();
            writeln!(text, "            offset_y: {},", platform.offset_y).unwrap();
            writeln!(text, "        }},").unwrap();
        } else {
            writeln!(
                text,
                "        StageRespawnPlatform {{ platform_index: {}, stage_point_index: {}, final_x: 0, final_y: 0, top_y: 0, facing: 1, offset_x: 0, offset_y: 0 }},",
                index, index + 4
            )
            .unwrap();
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
        let pos_x = vertex
            .pos_x
            .unwrap_or(vertex.source_x * stage.collision.scale);
        let pos_y = vertex
            .pos_y
            .unwrap_or(vertex.source_y * stage.collision.scale);
        let x10 = vertex.x10.unwrap_or(pos_x);
        let x14 = vertex.x14.unwrap_or(pos_y);
        writeln!(
            text,
            "    StageCollisionVertex {{ index: {}, source_x: {}, source_y: {}, pos_x: {}, pos_y: {}, x10: {}, x14: {} }},",
            vertex.index,
            rust_f32(vertex.source_x)?,
            rust_f32(vertex.source_y)?,
            rust_f32(pos_x)?,
            rust_f32(pos_y)?,
            rust_f32(x10)?,
            rust_f32(x14)?
        )
        .unwrap();
    }
    writeln!(text, "];\n").unwrap();

    writeln!(
        text,
        "const BATTLEFIELD_LEDGES: [StageLedge; {}] = [",
        stage.ledges.len()
    )
    .unwrap();
    for ledge in &stage.ledges {
        writeln!(
            text,
            "    StageLedge {{ index: {}, line_index: {}, side: {}, x_milli: {}, y_milli: {} }},",
            ledge.index,
            ledge.line_index,
            stage_ledge_side_literal(&ledge.side)?,
            ledge.x_milli,
            ledge.y_milli
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

    writeln!(
        text,
        "\nconst BATTLEFIELD_MAP_HEAD_POINT_MAPPINGS: [StageMapHeadPointMapping; {}] = [",
        map_head.point_mappings.len()
    )
    .unwrap();
    for mapping in &map_head.point_mappings {
        writeln!(text, "    StageMapHeadPointMapping {{").unwrap();
        writeln!(text, "        tree_index: {},", mapping.tree_index).unwrap();
        writeln!(
            text,
            "        stage_info_index: {},",
            mapping.stage_info_index
        )
        .unwrap();
        writeln!(
            text,
            "        joint_index: {},",
            rust_option_u16(mapping.joint_index)
        )
        .unwrap();
        writeln!(
            text,
            "        source_position: {},",
            stage_vec3_literal(&mapping.source_position)?
        )
        .unwrap();
        writeln!(text, "        scaled_x: {},", mapping.scaled_x).unwrap();
        writeln!(text, "        scaled_y: {},", mapping.scaled_y).unwrap();
        writeln!(text, "    }},").unwrap();
    }
    writeln!(text, "];").unwrap();

    writeln!(
        text,
        "\nconst BATTLEFIELD_MAP_HEAD_ENTRIES: [StageMapHeadEntry; {}] = [",
        map_head.entries.len()
    )
    .unwrap();
    for entry in &map_head.entries {
        writeln!(text, "    StageMapHeadEntry {{").unwrap();
        writeln!(text, "        index: {},", entry.index).unwrap();
        writeln!(
            text,
            "        joint_root_offset: {},",
            entry.joint_root_offset
        )
        .unwrap();
        writeln!(
            text,
            "        joint_root_index: {},",
            rust_option_u16(entry.joint_root_index)
        )
        .unwrap();
        writeln!(
            text,
            "        camera_desc_offset: {},",
            entry.camera_desc_offset
        )
        .unwrap();
        writeln!(text, "        x14_offset: {},", entry.x14_offset).unwrap();
        writeln!(text, "        x18_offset: {},", entry.x18_offset).unwrap();
        writeln!(text, "        fog_desc_offset: {},", entry.fog_desc_offset).unwrap();
        writeln!(text, "        vector_offset: {},", entry.vector_offset).unwrap();
        writeln!(text, "        vector_count: {},", entry.vector_count).unwrap();
        writeln!(text, "        x28_offset: {},", entry.x28_offset).unwrap();
        writeln!(text, "        x2c_offset: {},", entry.x2c_offset).unwrap();
        writeln!(text, "        x30: {},", entry.x30).unwrap();
        writeln!(text, "    }},").unwrap();
    }
    writeln!(text, "];").unwrap();

    writeln!(
        text,
        "\nconst BATTLEFIELD_MAP_HEAD_JOINTS: [StageMapHeadJoint; {}] = [",
        map_head.joints.len()
    )
    .unwrap();
    for joint in &map_head.joints {
        writeln!(text, "    StageMapHeadJoint {{").unwrap();
        writeln!(text, "        index: {},", joint.index).unwrap();
        writeln!(text, "        node_offset: {},", joint.node_offset).unwrap();
        writeln!(
            text,
            "        class_name_offset: {},",
            joint.class_name_offset
        )
        .unwrap();
        writeln!(text, "        flags: {},", joint.flags).unwrap();
        writeln!(text, "        child_offset: {},", joint.child_offset).unwrap();
        writeln!(text, "        next_offset: {},", joint.next_offset).unwrap();
        writeln!(
            text,
            "        child_index: {},",
            rust_option_u16(joint.child_index)
        )
        .unwrap();
        writeln!(
            text,
            "        next_index: {},",
            rust_option_u16(joint.next_index)
        )
        .unwrap();
        writeln!(text, "        dobjdesc_offset: {},", joint.dobjdesc_offset).unwrap();
        writeln!(
            text,
            "        rotation: {},",
            stage_vec3_literal(&joint.rotation)?
        )
        .unwrap();
        writeln!(
            text,
            "        scale: {},",
            stage_vec3_literal(&joint.scale)?
        )
        .unwrap();
        writeln!(
            text,
            "        position: {},",
            stage_vec3_literal(&joint.position)?
        )
        .unwrap();
        writeln!(text, "        mtx_offset: {},", joint.mtx_offset).unwrap();
        writeln!(text, "        robjdesc_offset: {},", joint.robjdesc_offset).unwrap();
        writeln!(text, "    }},").unwrap();
    }
    writeln!(text, "];").unwrap();

    writeln!(
        text,
        "\nconst BATTLEFIELD_OBJECT_CALLBACKS: [StageObjectCallbacks; {}] = [",
        stage.callbacks.object_callbacks.len()
    )
    .unwrap();
    for callback in &stage.callbacks.object_callbacks {
        writeln!(
            text,
            "    StageObjectCallbacks {{ object_id: {}, callback0: {}, callback1: {}, callback2: {}, callback3: {}, flags: {} }},",
            callback.object_id,
            rust_str(&callback.callback0),
            rust_str(&callback.callback1),
            rust_str(&callback.callback2),
            rust_str(&callback.callback3),
            callback.flags
        )
        .unwrap();
    }
    writeln!(text, "];").unwrap();

    writeln!(
        text,
        "\nconst BATTLEFIELD_SPAWN_MAPPINGS: [StageSpawnMapping; {}] = [",
        stage.callbacks.spawn_table.len()
    )
    .unwrap();
    for spawn in &stage.callbacks.spawn_table {
        writeln!(
            text,
            "    StageSpawnMapping {{ index: {}, x: {}, y: {}, z: {} }},",
            spawn.index, spawn.x, spawn.y, spawn.z
        )
        .unwrap();
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

fn rust_option_u16(value: Option<u16>) -> String {
    value
        .map(|value| format!("Some({value})"))
        .unwrap_or_else(|| "None".to_string())
}

fn stage_vec3_literal(value: &ExtractedStageVec3) -> Result<String, String> {
    Ok(format!(
        "StageVec3 {{ x: {}, y: {}, z: {} }}",
        rust_f32(value.x)?,
        rust_f32(value.y)?,
        rust_f32(value.z)?
    ))
}

fn stage_float_bounds_literal(value: &ExtractedStageFloatBounds) -> Result<String, String> {
    Ok(format!(
        "StageFloatBounds {{ left: {}, right: {}, top: {}, bottom: {} }}",
        rust_f32(value.left)?,
        rust_f32(value.right)?,
        rust_f32(value.top)?,
        rust_f32(value.bottom)?
    ))
}

fn stage_camera_info_literal(value: &ExtractedStageCameraInfo) -> Result<String, String> {
    Ok(format!(
        concat!(
            "StageCameraInfo {{ cam_bounds: {}, cam_x_offset: {}, cam_y_offset: {}, ",
            "cam_vertical_tilt: {}, cam_pan_degrees: {}, x20: {}, x24: {}, ",
            "cam_track_ratio: {}, cam_fixed_zoom: {}, cam_track_smooth: {}, ",
            "cam_zoom_rate: {}, cam_max_depth: {}, x3c: {}, pausecam_zpos_min: {}, ",
            "pausecam_zpos_init: {}, pausecam_zpos_max: {}, cam_angle_up: {}, ",
            "cam_angle_down: {}, cam_angle_left: {}, cam_angle_right: {}, ",
            "fixed_cam_pos: {}, fixed_cam_fov: {}, fixed_cam_vert_angle: {}, ",
            "fixed_cam_horz_angle: {} }}"
        ),
        stage_float_bounds_literal(&value.cam_bounds)?,
        rust_f32(value.cam_x_offset)?,
        rust_f32(value.cam_y_offset)?,
        rust_f32(value.cam_vertical_tilt)?,
        rust_f32(value.cam_pan_degrees)?,
        rust_f32(value.x20)?,
        rust_f32(value.x24)?,
        rust_f32(value.cam_track_ratio)?,
        rust_f32(value.cam_fixed_zoom)?,
        rust_f32(value.cam_track_smooth)?,
        rust_f32(value.cam_zoom_rate)?,
        rust_f32(value.cam_max_depth)?,
        rust_f32(value.x3c)?,
        rust_f32(value.pausecam_zpos_min)?,
        rust_f32(value.pausecam_zpos_init)?,
        rust_f32(value.pausecam_zpos_max)?,
        rust_f32(value.cam_angle_up)?,
        rust_f32(value.cam_angle_down)?,
        rust_f32(value.cam_angle_left)?,
        rust_f32(value.cam_angle_right)?,
        stage_vec3_literal(&value.fixed_cam_pos)?,
        rust_f32(value.fixed_cam_fov)?,
        rust_f32(value.fixed_cam_vert_angle)?,
        rust_f32(value.fixed_cam_horz_angle)?
    ))
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

fn stage_ledge_side_literal(side: &str) -> Result<&'static str, String> {
    match side {
        "left" => Ok("StageLedgeSide::Left"),
        "right" => Ok("StageLedgeSide::Right"),
        other => Err(format!("unknown stage ledge side for engine blob: {other}")),
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
    pos_x: f32,
    pos_y: f32,
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

#[derive(Debug, Clone)]
struct DerivedLedge {
    line_index: usize,
    side: &'static str,
    x_milli: i32,
    y_milli: i32,
}

#[derive(Debug, Clone)]
struct StageCallbackMetadata {
    stage_data_symbol: String,
    callback_table_symbol: String,
    object_callbacks: Vec<StageObjectCallbackMetadata>,
    on_init: String,
    on_demo_init: String,
    on_load: String,
    on_start: String,
    callback4: String,
    on_touch_line: String,
    on_check_shadow_render: String,
    flags2: u32,
    spawn_table_symbol: String,
    spawn_table: Vec<StageSpawnMappingMetadata>,
}

#[derive(Debug, Clone)]
struct StageObjectCallbackMetadata {
    object_id: u16,
    callback0: String,
    callback1: String,
    callback2: String,
    callback3: String,
    flags: u32,
}

#[derive(Debug, Clone)]
struct StageSpawnMappingMetadata {
    index: u16,
    x: i16,
    y: i16,
    z: i16,
}

#[derive(Debug, Clone)]
struct StageCameraMetadata {
    cam_bounds: StageFloatBoundsMetadata,
    cam_x_offset: f32,
    cam_y_offset: f32,
    cam_vertical_tilt: f32,
    cam_pan_degrees: f32,
    x20: f32,
    x24: f32,
    cam_track_ratio: f32,
    cam_fixed_zoom: f32,
    cam_track_smooth: f32,
    cam_zoom_rate: f32,
    cam_max_depth: f32,
    x3c: f32,
    pausecam_zpos_min: f32,
    pausecam_zpos_init: f32,
    pausecam_zpos_max: f32,
    cam_angle_up: f32,
    cam_angle_down: f32,
    cam_angle_left: f32,
    cam_angle_right: f32,
    fixed_cam_pos: StageVec3Metadata,
    fixed_cam_fov: f32,
    fixed_cam_vert_angle: f32,
    fixed_cam_horz_angle: f32,
}

#[derive(Debug, Clone, Copy)]
struct StageFloatBoundsMetadata {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

#[derive(Debug, Clone, Copy)]
struct StageVec3Metadata {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone)]
struct StageBoundsMetadata {
    camera: StageFloatBoundsMetadata,
    blast: StageFloatBoundsMetadata,
    cam_x_offset: f32,
    cam_y_offset: f32,
    source: &'static str,
}

#[derive(Debug, Clone)]
struct MapHeadScene {
    stage_dat_offset: u32,
    unk0_offset: u32,
    unk4: i32,
    entries_offset: u32,
    entry_count: i32,
    splines_offset: u32,
    spline_count: i32,
    unk18_offset: u32,
    unk1c: i32,
    unk20_offset: u32,
    unk24: i32,
    internals_offset: u32,
    internal_count: i32,
    entries: Vec<MapHeadModelGroup>,
    joints: Vec<MapHeadNode>,
    point_mappings: Vec<MapHeadPointMapping>,
}

#[derive(Debug, Clone)]
struct MapHeadPointMapping {
    tree_index: u16,
    stage_info_index: u16,
    joint_index: Option<usize>,
    source_position: StageVec3Metadata,
}

#[derive(Debug, Clone)]
struct MapHeadModelGroup {
    joint_root_offset: u32,
    joint_root_index: Option<usize>,
    camera_desc_offset: u32,
    x14_offset: u32,
    x18_offset: u32,
    fog_desc_offset: u32,
    vector_offset: u32,
    vector_count: i32,
    x28_offset: u32,
    x2c_offset: u32,
    x30: i32,
}

#[derive(Debug, Clone)]
struct MapHeadNode {
    index: usize,
    node_offset: u32,
    class_name_offset: u32,
    flags: u32,
    child_offset: u32,
    next_offset: u32,
    child_index: Option<usize>,
    next_index: Option<usize>,
    dobjdesc_offset: u32,
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    scale_x: f32,
    scale_y: f32,
    scale_z: f32,
    position_x: f32,
    position_y: f32,
    position_z: f32,
    mtx_offset: u32,
    robjdesc_offset: u32,
}

#[derive(Debug, Clone)]
struct StageRespawnPlatformMetadata {
    platform_index: u16,
    stage_point_index: u16,
    final_x: i32,
    final_y: i32,
    top_y: i32,
    facing: i8,
    offset_x: i32,
    offset_y: i32,
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
    let ground_param_offset = roots.roots.get("grGroundParam").copied();
    let scale = ground_param_offset
        .map(|offset| {
            read_data_f32(
                dat,
                roots.data_block_size,
                offset as usize,
                "grGroundParam.x0",
            )
        })
        .transpose()?
        .unwrap_or(1.0);
    let coll = parse_map_coll_data(dat, &roots, coll_offset)?;
    let vertices = parse_vertices(dat, &roots, &coll, scale)?;
    let mut lines = parse_lines(dat, &roots, &coll)?;
    apply_mp_lib_load_empty_line_prune(&mut lines, &vertices);
    let joints = parse_joints(dat, &roots, &coll, scale)?;
    let mut surfaces = derive_surfaces(&lines, &vertices, &coll);
    canonicalize_stage_surfaces(&stage.stage_id, &mut surfaces);
    let ledges = derive_ledges(&lines, &vertices, &coll);
    let dynamic_joint_count = joints
        .iter()
        .filter(|joint| joint.dynamic_count > 0)
        .count() as u16;
    let map_head_object_tree = parse_map_head_object_tree(dat, &roots)?;
    let stage_bounds = map_head_object_tree
        .as_ref()
        .and_then(|scene| derive_stage_bounds_from_map_head(scene, scale));
    let camera =
        parse_stage_camera_metadata(dat, &roots, ground_param_offset, stage_bounds.as_ref())?;
    let respawn_platforms = map_head_object_tree
        .as_ref()
        .and_then(|scene| derive_respawn_platforms_from_map_head(scene, scale, &camera));
    let source_blast_zones =
        stage_bounds
            .as_ref()
            .map(|bounds| bounds.blast)
            .unwrap_or(StageFloatBoundsMetadata {
                left: -99999.0,
                right: 99999.0,
                top: 99999.0,
                bottom: -99999.0,
            });
    let callbacks = decomp_stage_metadata(stage).unwrap_or_else(empty_stage_callback_metadata);
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
    let mut pending_stage_layers = vec!["itemdata"];
    if map_head_object_tree.is_none() {
        pending_stage_layers.insert(0, "map_head_object_tree");
    }
    if respawn_platforms.is_none() {
        pending_stage_layers.push("stage_info_x280_respawn_platforms");
    }
    if stage_bounds.is_none() {
        pending_stage_layers.push("camera_bounds");
        pending_stage_layers.push("blast_zones");
    }

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
                ".research/doldecomp-melee/src/melee/mp/mplib.c::mpPruneEmptyLines",
                ".research/doldecomp-melee/src/melee/mp/mplib.c::mpLibLoad"
            ],
            "notes": "Collision vertices preserve raw DAT source coordinates and scaled engine milli-units matching mpLibLoad/Ground_801C0498. Collision lines are baked after mpPruneEmptyLines topology/LINE_FLAG_EMPTY mutation."
        },
        "collision": {
            "source_root": "coll_data",
            "coll_data_offset": coll_offset,
            "scale": scale,
            "line_flag_refs": line_flag_refs_to_json(),
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
                "pos_x": vertex.pos_x,
                "pos_y": vertex.pos_y,
                "x10": vertex.pos_x,
                "x14": vertex.pos_y,
                "x": vertex.x,
                "y": vertex.y,
            })).collect::<Vec<_>>(),
            "lines": lines.iter().enumerate().map(|(index, line)| line_to_json(index, line, &vertices, &coll)).collect::<Vec<_>>(),
            "joints": joints.iter().enumerate().map(|(index, joint)| joint_to_json(index, joint, scale)).collect::<Vec<_>>(),
        },
        "ledges": ledges.iter().map(|ledge| json!({
            "index": ledge.line_index,
            "line_index": ledge.line_index,
            "side": ledge.side,
            "x_milli": ledge.x_milli,
            "y_milli": ledge.y_milli,
            "source": {
                "root": "coll_data",
                "line_flag": "LINE_FLAG_LEDGE",
                "lo_flag_mask": COLL_LINE_LEDGE,
                "rule": "mpLib_80051BA8_Floor returns the LINE_FLAG_LEDGE floor line id; side-specific endpoint comes from mpColl_80044164/800443C4"
            }
        })).collect::<Vec<_>>(),
        "dynamic_collision": {
            "source_root": "coll_data",
            "line_start": coll.dynamic_start,
            "line_count": coll.dynamic_count,
            "joint_count_with_dynamic_lines": dynamic_joint_count,
            "requires_stage_callbacks": coll.dynamic_count > 0,
        },
        "map_head_object_tree": map_head_object_tree
            .as_ref()
            .map(|groups| map_head_object_tree_to_json(groups, scale)),
        "camera": stage_camera_metadata_to_json(&camera),
        "source_blast_zones": stage_float_bounds_to_json(&source_blast_zones),
        "callbacks": stage_callback_metadata_to_json(&callbacks),
        "main_floor": main_floor.as_ref().map(surface_to_value),
        "soft_platforms": soft_platforms.iter().map(surface_to_value).collect::<Vec<_>>(),
        "blast_zones": {
            "left_x": source_units_to_milli(source_blast_zones.left),
            "right_x": source_units_to_milli(source_blast_zones.right),
            "top_y": source_units_to_milli(source_blast_zones.top),
            "bottom_y": source_units_to_milli(source_blast_zones.bottom),
            "source": stage_bounds.as_ref().map(|bounds| bounds.source).unwrap_or("stage_info_default"),
        },
        "spawn_points": if stage.stage_id == "battlefield" {
            Some(build_battlefield_stage_asset()["spawn_points"].clone())
        } else {
            None
        },
        "respawn_platforms": respawn_platforms
            .as_ref()
            .map(|platforms| platforms.iter().map(respawn_platform_to_json).collect::<Vec<_>>()),
        "pending_stage_layers": pending_stage_layers,
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
        let pos_x = source_x * scale;
        let pos_y = source_y * scale;
        vertices.push(CollVertex {
            source_x,
            source_y,
            pos_x,
            pos_y,
            x: source_units_to_milli(pos_x),
            y: source_units_to_milli(pos_y),
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

fn apply_mp_lib_load_empty_line_prune(lines: &mut [MapLine], vertices: &[CollVertex]) {
    for index in 0..lines.len() {
        let Some(v0) = vertices.get(lines[index].v0_idx as usize) else {
            continue;
        };
        let Some(v1) = vertices.get(lines[index].v1_idx as usize) else {
            continue;
        };
        if v0.source_x != v1.source_x || v0.source_y != v1.source_y {
            continue;
        }

        let empty_line_id = index as i16;
        let prev_id0 = lines[index].prev_id0;
        let next_id0 = lines[index].next_id0;
        for line in lines.iter_mut() {
            if line.prev_id0 == empty_line_id {
                line.prev_id0 = prev_id0;
            }
            if line.next_id0 == empty_line_id {
                line.next_id0 = next_id0;
            }
            if line.prev_id1 == empty_line_id {
                line.prev_id1 = prev_id0;
            }
            if line.next_id1 == empty_line_id {
                line.next_id1 = next_id0;
            }
        }

        let line = &mut lines[index];
        line.hi_flags |= COLL_LINE_EMPTY;
        line.prev_id0 = -1;
        line.next_id0 = -1;
        line.prev_id1 = -1;
        line.next_id1 = -1;
    }
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

fn parse_map_head_object_tree(
    dat: &[u8],
    roots: &DatRoots,
) -> Result<Option<MapHeadScene>, String> {
    let Some(map_head_offset) = roots.roots.get("map_head").copied() else {
        return Ok(None);
    };
    let stage_dat_offset = map_head_offset as usize;
    let unk0_offset = read_data_u32(
        dat,
        roots.data_block_size,
        stage_dat_offset,
        "map_head.unk0",
    )?;
    let unk4 = read_data_i32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x04,
        "map_head.unk4",
    )?;
    let entries_offset = read_data_u32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x08,
        "map_head.entries",
    )?;
    let entry_count = read_data_i32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x0C,
        "map_head.entry_count",
    )?;
    let splines_offset = read_data_u32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x10,
        "map_head.splines",
    )?;
    let spline_count = read_data_i32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x14,
        "map_head.spline_count",
    )?;
    let unk18_offset = read_data_u32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x18,
        "map_head.unk18",
    )?;
    let unk1c = read_data_i32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x1C,
        "map_head.unk1c",
    )?;
    let unk20_offset = read_data_u32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x20,
        "map_head.unk20",
    )?;
    let unk24 = read_data_i32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x24,
        "map_head.unk24",
    )?;
    let internals_offset = read_data_u32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x28,
        "map_head.internals",
    )?;
    let internal_count = read_data_i32(
        dat,
        roots.data_block_size,
        stage_dat_offset + 0x2C,
        "map_head.internal_count",
    )?;

    let mut entries = Vec::new();
    let mut joints = Vec::new();
    let mut joint_indices = BTreeMap::new();
    if entry_count > 0 && is_data_offset_in_range(roots.data_block_size, entries_offset, 0x34) {
        for index in 0..entry_count as usize {
            let entry_offset = entries_offset as usize + index * 0x34;
            if entry_offset + 0x34 > roots.data_block_size {
                break;
            }
            let joint_root_offset = read_data_u32(
                dat,
                roots.data_block_size,
                entry_offset,
                "map_head.entry.joint_root",
            )?;
            let joint_root_index = parse_map_head_joint_index(
                dat,
                roots,
                joint_root_offset,
                &mut joint_indices,
                &mut joints,
            )
            .ok()
            .flatten();
            entries.push(MapHeadModelGroup {
                joint_root_offset,
                joint_root_index,
                camera_desc_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x10,
                    "map_head.entry.camera_desc",
                )?,
                x14_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x14,
                    "map_head.entry.x14",
                )?,
                x18_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x18,
                    "map_head.entry.x18",
                )?,
                fog_desc_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x1C,
                    "map_head.entry.fog_desc",
                )?,
                vector_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x20,
                    "map_head.entry.vector",
                )?,
                vector_count: read_data_i32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x24,
                    "map_head.entry.vector_count",
                )?,
                x28_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x28,
                    "map_head.entry.x28",
                )?,
                x2c_offset: read_data_u32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x2C,
                    "map_head.entry.x2c",
                )?,
                x30: read_data_i32(
                    dat,
                    roots.data_block_size,
                    entry_offset + 0x30,
                    "map_head.entry.x30",
                )?,
            });
        }
    }
    let mut scene = MapHeadScene {
        stage_dat_offset: map_head_offset,
        unk0_offset,
        unk4,
        entries_offset,
        entry_count,
        splines_offset,
        spline_count,
        unk18_offset,
        unk1c,
        unk20_offset,
        unk24,
        internals_offset,
        internal_count,
        entries,
        joints,
        point_mappings: Vec::new(),
    };
    scene.point_mappings =
        parse_map_head_point_mappings(dat, roots, unk0_offset, unk4, &joint_indices, &scene)?;

    Ok(Some(scene))
}

fn parse_map_head_point_mappings(
    dat: &[u8],
    roots: &DatRoots,
    table_offset: u32,
    table_count: i32,
    joint_indices: &BTreeMap<u32, usize>,
    scene: &MapHeadScene,
) -> Result<Vec<MapHeadPointMapping>, String> {
    if table_count <= 0 || !is_data_offset_in_range(roots.data_block_size, table_offset, 0x0C) {
        return Ok(Vec::new());
    }

    let world_positions = map_head_world_positions(scene);
    let mut mappings = Vec::new();
    for record_index in 0..table_count as usize {
        let record_offset = table_offset as usize + record_index * 0x0C;
        if record_offset + 0x0C > roots.data_block_size {
            break;
        }
        let joint_offset = read_data_u32(
            dat,
            roots.data_block_size,
            record_offset,
            "map_head.unk0.joint",
        )?;
        let pair_offset = read_data_u32(
            dat,
            roots.data_block_size,
            record_offset + 4,
            "map_head.unk0.pairs",
        )?;
        let pair_count = read_data_i32(
            dat,
            roots.data_block_size,
            record_offset + 8,
            "map_head.unk0.pair_count",
        )?;
        if pair_count <= 0 || !is_data_offset_in_range(roots.data_block_size, pair_offset, 4) {
            continue;
        }
        let Some(root_index) = joint_indices.get(&joint_offset).copied() else {
            continue;
        };
        let tree_walk = map_head_ground_801c34ac_tree_walk(scene, root_index);
        for pair_index in 0..pair_count as usize {
            let pair_data_offset = pair_offset as usize + pair_index * 4;
            if pair_data_offset + 4 > roots.data_block_size {
                break;
            }
            let tree_index = read_data_i16(
                dat,
                roots.data_block_size,
                pair_data_offset,
                "map_head.unk0.pair.tree_index",
            )?;
            let stage_info_index = read_data_i16(
                dat,
                roots.data_block_size,
                pair_data_offset + 2,
                "map_head.unk0.pair.stage_info_index",
            )?;
            if tree_index < 0 || stage_info_index < 0 {
                continue;
            }
            let joint_index = tree_walk.get(tree_index as usize).copied();
            let source_position = joint_index
                .and_then(|index| world_positions.get(index).copied())
                .unwrap_or(StageVec3Metadata {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            mappings.push(MapHeadPointMapping {
                tree_index: tree_index as u16,
                stage_info_index: stage_info_index as u16,
                joint_index,
                source_position,
            });
        }
    }
    Ok(mappings)
}

fn map_head_ground_801c34ac_tree_walk(scene: &MapHeadScene, root_index: usize) -> Vec<usize> {
    let parents = map_head_parent_indices(scene);
    let mut result = Vec::new();
    let mut current = Some(root_index);
    while let Some(index) = current {
        if scene.joints.get(index).is_none() || result.contains(&index) {
            break;
        }
        result.push(index);
        current = map_head_ground_801c34ac_next(scene, &parents, index);
    }
    result
}

fn map_head_parent_indices(scene: &MapHeadScene) -> Vec<Option<usize>> {
    let mut parents = vec![None; scene.joints.len()];
    let mut visited = vec![false; scene.joints.len()];
    for entry in &scene.entries {
        if let Some(root) = entry.joint_root_index {
            map_head_parent_indices_walk(scene, root, None, &mut parents, &mut visited);
        }
    }
    parents
}

fn map_head_parent_indices_walk(
    scene: &MapHeadScene,
    index: usize,
    parent: Option<usize>,
    parents: &mut [Option<usize>],
    visited: &mut [bool],
) {
    if visited.get(index).copied().unwrap_or(true) {
        return;
    }
    visited[index] = true;
    if let Some(slot) = parents.get_mut(index) {
        *slot = parent;
    }
    let Some(node) = scene.joints.get(index) else {
        return;
    };
    if let Some(child) = node.child_index {
        map_head_parent_indices_walk(scene, child, Some(index), parents, visited);
    }
    if let Some(next) = node.next_index {
        map_head_parent_indices_walk(scene, next, parent, parents, visited);
    }
}

fn map_head_ground_801c34ac_next(
    scene: &MapHeadScene,
    parents: &[Option<usize>],
    index: usize,
) -> Option<usize> {
    let node = scene.joints.get(index)?;
    if node.flags & HSD_JOBJ_INSTANCE == 0 {
        if let Some(child) = node.child_index {
            return Some(child);
        }
    }
    if let Some(next) = node.next_index {
        return Some(next);
    }

    let mut current = index;
    loop {
        let parent = parents.get(current).copied().flatten()?;
        if let Some(next) = scene.joints.get(parent).and_then(|node| node.next_index) {
            return Some(next);
        }
        current = parent;
    }
}

fn parse_map_head_joint_index(
    dat: &[u8],
    roots: &DatRoots,
    node_offset: u32,
    joint_indices: &mut BTreeMap<u32, usize>,
    joints: &mut Vec<MapHeadNode>,
) -> Result<Option<usize>, String> {
    if node_offset == 0 || !is_data_offset_in_range(roots.data_block_size, node_offset, 0x40) {
        return Ok(None);
    }
    if let Some(index) = joint_indices.get(&node_offset).copied() {
        return Ok(Some(index));
    }
    let offset = node_offset as usize;
    let index = joints.len();
    joint_indices.insert(node_offset, index);
    let class_name_offset = read_data_u32(
        dat,
        roots.data_block_size,
        offset,
        "map_head.node.class_name",
    )?;
    let flags = read_data_u32(
        dat,
        roots.data_block_size,
        offset + 4,
        "map_head.node.flags",
    )?;
    let child_offset = read_data_u32(
        dat,
        roots.data_block_size,
        offset + 8,
        "map_head.node.child_offset",
    )?;
    let next_offset = read_data_u32(
        dat,
        roots.data_block_size,
        offset + 0x0C,
        "map_head.node.next_offset",
    )?;
    let data_offset = read_data_u32(
        dat,
        roots.data_block_size,
        offset + 0x10,
        "map_head.node.dobjdesc_offset",
    )?;
    let mtx_offset = read_data_u32(
        dat,
        roots.data_block_size,
        offset + 0x38,
        "map_head.node.mtx_offset",
    )?;
    let robjdesc_offset = read_data_u32(
        dat,
        roots.data_block_size,
        offset + 0x3C,
        "map_head.node.robjdesc_offset",
    )?;
    joints.push(MapHeadNode {
        index,
        node_offset,
        class_name_offset,
        flags,
        child_offset,
        next_offset,
        child_index: None,
        next_index: None,
        dobjdesc_offset: data_offset,
        rotation_x: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x14,
            "map_head.node.rotation_x",
        )?,
        rotation_y: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x18,
            "map_head.node.rotation_y",
        )?,
        rotation_z: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x1C,
            "map_head.node.rotation_z",
        )?,
        scale_x: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x20,
            "map_head.node.scale_x",
        )?,
        scale_y: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x24,
            "map_head.node.scale_y",
        )?,
        scale_z: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x28,
            "map_head.node.scale_z",
        )?,
        position_x: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x2C,
            "map_head.node.position_x",
        )?,
        position_y: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x30,
            "map_head.node.position_y",
        )?,
        position_z: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x34,
            "map_head.node.position_z",
        )?,
        mtx_offset,
        robjdesc_offset,
    });
    let child_index = parse_map_head_joint_index(dat, roots, child_offset, joint_indices, joints)?;
    let next_index = parse_map_head_joint_index(dat, roots, next_offset, joint_indices, joints)?;
    if let Some(joint) = joints.get_mut(index) {
        joint.child_index = child_index;
        joint.next_index = next_index;
    }
    Ok(Some(index))
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

fn derive_ledges(
    lines: &[MapLine],
    vertices: &[CollVertex],
    coll: &MapCollData,
) -> Vec<DerivedLedge> {
    let mut ledges = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line_kind(index, line, coll) != "floor"
            || (line.lo_flags & COLL_LINE_SOFT_FLOOR) != 0
            || (line.lo_flags & COLL_LINE_LEDGE) == 0
        {
            continue;
        }
        let Some(v0) = vertices.get(line.v0_idx as usize) else {
            continue;
        };
        let Some(v1) = vertices.get(line.v1_idx as usize) else {
            continue;
        };
        if !is_floor_line(lines, coll, line.prev_id0) {
            ledges.push(DerivedLedge {
                line_index: index,
                side: "left",
                x_milli: v0.x,
                y_milli: v0.y,
            });
        }
        if !is_floor_line(lines, coll, line.next_id0) {
            ledges.push(DerivedLedge {
                line_index: index,
                side: "right",
                x_milli: v1.x,
                y_milli: v1.y,
            });
        }
    }
    ledges.sort_by_key(|ledge| (ledge.x_milli, ledge.line_index, ledge.y_milli));
    ledges.dedup_by_key(|ledge| (ledge.x_milli, ledge.y_milli, ledge.side));
    ledges
}

fn is_floor_line(lines: &[MapLine], coll: &MapCollData, line_index: i16) -> bool {
    if line_index < 0 {
        return false;
    }
    let index = line_index as usize;
    lines
        .get(index)
        .is_some_and(|line| matches!(line_kind(index, line, coll), "floor" | "soft_floor"))
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
        "source_flags": line_flags_to_json(index, line, coll),
        "passable": (line.lo_flags & COLL_LINE_SOFT_FLOOR) != 0,
        "x0": v0.map(|vertex| vertex.x),
        "y0": v0.map(|vertex| vertex.y),
        "x1": v1.map(|vertex| vertex.x),
        "y1": v1.map(|vertex| vertex.y),
    })
}

fn line_flag_refs_to_json() -> Value {
    json!({
        "source": ".research/doldecomp-melee/src/melee/mp/forward.h",
        "hi_flags": {
            "LINE_FLAG_KIND": COLL_LINE_KIND_MASK,
            "LINE_FLAG_EMPTY": COLL_LINE_EMPTY
        },
        "lo_flags": {
            "LINE_FLAG_PLATFORM": COLL_LINE_PLATFORM,
            "LINE_FLAG_LEDGE": COLL_LINE_LEDGE
        },
        "runtime_flags": {
            "LINE_FLAG_ENABLED": COLL_LINE_ENABLED,
            "LINE_FLAG_HIDDEN": COLL_LINE_HIDDEN,
            "mpLibLoad": "CollLine.flags = MapLine.hi_flags | LINE_FLAG_ENABLED; MapLine.lo_flags stays source data"
        }
    })
}

fn line_flags_to_json(index: usize, line: &MapLine, coll: &MapCollData) -> Value {
    let enabled_after_mp_lib_load = line_kind(index, line, coll) != "unknown";
    let runtime_flags = if enabled_after_mp_lib_load {
        u32::from(line.hi_flags) | COLL_LINE_ENABLED
    } else {
        u32::from(line.hi_flags)
    };
    json!({
        "kind_mask": line.hi_flags & COLL_LINE_KIND_MASK,
        "empty": (line.hi_flags & COLL_LINE_EMPTY) != 0,
        "platform": (line.lo_flags & COLL_LINE_PLATFORM) != 0,
        "ledge": (line.lo_flags & COLL_LINE_LEDGE) != 0,
        "enabled_after_mpLibLoad": enabled_after_mp_lib_load,
        "runtime_flags_after_mpLibLoad": runtime_flags
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

fn derive_respawn_platforms_from_map_head(
    scene: &MapHeadScene,
    scale: f32,
    camera: &StageCameraMetadata,
) -> Option<Vec<StageRespawnPlatformMetadata>> {
    let top_y = source_units_to_milli(camera.cam_bounds.top + camera.cam_y_offset);
    let mut platforms = Vec::new();
    for platform_index in 0..4 {
        let stage_point_index = platform_index + 4;
        let mapping = scene
            .point_mappings
            .iter()
            .find(|mapping| mapping.stage_info_index == stage_point_index as u16)?;
        let final_x = source_units_to_milli(mapping.source_position.x * scale);
        let final_y = source_units_to_milli(mapping.source_position.y * scale);
        platforms.push(StageRespawnPlatformMetadata {
            platform_index: platform_index as u16,
            stage_point_index: stage_point_index as u16,
            final_x,
            final_y,
            top_y,
            facing: if final_x >= 0 { -1 } else { 1 },
            offset_x: 0,
            offset_y: 0,
        });
    }
    Some(platforms)
}

fn respawn_platform_to_json(platform: &StageRespawnPlatformMetadata) -> Value {
    json!({
        "platform_index": platform.platform_index,
        "stage_point_index": platform.stage_point_index,
        "final_x": platform.final_x,
        "final_y": platform.final_y,
        "top_y": platform.top_y,
        "facing": platform.facing,
        "offset_x": platform.offset_x,
        "offset_y": platform.offset_y,
        "source": {
            "decomp": [
                ".research/doldecomp-melee/src/melee/gm/gm_1601.c::fn_8016719C",
                ".research/doldecomp-melee/src/melee/ft/ft_0D4D.c::ftCo_800D4FF4",
                ".research/doldecomp-melee/src/melee/gr/stage.c::Stage_80224E38",
                ".research/doldecomp-melee/src/melee/gr/ground.c::Ground_801C34AC",
                ".research/doldecomp-melee/src/melee/gr/ground.c::Ground_801C2D24"
            ],
            "rule": "normal-stage Rebirth platform uses stage_info.x280[platform_index + 4] with camera top y as the spawn start and the mapped JObj world y as the Rebirth target"
        }
    })
}

fn map_head_object_tree_to_json(scene: &MapHeadScene, scale: f32) -> Value {
    json!({
        "source_root": "map_head",
        "stage_dat_offset": scene.stage_dat_offset,
        "stage_dat_offset_hex": format!("0x{:08x}", scene.stage_dat_offset),
        "unk0_offset": scene.unk0_offset,
        "unk4": scene.unk4,
        "entries_offset": scene.entries_offset,
        "entry_count": scene.entry_count,
        "splines_offset": scene.splines_offset,
        "spline_count": scene.spline_count,
        "unk18_offset": scene.unk18_offset,
        "unk1c": scene.unk1c,
        "unk20_offset": scene.unk20_offset,
        "unk24": scene.unk24,
        "internals_offset": scene.internals_offset,
        "internal_count": scene.internal_count,
        "entry_count_decoded": scene.entries.len(),
        "joint_count_decoded": scene.joints.len(),
        "entries": scene.entries.iter().enumerate().map(|(index, entry)| json!({
            "index": index,
            "joint_root_offset": entry.joint_root_offset,
            "joint_root_index": entry.joint_root_index,
            "camera_desc_offset": entry.camera_desc_offset,
            "x14_offset": entry.x14_offset,
            "x18_offset": entry.x18_offset,
            "fog_desc_offset": entry.fog_desc_offset,
            "vector_offset": entry.vector_offset,
            "vector_count": entry.vector_count,
            "x28_offset": entry.x28_offset,
            "x2c_offset": entry.x2c_offset,
            "x30": entry.x30,
        })).collect::<Vec<_>>(),
        "joints": scene.joints.iter().map(map_head_node_to_json).collect::<Vec<_>>(),
        "point_mappings": scene.point_mappings.iter().map(|mapping| json!({
            "tree_index": mapping.tree_index,
            "stage_info_index": mapping.stage_info_index,
            "joint_index": mapping.joint_index,
            "source_position": {
                "x": mapping.source_position.x,
                "y": mapping.source_position.y,
                "z": mapping.source_position.z,
            },
            "scaled_x": source_units_to_milli(mapping.source_position.x * scale),
            "scaled_y": source_units_to_milli(mapping.source_position.y * scale),
        })).collect::<Vec<_>>(),
        "struct_refs": [
            ".research/doldecomp-melee/src/melee/gr/grdatfiles.c::grDatFiles_801C6038",
            ".research/doldecomp-melee/src/melee/gr/ground.c::Ground_801C34AC",
            ".research/doldecomp-melee/src/melee/gr/types.h::UnkStageDat",
            ".research/doldecomp-melee/src/melee/gr/types.h::UnkStageDat_x8_t",
            ".research/doldecomp-melee/src/sysdolphin/baselib/jobj.h::HSD_Joint",
            ".research/doldecomp-melee/src/melee/gr/types.h::StageInfo",
        ],
    })
}

fn map_head_node_to_json(node: &MapHeadNode) -> Value {
    json!({
        "index": node.index,
        "node_offset": node.node_offset,
        "node_offset_hex": format!("0x{:08x}", node.node_offset),
        "class_name_offset": node.class_name_offset,
        "flags": node.flags,
        "child_offset": node.child_offset,
        "next_offset": node.next_offset,
        "child_index": node.child_index,
        "next_index": node.next_index,
        "dobjdesc_offset": node.dobjdesc_offset,
        "mtx_offset": node.mtx_offset,
        "robjdesc_offset": node.robjdesc_offset,
        "rotation": {
            "x": node.rotation_x,
            "y": node.rotation_y,
            "z": node.rotation_z,
        },
        "scale": {
            "x": node.scale_x,
            "y": node.scale_y,
            "z": node.scale_z,
            "x_milli": source_units_to_milli(node.scale_x),
            "y_milli": source_units_to_milli(node.scale_y),
            "z_milli": source_units_to_milli(node.scale_z),
        },
        "position": {
            "x": node.position_x,
            "y": node.position_y,
            "z": node.position_z,
            "x_milli": source_units_to_milli(node.position_x),
            "y_milli": source_units_to_milli(node.position_y),
            "z_milli": source_units_to_milli(node.position_z),
        },
    })
}

fn parse_stage_camera_metadata(
    dat: &[u8],
    roots: &DatRoots,
    ground_param_offset: Option<u32>,
    bounds: Option<&StageBoundsMetadata>,
) -> Result<StageCameraMetadata, String> {
    let default_bounds = StageFloatBoundsMetadata {
        left: -170.0,
        right: 170.0,
        top: 120.0,
        bottom: -60.0,
    };
    let Some(offset) = ground_param_offset.map(|offset| offset as usize) else {
        return Ok(StageCameraMetadata {
            cam_bounds: bounds.map_or(default_bounds, |bounds| bounds.camera),
            cam_x_offset: bounds.map_or(0.0, |bounds| bounds.cam_x_offset),
            cam_y_offset: bounds.map_or(0.0, |bounds| bounds.cam_y_offset),
            cam_vertical_tilt: 30.0,
            cam_pan_degrees: -10.0,
            x20: 0.2,
            x24: 0.2,
            cam_track_ratio: 0.0,
            cam_fixed_zoom: 0.0,
            cam_track_smooth: 0.0,
            cam_zoom_rate: 82.0,
            cam_max_depth: 1000.0,
            x3c: 0.0,
            pausecam_zpos_min: 0.0,
            pausecam_zpos_init: 0.0,
            pausecam_zpos_max: 0.0,
            cam_angle_up: 0.0,
            cam_angle_down: 0.0,
            cam_angle_left: 0.0,
            cam_angle_right: 0.0,
            fixed_cam_pos: StageVec3Metadata {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            fixed_cam_fov: 0.0,
            fixed_cam_vert_angle: 0.0,
            fixed_cam_horz_angle: 0.0,
        });
    };

    Ok(StageCameraMetadata {
        cam_bounds: bounds.map_or(default_bounds, |bounds| bounds.camera),
        cam_x_offset: bounds.map_or(0.0, |bounds| bounds.cam_x_offset),
        cam_y_offset: bounds.map_or(0.0, |bounds| bounds.cam_y_offset),
        cam_vertical_tilt: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x08,
            "grGroundParam.x8",
        )? as f32,
        cam_pan_degrees: read_data_i32(
            dat,
            roots.data_block_size,
            offset + 0x14,
            "grGroundParam.x14",
        )? as f32,
        x20: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x1C,
            "grGroundParam.x1C",
        )?,
        x24: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x18,
            "grGroundParam.x18",
        )?,
        cam_track_ratio: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x20,
            "grGroundParam.x20",
        )?,
        cam_fixed_zoom: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x24,
            "grGroundParam.x24",
        )?,
        cam_track_smooth: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x28,
            "grGroundParam.x28",
        )?,
        cam_zoom_rate: read_data_i32(
            dat,
            roots.data_block_size,
            offset + 0x0C,
            "grGroundParam.xC",
        )? as f32,
        cam_max_depth: read_data_i32(
            dat,
            roots.data_block_size,
            offset + 0x10,
            "grGroundParam.x10",
        )? as f32,
        x3c: read_data_i16(
            dat,
            roots.data_block_size,
            offset + 0x2E,
            "grGroundParam.x2E",
        )? as f32,
        pausecam_zpos_min: read_data_i32(
            dat,
            roots.data_block_size,
            offset + 0x30,
            "grGroundParam.x30",
        )? as f32,
        pausecam_zpos_init: read_data_i32(
            dat,
            roots.data_block_size,
            offset + 0x34,
            "grGroundParam.x34",
        )? as f32,
        pausecam_zpos_max: read_data_i32(
            dat,
            roots.data_block_size,
            offset + 0x38,
            "grGroundParam.x38",
        )? as f32,
        cam_angle_up: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x3C,
            "grGroundParam.x3C",
        )?,
        cam_angle_down: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x40,
            "grGroundParam.x40",
        )?,
        cam_angle_left: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x44,
            "grGroundParam.x44",
        )?,
        cam_angle_right: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x48,
            "grGroundParam.x48",
        )?,
        fixed_cam_pos: StageVec3Metadata {
            x: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x50,
                "grGroundParam.x50",
            )?,
            y: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x54,
                "grGroundParam.x54",
            )?,
            z: read_data_f32(
                dat,
                roots.data_block_size,
                offset + 0x58,
                "grGroundParam.x58",
            )?,
        },
        fixed_cam_fov: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x5C,
            "grGroundParam.x5C",
        )?,
        fixed_cam_vert_angle: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x60,
            "grGroundParam.x60",
        )?,
        fixed_cam_horz_angle: read_data_f32(
            dat,
            roots.data_block_size,
            offset + 0x64,
            "grGroundParam.x64",
        )?,
    })
}

fn derive_stage_bounds_from_map_head(
    scene: &MapHeadScene,
    scale: f32,
) -> Option<StageBoundsMetadata> {
    let world_positions = map_head_world_positions(scene);
    let root = scene.entries.first()?.joint_root_index?;
    let mut chain = Vec::new();
    let mut current = scene.joints.get(root)?;
    chain.push(current.index);
    while let Some(child) = current.child_index {
        current = scene.joints.get(child)?;
        chain.push(current.index);
        let mut sibling = current;
        while let Some(next) = sibling.next_index {
            sibling = scene.joints.get(next)?;
            chain.push(sibling.index);
        }
        break;
    }
    if chain.len() < 5 {
        return None;
    }
    let start = if let (Some(left), Some(right)) =
        (world_positions.get(chain[1]), world_positions.get(chain[2]))
    {
        if left.x < 0.0 && right.x > 0.0 {
            1
        } else {
            2
        }
    } else {
        2
    };
    if chain.len() <= start + 3 {
        return None;
    }
    let cam_a = *world_positions.get(chain[start])?;
    let cam_b = *world_positions.get(chain[start + 1])?;
    let blast_a = *world_positions.get(chain[start + 2])?;
    let blast_b = *world_positions.get(chain[start + 3])?;
    Some(StageBoundsMetadata {
        camera: scale_bounds(bounds_from_points(cam_a, cam_b), scale),
        blast: scale_bounds(bounds_from_points(blast_a, blast_b), scale),
        cam_x_offset: 0.0,
        cam_y_offset: 0.0,
        source: "map_head_marker_nodes_Ground_801C39C0",
    })
}

fn map_head_world_positions(scene: &MapHeadScene) -> Vec<StageVec3Metadata> {
    let mut positions = vec![
        StageVec3Metadata {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        scene.joints.len()
    ];
    let mut visited = vec![false; scene.joints.len()];
    for entry in &scene.entries {
        if let Some(root) = entry.joint_root_index {
            map_head_world_positions_walk(
                scene,
                root,
                StageVec3Metadata {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                StageVec3Metadata {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                &mut positions,
                &mut visited,
            );
        }
    }
    positions
}

fn map_head_world_positions_walk(
    scene: &MapHeadScene,
    index: usize,
    parent_pos: StageVec3Metadata,
    parent_scale: StageVec3Metadata,
    positions: &mut [StageVec3Metadata],
    visited: &mut [bool],
) {
    if visited.get(index).copied().unwrap_or(true) {
        return;
    }
    let Some(node) = scene.joints.get(index) else {
        return;
    };
    visited[index] = true;
    let pos = StageVec3Metadata {
        x: parent_pos.x + node.position_x * parent_scale.x,
        y: parent_pos.y + node.position_y * parent_scale.y,
        z: parent_pos.z + node.position_z * parent_scale.z,
    };
    let scale = StageVec3Metadata {
        x: parent_scale.x * node.scale_x,
        y: parent_scale.y * node.scale_y,
        z: parent_scale.z * node.scale_z,
    };
    positions[index] = pos;
    if let Some(child) = node.child_index {
        map_head_world_positions_walk(scene, child, pos, scale, positions, visited);
    }
    if let Some(next) = node.next_index {
        map_head_world_positions_walk(scene, next, parent_pos, parent_scale, positions, visited);
    }
}

fn bounds_from_points(a: StageVec3Metadata, b: StageVec3Metadata) -> StageFloatBoundsMetadata {
    StageFloatBoundsMetadata {
        left: a.x.min(b.x),
        right: a.x.max(b.x),
        top: a.y.max(b.y),
        bottom: a.y.min(b.y),
    }
}

fn scale_bounds(bounds: StageFloatBoundsMetadata, scale: f32) -> StageFloatBoundsMetadata {
    StageFloatBoundsMetadata {
        left: bounds.left * scale,
        right: bounds.right * scale,
        top: bounds.top * scale,
        bottom: bounds.bottom * scale,
    }
}

fn stage_float_bounds_to_json(bounds: &StageFloatBoundsMetadata) -> Value {
    json!({
        "left": bounds.left,
        "right": bounds.right,
        "top": bounds.top,
        "bottom": bounds.bottom,
    })
}

fn stage_camera_metadata_to_json(camera: &StageCameraMetadata) -> Value {
    json!({
        "cam_bounds": stage_float_bounds_to_json(&camera.cam_bounds),
        "cam_x_offset": camera.cam_x_offset,
        "cam_y_offset": camera.cam_y_offset,
        "cam_vertical_tilt": camera.cam_vertical_tilt,
        "cam_pan_degrees": camera.cam_pan_degrees,
        "x20": camera.x20,
        "x24": camera.x24,
        "cam_track_ratio": camera.cam_track_ratio,
        "cam_fixed_zoom": camera.cam_fixed_zoom,
        "cam_track_smooth": camera.cam_track_smooth,
        "cam_zoom_rate": camera.cam_zoom_rate,
        "cam_max_depth": camera.cam_max_depth,
        "x3c": camera.x3c,
        "pausecam_zpos_min": camera.pausecam_zpos_min,
        "pausecam_zpos_init": camera.pausecam_zpos_init,
        "pausecam_zpos_max": camera.pausecam_zpos_max,
        "cam_angle_up": camera.cam_angle_up,
        "cam_angle_down": camera.cam_angle_down,
        "cam_angle_left": camera.cam_angle_left,
        "cam_angle_right": camera.cam_angle_right,
        "fixed_cam_pos": {
            "x": camera.fixed_cam_pos.x,
            "y": camera.fixed_cam_pos.y,
            "z": camera.fixed_cam_pos.z,
        },
        "fixed_cam_fov": camera.fixed_cam_fov,
        "fixed_cam_vert_angle": camera.fixed_cam_vert_angle,
        "fixed_cam_horz_angle": camera.fixed_cam_horz_angle,
        "struct_refs": [
            ".research/doldecomp-melee/src/melee/gr/types.h::StageCameraInfo",
            ".research/doldecomp-melee/src/melee/gr/types.h::UnkStage6B0",
            ".research/doldecomp-melee/src/melee/gr/ground.c::Ground_801C0800",
            ".research/doldecomp-melee/src/melee/gr/ground.c::Ground_801C39C0",
            ".research/doldecomp-melee/src/melee/gr/stage.c",
        ],
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

fn empty_stage_callback_metadata() -> StageCallbackMetadata {
    StageCallbackMetadata {
        stage_data_symbol: String::new(),
        callback_table_symbol: String::new(),
        object_callbacks: Vec::new(),
        on_init: String::new(),
        on_demo_init: String::new(),
        on_load: String::new(),
        on_start: String::new(),
        callback4: String::new(),
        on_touch_line: String::new(),
        on_check_shadow_render: String::new(),
        flags2: 0,
        spawn_table_symbol: String::new(),
        spawn_table: Vec::new(),
    }
}

fn decomp_stage_metadata(stage: &ResolvedStageInput) -> Option<StageCallbackMetadata> {
    let decomp_ref = stage.decomp_ref?;
    let text = fs::read_to_string(decomp_ref).ok()?;
    let (stage_data_symbol, stage_data_fields) =
        parse_named_initializer_fields(&text, "StageData")?;
    let callback_table_symbol = normalize_c_token(stage_data_fields.get(1)?);
    let object_callbacks = parse_stage_callback_table(&text, &callback_table_symbol);
    let spawn_table_symbol =
        normalize_c_token(stage_data_fields.get(11).map_or("", String::as_str));
    let spawn_table = if spawn_table_symbol.is_empty() {
        Vec::new()
    } else {
        parse_s16vec3_table(&text, &spawn_table_symbol)
    };
    Some(StageCallbackMetadata {
        stage_data_symbol,
        callback_table_symbol,
        object_callbacks,
        on_init: normalize_c_token(stage_data_fields.get(3).map_or("", String::as_str)),
        on_demo_init: normalize_c_token(stage_data_fields.get(4).map_or("", String::as_str)),
        on_load: normalize_c_token(stage_data_fields.get(5).map_or("", String::as_str)),
        on_start: normalize_c_token(stage_data_fields.get(6).map_or("", String::as_str)),
        callback4: normalize_c_token(stage_data_fields.get(7).map_or("", String::as_str)),
        on_touch_line: normalize_c_token(stage_data_fields.get(8).map_or("", String::as_str)),
        on_check_shadow_render: normalize_c_token(
            stage_data_fields.get(9).map_or("", String::as_str),
        ),
        flags2: parse_c_u32(stage_data_fields.get(10).map_or("0", String::as_str)),
        spawn_table_symbol,
        spawn_table,
    })
}

fn parse_named_initializer_fields(text: &str, type_name: &str) -> Option<(String, Vec<String>)> {
    let marker = format!("{type_name} ");
    let start = text.find(&marker)? + marker.len();
    let after_type = &text[start..];
    let symbol = after_type
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if symbol.is_empty() {
        return None;
    }
    let initializer_start = text[start..].find('{')? + start;
    let initializer = balanced_brace_block(text, initializer_start)?;
    Some((symbol, split_top_level_commas(initializer)))
}

fn parse_stage_callback_table(text: &str, symbol: &str) -> Vec<StageObjectCallbackMetadata> {
    if symbol.is_empty() {
        return Vec::new();
    }
    let Some(symbol_pos) = text.find(symbol) else {
        return Vec::new();
    };
    let Some(open) = text[symbol_pos..]
        .find('{')
        .map(|relative| symbol_pos + relative)
    else {
        return Vec::new();
    };
    let Some(block) = balanced_brace_block(text, open) else {
        return Vec::new();
    };
    top_level_brace_entries(&block)
        .into_iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let fields = split_top_level_commas(&entry);
            if fields.len() < 5 {
                return None;
            }
            Some(StageObjectCallbackMetadata {
                object_id: index as u16,
                callback0: normalize_c_token(&fields[0]),
                callback1: normalize_c_token(&fields[1]),
                callback2: normalize_c_token(&fields[2]),
                callback3: normalize_c_token(&fields[3]),
                flags: parse_c_u32(&fields[4]),
            })
        })
        .collect()
}

fn parse_s16vec3_table(text: &str, symbol: &str) -> Vec<StageSpawnMappingMetadata> {
    let Some(symbol_pos) = text.find(symbol) else {
        return Vec::new();
    };
    let Some(open) = text[symbol_pos..]
        .find('{')
        .map(|relative| symbol_pos + relative)
    else {
        return Vec::new();
    };
    let Some(block) = balanced_brace_block(text, open) else {
        return Vec::new();
    };
    top_level_brace_entries(&block)
        .into_iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let fields = split_top_level_commas(&entry);
            if fields.len() < 3 {
                return None;
            }
            Some(StageSpawnMappingMetadata {
                index: index as u16,
                x: parse_c_i16(&fields[0]),
                y: parse_c_i16(&fields[1]),
                z: parse_c_i16(&fields[2]),
            })
        })
        .collect()
}

fn balanced_brace_block(text: &str, open_brace: usize) -> Option<&str> {
    let mut depth = 0_i32;
    let mut start = None;
    for (relative, ch) in text[open_brace..].char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(open_brace + relative + 1);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return start.map(|start| &text[start..open_brace + relative]);
                }
            }
            _ => {}
        }
    }
    None
}

fn top_level_brace_entries(block: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut depth = 0_i32;
    let mut start = None;
    for (index, ch) in block.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(index + 1);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = start.take() {
                        entries.push(block[start..index].to_string());
                    }
                }
            }
            _ => {}
        }
    }
    entries
}

fn split_top_level_commas(block: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (index, ch) in block.char_indices() {
        match ch {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                let value = block[start..index].trim();
                if !value.is_empty() {
                    fields.push(value.to_string());
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let value = block[start..].trim();
    if !value.is_empty() {
        fields.push(value.to_string());
    }
    fields
}

fn normalize_c_token(value: &str) -> String {
    let value = value.trim().trim_matches('"');
    if value == "NULL" || value == "0" {
        String::new()
    } else {
        value.to_string()
    }
}

fn parse_c_u32(value: &str) -> u32 {
    let value = value.trim().trim_end_matches('U').trim_end_matches('u');
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).unwrap_or(0)
    } else {
        value.parse().unwrap_or(0)
    }
}

fn parse_c_i16(value: &str) -> i16 {
    parse_c_u32(value) as i16
}

fn stage_callback_metadata_to_json(callbacks: &StageCallbackMetadata) -> Value {
    json!({
        "stage_data_symbol": callbacks.stage_data_symbol,
        "callback_table_symbol": callbacks.callback_table_symbol,
        "object_callbacks": callbacks.object_callbacks.iter().map(|callback| json!({
            "object_id": callback.object_id,
            "callback0": callback.callback0,
            "callback1": callback.callback1,
            "callback2": callback.callback2,
            "callback3": callback.callback3,
            "flags": callback.flags,
        })).collect::<Vec<_>>(),
        "on_init": callbacks.on_init,
        "on_demo_init": callbacks.on_demo_init,
        "on_load": callbacks.on_load,
        "on_start": callbacks.on_start,
        "callback4": callbacks.callback4,
        "on_touch_line": callbacks.on_touch_line,
        "on_check_shadow_render": callbacks.on_check_shadow_render,
        "flags2": callbacks.flags2,
        "spawn_table_symbol": callbacks.spawn_table_symbol,
        "spawn_table": callbacks.spawn_table.iter().map(|spawn| json!({
            "index": spawn.index,
            "x": spawn.x,
            "y": spawn.y,
            "z": spawn.z,
        })).collect::<Vec<_>>(),
        "struct_refs": [
            ".research/doldecomp-melee/src/melee/gr/types.h::StageCallbacks",
            ".research/doldecomp-melee/src/melee/gr/types.h::StageData",
        ],
    })
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

fn is_data_offset_in_range(data_block_size: usize, offset: u32, size: usize) -> bool {
    (offset as usize)
        .checked_add(size)
        .is_some_and(|end| end <= data_block_size)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_coll_data(floor_count: i16) -> MapCollData {
        MapCollData {
            verts_offset: 0,
            vert_count: 2,
            lines_offset: 0,
            line_count: floor_count.max(0) as usize,
            floor_start: 0,
            floor_count,
            ceiling_start: -1,
            ceiling_count: 0,
            right_wall_start: -1,
            right_wall_count: 0,
            left_wall_start: -1,
            left_wall_count: 0,
            dynamic_start: -1,
            dynamic_count: 0,
            joints_offset: 0,
            joint_count: 0,
            x2c: 0,
        }
    }

    fn floor_line(lo_flags: u16) -> MapLine {
        MapLine {
            v0_idx: 0,
            v1_idx: 1,
            prev_id0: -1,
            next_id0: -1,
            prev_id1: -1,
            next_id1: -1,
            hi_flags: COLL_LINE_FLOOR,
            lo_flags,
        }
    }

    #[test]
    fn derive_ledges_requires_source_line_ledge_flag() {
        let vertices = vec![
            CollVertex {
                source_x: -10.0,
                source_y: 0.0,
                pos_x: -10.0,
                pos_y: 0.0,
                x: -10_000,
                y: 0,
            },
            CollVertex {
                source_x: 10.0,
                source_y: 0.0,
                pos_x: 10.0,
                pos_y: 0.0,
                x: 10_000,
                y: 0,
            },
        ];
        let coll = test_coll_data(1);

        assert!(derive_ledges(&[floor_line(0)], &vertices, &coll).is_empty());
    }
}
