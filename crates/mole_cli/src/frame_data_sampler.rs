use mole_frame_data::FrameDataSampleOptions as SharedFrameDataSampleOptions;
use serde_json::{json, Value};
use std::path::Path;

use crate::{base_report, FrameDataSampleOptions};

pub(crate) fn frame_data_sample_report(root: &Path, options: &FrameDataSampleOptions) -> Value {
    let mut report = base_report("frame-data sample", root);
    report["character"] = json!(options.character);
    report["source_character"] = json!(options.source_character);
    report["state"] = json!(options.state);
    report["frame"] = json!(options.frame);
    report["wrote_artifact"] = json!(false);
    report["source_space"] = json!("melee_xyz");

    match sample_state_frame(root, options) {
        Ok(sample) => {
            report["ok"] = json!(true);
            report["sample"] = sample;
            report["errors"] = json!([]);
        }
        Err(error) => {
            report["ok"] = json!(false);
            report["sample"] = Value::Null;
            report["errors"] = json!([error]);
        }
    }

    report
}

pub(crate) fn sample_state_frame(
    root: &Path,
    options: &FrameDataSampleOptions,
) -> Result<Value, String> {
    mole_frame_data::sample_state_frame(root, &shared_options(options))
}

fn shared_options(options: &FrameDataSampleOptions) -> SharedFrameDataSampleOptions {
    SharedFrameDataSampleOptions {
        character: options.character.clone(),
        source_character: options.source_character.clone(),
        state: options.state.clone(),
        frame: options.frame,
    }
}
