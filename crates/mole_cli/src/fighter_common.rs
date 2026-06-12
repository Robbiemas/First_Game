use serde_json::{json, Value};
use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path};

use crate::{FighterCommonCommand, FighterCommonExtractOptions, SCHEMA_VERSION};

const DAT_DATA_BLOCK_BASE: usize = 0x20;
pub(crate) const FIGHTER_COMMON_GENERATOR: &str = "crates/mole_cli/src/fighter_common.rs";
pub(crate) const FIGHTER_COMMON_COMMAND: &str =
    "cargo run -p mole_cli -- fighter-common extract --write --json";
const DEFAULT_PLCO_DAT: &str = "resources/melee/raw/PlCo.dat";
pub(crate) const FIGHTER_COMMON_INPUTS: &[&str] = &[DEFAULT_PLCO_DAT];
const ACCESSORY_ASSET_PATH: &str = "resources/melee/extracted/fighter_common_accessories.json";
const ENGINE_FIGHTER_COMMON_MODULE: &str = "crates/mole_core/src/generated/fighter_common.rs";
pub(crate) const FIGHTER_COMMON_OUTPUTS: &[&str] =
    &[ACCESSORY_ASSET_PATH, ENGINE_FIGHTER_COMMON_MODULE];
const HSD_JOINT_SIZE: usize = 0x40;
const HSD_VTX_DESC_SIZE: usize = 0x18;
const GX_VA_POS: u32 = 9;
const GX_VA_NULL: u32 = 0xFF;
const GX_DIRECT: u32 = 1;
const GX_INDEX8: u32 = 2;
const GX_INDEX16: u32 = 3;
const GX_POS_XYZ: u32 = 1;
const GX_S16: u32 = 3;
const GX_F32: u32 = 4;

pub(crate) fn fighter_common_report(root: &Path, command: &FighterCommonCommand) -> Value {
    match command {
        FighterCommonCommand::Extract(options) => extract_report(root, options),
    }
}

fn extract_report(root: &Path, options: &FighterCommonExtractOptions) -> Value {
    match extract_accessory(root, options) {
        Ok(output) => {
            let mut report = json!({
                "schema_version": SCHEMA_VERSION,
                "command": "fighter-common extract",
                "project_root": root.display().to_string(),
                "ok": true,
                "mutated": options.write,
                "source": output.asset["source"].clone(),
                "accessory": output.asset["accessory"].clone(),
                "written_paths": output.written_paths,
                "engine_blob": {
                    "path": ENGINE_FIGHTER_COMMON_MODULE,
                    "status": if options.write { "written" } else { "preview" },
                },
            });
            if !options.write {
                report["recommended_next"] = json!(["mole fighter-common extract --write --json",]);
            }
            report
        }
        Err(error) => json!({
            "schema_version": SCHEMA_VERSION,
            "command": "fighter-common extract",
            "project_root": root.display().to_string(),
            "ok": false,
            "mutated": false,
            "error": error,
        }),
    }
}

struct ExtractOutput {
    asset: Value,
    written_paths: Vec<String>,
}

fn extract_accessory(
    root: &Path,
    options: &FighterCommonExtractOptions,
) -> Result<ExtractOutput, String> {
    let dat_path = options.dat.as_deref().unwrap_or(DEFAULT_PLCO_DAT);
    let dat = fs::read(root.join(dat_path))
        .map_err(|error| format!("failed to read {}: {error}", root.join(dat_path).display()))?;
    let roots = parse_dat_roots(&dat)?;
    let root_offset = *roots
        .roots
        .get(&options.root_symbol)
        .ok_or_else(|| format!("DAT is missing root symbol {}", options.root_symbol))?;
    let pointer_offset = root_offset
        .checked_add(
            u32::try_from(options.slot)
                .map_err(|_| "slot is too large".to_string())?
                .checked_mul(4)
                .ok_or_else(|| "slot offset overflowed".to_string())?,
        )
        .ok_or_else(|| "root slot offset overflowed".to_string())?;
    let joint_offset = read_data_u32(
        &dat,
        roots.data_block_size,
        pointer_offset as usize,
        "ftLoadCommonData slot",
    )?;
    let joint = parse_joint(&dat, &roots, joint_offset)?;
    let dobj = parse_dobj(&dat, &roots, joint.dobj_offset)?;
    let pobj = parse_pobj(&dat, &roots, dobj.pobj_offset)?;
    let vtx_descs = parse_vtx_descs(&dat, &roots, pobj.verts_offset)?;
    let mesh = decode_pobj_mesh_bounds(&dat, &roots, &pobj, &vtx_descs)?;

    let asset = json!({
        "schema_version": SCHEMA_VERSION,
        "source": {
            "kind": "melee_fighter_common_dat",
            "raw_dat": dat_path,
            "dat_file": stage_dat_file_name(dat_path),
            "root_symbol": options.root_symbol,
            "pointer_table_slot": options.slot,
            "pointer_table_offset": pointer_offset,
            "pointer_table_offset_hex": hex(pointer_offset),
            "decomp_refs": [
                ".research/doldecomp-melee/src/melee/ft/fighter.c::Fighter_LoadCommonData",
                ".research/doldecomp-melee/src/melee/ft/ft_0C31.c::ftCo_800C6408",
                ".research/doldecomp-melee/src/sysdolphin/baselib/jobj.h::HSD_Joint",
                ".research/doldecomp-melee/src/sysdolphin/baselib/pobj.h::HSD_PObjDesc"
            ],
        },
        "accessory": {
            "symbol": options.symbol,
            "joint_root_data_offset": joint_offset,
            "joint_root_data_offset_hex": hex(joint_offset),
            "joint_count": 1,
            "joint": joint_to_json(&joint),
            "dobj": dobj_to_json(&dobj),
            "pobj": pobj_to_json(&pobj),
            "vertex_descriptors": vtx_descs.iter().map(vtx_desc_to_json).collect::<Vec<_>>(),
            "mesh": mesh_to_json(&mesh),
        },
    });

    let mut written_paths = Vec::new();
    if options.write {
        write_json(root, ACCESSORY_ASSET_PATH, &asset)?;
        written_paths.push(ACCESSORY_ASSET_PATH.to_string());
        let rust = generated_rust(options, dat_path, &joint, &mesh)?;
        write_text(root, ENGINE_FIGHTER_COMMON_MODULE, &rust)?;
        written_paths.push(ENGINE_FIGHTER_COMMON_MODULE.to_string());
    }

    Ok(ExtractOutput {
        asset,
        written_paths,
    })
}

#[derive(Debug, Clone)]
struct DatRoots {
    data_block_size: usize,
    roots: BTreeMap<String, u32>,
}

#[derive(Debug, Clone)]
struct JointDesc {
    data_offset: u32,
    flags: u32,
    child_offset: u32,
    next_offset: u32,
    dobj_offset: u32,
    rotation: Vec3,
    scale: Vec3,
    position: Vec3,
}

#[derive(Debug, Clone)]
struct DObjDesc {
    data_offset: u32,
    next_offset: u32,
    mobj_offset: u32,
    pobj_offset: u32,
}

#[derive(Debug, Clone)]
struct PObjDesc {
    data_offset: u32,
    next_offset: u32,
    verts_offset: u32,
    flags: u16,
    n_display: u16,
    display_offset: u32,
    union_offset: u32,
}

#[derive(Debug, Clone)]
struct VtxDesc {
    data_offset: u32,
    attr: u32,
    attr_type: u32,
    comp_cnt: u32,
    comp_type: u32,
    frac: u8,
    stride: u16,
    vertex_offset: u32,
}

#[derive(Debug, Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone)]
struct MeshBounds {
    primitive_count: usize,
    vertex_emit_count: usize,
    unique_position_index_count: usize,
    position_index_min: u16,
    position_index_max: u16,
    min: Vec3,
    max: Vec3,
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
        roots,
    })
}

fn parse_joint(dat: &[u8], roots: &DatRoots, offset: u32) -> Result<JointDesc, String> {
    let base = offset as usize;
    checked_data_offset(roots.data_block_size, base, HSD_JOINT_SIZE, "HSD_Joint")?;
    Ok(JointDesc {
        data_offset: offset,
        flags: read_data_u32(dat, roots.data_block_size, base + 0x04, "HSD_Joint.flags")?,
        child_offset: read_data_u32(dat, roots.data_block_size, base + 0x08, "HSD_Joint.child")?,
        next_offset: read_data_u32(dat, roots.data_block_size, base + 0x0C, "HSD_Joint.next")?,
        dobj_offset: read_data_u32(dat, roots.data_block_size, base + 0x10, "HSD_Joint.dobj")?,
        rotation: read_data_vec3(
            dat,
            roots.data_block_size,
            base + 0x14,
            "HSD_Joint.rotation",
        )?,
        scale: read_data_vec3(dat, roots.data_block_size, base + 0x20, "HSD_Joint.scale")?,
        position: read_data_vec3(
            dat,
            roots.data_block_size,
            base + 0x2C,
            "HSD_Joint.position",
        )?,
    })
}

fn parse_dobj(dat: &[u8], roots: &DatRoots, offset: u32) -> Result<DObjDesc, String> {
    let base = offset as usize;
    Ok(DObjDesc {
        data_offset: offset,
        next_offset: read_data_u32(dat, roots.data_block_size, base + 0x04, "HSD_DObjDesc.next")?,
        mobj_offset: read_data_u32(dat, roots.data_block_size, base + 0x08, "HSD_DObjDesc.mobj")?,
        pobj_offset: read_data_u32(dat, roots.data_block_size, base + 0x0C, "HSD_DObjDesc.pobj")?,
    })
}

fn parse_pobj(dat: &[u8], roots: &DatRoots, offset: u32) -> Result<PObjDesc, String> {
    let base = offset as usize;
    Ok(PObjDesc {
        data_offset: offset,
        next_offset: read_data_u32(dat, roots.data_block_size, base + 0x04, "HSD_PObjDesc.next")?,
        verts_offset: read_data_u32(
            dat,
            roots.data_block_size,
            base + 0x08,
            "HSD_PObjDesc.verts",
        )?,
        flags: read_data_u16(
            dat,
            roots.data_block_size,
            base + 0x0C,
            "HSD_PObjDesc.flags",
        )?,
        n_display: read_data_u16(
            dat,
            roots.data_block_size,
            base + 0x0E,
            "HSD_PObjDesc.n_display",
        )?,
        display_offset: read_data_u32(
            dat,
            roots.data_block_size,
            base + 0x10,
            "HSD_PObjDesc.display",
        )?,
        union_offset: read_data_u32(dat, roots.data_block_size, base + 0x14, "HSD_PObjDesc.u")?,
    })
}

fn parse_vtx_descs(dat: &[u8], roots: &DatRoots, offset: u32) -> Result<Vec<VtxDesc>, String> {
    let mut out = Vec::new();
    for index in 0..64 {
        let base = offset as usize + index * HSD_VTX_DESC_SIZE;
        let attr = read_data_u32(dat, roots.data_block_size, base, "HSD_VtxDescList.attr")?;
        if attr == GX_VA_NULL {
            break;
        }
        out.push(VtxDesc {
            data_offset: (offset as usize + index * HSD_VTX_DESC_SIZE) as u32,
            attr,
            attr_type: read_data_u32(
                dat,
                roots.data_block_size,
                base + 0x04,
                "HSD_VtxDescList.attr_type",
            )?,
            comp_cnt: read_data_u32(
                dat,
                roots.data_block_size,
                base + 0x08,
                "HSD_VtxDescList.comp_cnt",
            )?,
            comp_type: read_data_u32(
                dat,
                roots.data_block_size,
                base + 0x0C,
                "HSD_VtxDescList.comp_type",
            )?,
            frac: read_data_u8(
                dat,
                roots.data_block_size,
                base + 0x10,
                "HSD_VtxDescList.frac",
            )?,
            stride: read_data_u16(
                dat,
                roots.data_block_size,
                base + 0x12,
                "HSD_VtxDescList.stride",
            )?,
            vertex_offset: read_data_u32(
                dat,
                roots.data_block_size,
                base + 0x14,
                "HSD_VtxDescList.vertex",
            )?,
        });
    }
    Ok(out)
}

fn decode_pobj_mesh_bounds(
    dat: &[u8],
    roots: &DatRoots,
    pobj: &PObjDesc,
    descs: &[VtxDesc],
) -> Result<MeshBounds, String> {
    let Some(pos_desc) = descs.iter().find(|desc| desc.attr == GX_VA_POS) else {
        return Err("PObj has no GX_VA_POS vertex descriptor".to_string());
    };
    if pos_desc.comp_cnt != GX_POS_XYZ {
        return Err(format!(
            "GX_VA_POS descriptor is not GX_POS_XYZ: {}",
            pos_desc.comp_cnt
        ));
    }
    let display_len = usize::from(pobj.n_display)
        .checked_shl(5)
        .ok_or_else(|| "display length overflowed".to_string())?;
    let display_start = DAT_DATA_BLOCK_BASE
        + checked_data_offset(
            roots.data_block_size,
            pobj.display_offset as usize,
            display_len,
            "PObj display list",
        )?;
    let display_end = display_start + display_len;
    let mut cursor = display_start;
    let mut primitive_count = 0_usize;
    let mut vertex_emit_count = 0_usize;
    let mut seen = [false; 65536];
    let mut unique_position_index_count = 0_usize;
    let mut position_index_min = u16::MAX;
    let mut position_index_max = 0_u16;
    let mut min = Vec3 {
        x: f32::INFINITY,
        y: f32::INFINITY,
        z: f32::INFINITY,
    };
    let mut max = Vec3 {
        x: f32::NEG_INFINITY,
        y: f32::NEG_INFINITY,
        z: f32::NEG_INFINITY,
    };

    while cursor + 3 <= display_end {
        let opcode = dat[cursor];
        if opcode & 0xF8 == 0 {
            break;
        }
        let vertex_count = u16::from_be_bytes([dat[cursor + 1], dat[cursor + 2]]) as usize;
        cursor += 3;
        primitive_count += 1;
        for _ in 0..vertex_count {
            for desc in descs {
                let index = read_display_attr_index(dat, display_end, &mut cursor, desc)?;
                if desc.attr == GX_VA_POS {
                    let point = read_position(dat, roots.data_block_size, pos_desc, index)?;
                    min.x = min.x.min(point.x);
                    min.y = min.y.min(point.y);
                    min.z = min.z.min(point.z);
                    max.x = max.x.max(point.x);
                    max.y = max.y.max(point.y);
                    max.z = max.z.max(point.z);
                    if !seen[index as usize] {
                        seen[index as usize] = true;
                        unique_position_index_count += 1;
                    }
                    position_index_min = position_index_min.min(index);
                    position_index_max = position_index_max.max(index);
                    vertex_emit_count += 1;
                }
            }
        }
    }

    if vertex_emit_count == 0 {
        return Err("PObj display list emitted no positions".to_string());
    }

    Ok(MeshBounds {
        primitive_count,
        vertex_emit_count,
        unique_position_index_count,
        position_index_min,
        position_index_max,
        min,
        max,
    })
}

fn read_display_attr_index(
    dat: &[u8],
    display_end: usize,
    cursor: &mut usize,
    desc: &VtxDesc,
) -> Result<u16, String> {
    match desc.attr_type {
        GX_INDEX8 => {
            if *cursor >= display_end {
                return Err("display list ended while reading GX_INDEX8".to_string());
            }
            let index = dat[*cursor] as u16;
            *cursor += 1;
            Ok(index)
        }
        GX_INDEX16 => {
            if *cursor + 1 >= display_end {
                return Err("display list ended while reading GX_INDEX16".to_string());
            }
            let index = u16::from_be_bytes([dat[*cursor], dat[*cursor + 1]]);
            *cursor += 2;
            Ok(index)
        }
        GX_DIRECT => {
            let bytes = direct_attr_size(desc)?;
            if *cursor + bytes > display_end {
                return Err("display list ended while skipping GX_DIRECT attribute".to_string());
            }
            let direct_offset = *cursor;
            *cursor += bytes;
            Ok(u16::try_from(direct_offset).unwrap_or(u16::MAX))
        }
        other => Err(format!("unsupported GX attr_type {other}")),
    }
}

fn direct_attr_size(desc: &VtxDesc) -> Result<usize, String> {
    let component_count = match desc.attr {
        GX_VA_POS => match desc.comp_cnt {
            0 => 2,
            GX_POS_XYZ => 3,
            other => return Err(format!("unsupported direct position comp_cnt {other}")),
        },
        _ => 1,
    };
    let component_size = match desc.comp_type {
        GX_S16 => 2,
        GX_F32 => 4,
        0 | 1 => 1,
        2 => 2,
        other => return Err(format!("unsupported direct comp_type {other}")),
    };
    Ok(component_count * component_size)
}

fn read_position(
    dat: &[u8],
    data_block_size: usize,
    desc: &VtxDesc,
    index: u16,
) -> Result<Vec3, String> {
    let base = desc.vertex_offset as usize + usize::from(index) * usize::from(desc.stride);
    match desc.comp_type {
        GX_S16 => {
            let divisor = 2_f32.powi(i32::from(desc.frac));
            Ok(Vec3 {
                x: f32::from(read_data_i16(dat, data_block_size, base, "GX_VA_POS.x")?) / divisor,
                y: f32::from(read_data_i16(
                    dat,
                    data_block_size,
                    base + 2,
                    "GX_VA_POS.y",
                )?) / divisor,
                z: f32::from(read_data_i16(
                    dat,
                    data_block_size,
                    base + 4,
                    "GX_VA_POS.z",
                )?) / divisor,
            })
        }
        GX_F32 => Ok(Vec3 {
            x: read_data_f32(dat, data_block_size, base, "GX_VA_POS.x")?,
            y: read_data_f32(dat, data_block_size, base + 4, "GX_VA_POS.y")?,
            z: read_data_f32(dat, data_block_size, base + 8, "GX_VA_POS.z")?,
        }),
        other => Err(format!("unsupported GX_VA_POS comp_type {other}")),
    }
}

fn generated_rust(
    options: &FighterCommonExtractOptions,
    dat_path: &str,
    joint: &JointDesc,
    mesh: &MeshBounds,
) -> Result<String, String> {
    let mut out = String::new();
    writeln!(
        out,
        "// @generated by mole_cli fighter-common extract; do not edit by hand."
    )
    .unwrap();
    writeln!(
        out,
        "use crate::state::{{FighterCommonAccessoryProfile, SourceBounds3, SourceVec3}};"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "pub(crate) const COMMON_TROPHY_PLATFORM_ACCESSORY: FighterCommonAccessoryProfile = FighterCommonAccessoryProfile {{"
    )
    .unwrap();
    writeln!(out, "    symbol: {:?},", options.symbol).unwrap();
    writeln!(out, "    source_dat: {:?},", stage_dat_file_name(dat_path)).unwrap();
    writeln!(out, "    root_symbol: {:?},", options.root_symbol).unwrap();
    writeln!(out, "    pointer_table_slot: {},", options.slot).unwrap();
    writeln!(
        out,
        "    joint_root_data_offset: 0x{:x},",
        joint.data_offset
    )
    .unwrap();
    writeln!(out, "    joint_count: 1,").unwrap();
    writeln!(out, "    mesh_primitive_count: {},", mesh.primitive_count).unwrap();
    writeln!(
        out,
        "    mesh_vertex_emit_count: {},",
        mesh.vertex_emit_count
    )
    .unwrap();
    writeln!(
        out,
        "    mesh_unique_position_index_count: {},",
        mesh.unique_position_index_count
    )
    .unwrap();
    writeln!(out, "    mesh_bounds: SourceBounds3 {{").unwrap();
    writeln!(
        out,
        "        min: SourceVec3 {{ x: {}, y: {}, z: {} }},",
        f32_literal(mesh.min.x),
        f32_literal(mesh.min.y),
        f32_literal(mesh.min.z)
    )
    .unwrap();
    writeln!(
        out,
        "        max: SourceVec3 {{ x: {}, y: {}, z: {} }},",
        f32_literal(mesh.max.x),
        f32_literal(mesh.max.y),
        f32_literal(mesh.max.z)
    )
    .unwrap();
    writeln!(out, "    }},").unwrap();
    writeln!(out, "}};").unwrap();
    Ok(out)
}

fn joint_to_json(joint: &JointDesc) -> Value {
    json!({
        "data_offset": joint.data_offset,
        "data_offset_hex": hex(joint.data_offset),
        "flags": joint.flags,
        "flags_hex": hex(joint.flags),
        "child_offset": joint.child_offset,
        "next_offset": joint.next_offset,
        "dobj_offset": joint.dobj_offset,
        "rotation": vec3_to_json(joint.rotation),
        "scale": vec3_to_json(joint.scale),
        "position": vec3_to_json(joint.position),
    })
}

fn dobj_to_json(dobj: &DObjDesc) -> Value {
    json!({
        "data_offset": dobj.data_offset,
        "data_offset_hex": hex(dobj.data_offset),
        "next_offset": dobj.next_offset,
        "mobj_offset": dobj.mobj_offset,
        "pobj_offset": dobj.pobj_offset,
    })
}

fn pobj_to_json(pobj: &PObjDesc) -> Value {
    json!({
        "data_offset": pobj.data_offset,
        "data_offset_hex": hex(pobj.data_offset),
        "next_offset": pobj.next_offset,
        "verts_offset": pobj.verts_offset,
        "flags": pobj.flags,
        "flags_hex": hex(u32::from(pobj.flags)),
        "n_display": pobj.n_display,
        "display_offset": pobj.display_offset,
        "display_offset_hex": hex(pobj.display_offset),
        "union_offset": pobj.union_offset,
    })
}

fn vtx_desc_to_json(desc: &VtxDesc) -> Value {
    json!({
        "data_offset": desc.data_offset,
        "data_offset_hex": hex(desc.data_offset),
        "attr": desc.attr,
        "attr_type": desc.attr_type,
        "comp_cnt": desc.comp_cnt,
        "comp_type": desc.comp_type,
        "frac": desc.frac,
        "stride": desc.stride,
        "vertex_offset": desc.vertex_offset,
        "vertex_offset_hex": hex(desc.vertex_offset),
    })
}

fn mesh_to_json(mesh: &MeshBounds) -> Value {
    json!({
        "primitive_count": mesh.primitive_count,
        "vertex_emit_count": mesh.vertex_emit_count,
        "unique_position_index_count": mesh.unique_position_index_count,
        "position_index_min": mesh.position_index_min,
        "position_index_max": mesh.position_index_max,
        "bounds": {
            "min": vec3_to_json(mesh.min),
            "max": vec3_to_json(mesh.max),
        },
    })
}

fn vec3_to_json(vec: Vec3) -> Value {
    json!({ "x": vec.x, "y": vec.y, "z": vec.z })
}

fn read_data_vec3(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<Vec3, String> {
    Ok(Vec3 {
        x: read_data_f32(dat, data_block_size, offset, field)?,
        y: read_data_f32(dat, data_block_size, offset + 4, field)?,
        z: read_data_f32(dat, data_block_size, offset + 8, field)?,
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

fn read_data_u16(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<u16, String> {
    let file_offset = DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 2, field)?;
    Ok(u16::from_be_bytes([dat[file_offset], dat[file_offset + 1]]))
}

fn read_data_i16(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<i16, String> {
    let file_offset = DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 2, field)?;
    Ok(i16::from_be_bytes([dat[file_offset], dat[file_offset + 1]]))
}

fn read_data_u8(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<u8, String> {
    let file_offset = DAT_DATA_BLOCK_BASE + checked_data_offset(data_block_size, offset, 1, field)?;
    Ok(dat[file_offset])
}

fn read_data_f32(
    dat: &[u8],
    data_block_size: usize,
    offset: usize,
    field: &str,
) -> Result<f32, String> {
    let value = f32::from_bits(read_data_u32(dat, data_block_size, offset, field)?);
    if !value.is_finite() {
        return Err(format!("DAT float {field} at 0x{offset:x} is not finite"));
    }
    Ok(value)
}

fn checked_data_offset(
    data_block_size: usize,
    offset: usize,
    len: usize,
    field: &str,
) -> Result<usize, String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("DAT offset overflow for {field}"))?;
    if end > data_block_size {
        return Err(format!(
            "DAT field {field} at 0x{offset:x}..0x{end:x} is outside data block 0x{data_block_size:x}"
        ));
    }
    Ok(offset)
}

fn read_u32(bytes: &[u8], offset: usize, field: &str) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| format!("offset overflow for {field}"))?;
    if end > bytes.len() {
        return Err(format!(
            "{field} at 0x{offset:x}..0x{end:x} is outside file length 0x{:x}",
            bytes.len()
        ));
    }
    Ok(u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

fn dat_string(bytes: &[u8], offset: usize) -> Result<String, String> {
    if offset >= bytes.len() {
        return Err(format!("DAT string offset 0x{offset:x} is outside file"));
    }
    let end = bytes[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|relative| offset + relative)
        .ok_or_else(|| format!("DAT string at 0x{offset:x} is not NUL-terminated"))?;
    std::str::from_utf8(&bytes[offset..end])
        .map(str::to_string)
        .map_err(|error| format!("DAT string at 0x{offset:x} is not UTF-8: {error}"))
}

fn write_json(root: &Path, relative: &str, value: &Value) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {relative}: {error}"))?;
    fs::write(&path, format!("{text}\n")).map_err(|error| format!("{}: {error}", path.display()))
}

fn write_text(root: &Path, relative: &str, text: &str) -> Result<(), String> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))
}

fn f32_literal(value: f32) -> String {
    if value == 0.0 {
        return "0.0_f32".to_string();
    }
    let mut literal = format!("{value:?}");
    if !literal.contains('.') && !literal.contains('e') && !literal.contains('E') {
        literal.push_str(".0");
    }
    literal.push_str("_f32");
    literal
}

fn hex(value: u32) -> String {
    format!("0x{value:08x}")
}

fn stage_dat_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}
