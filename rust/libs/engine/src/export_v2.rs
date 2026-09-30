use std::{collections::HashMap, path::Path};

use timeline::{TimelineSerialization, serialization::Clip};

use crate::export::ExportOption;
use anyhow::Result;

#[cfg(any())]
pub fn export(
    timeline_serialization: &TimelineSerialization,
    output_path: &Path,
    option: &ExportOption,
) -> Result<()> {
    // identify all the decoders we need
    // let decoders = HashMap::new();
    for clip in timeline_serialization.editing_state.clips {
        match clip {
            Clip::Video(media_clip_data) => {
                let asset_id = media_clip_data.asset_id;
            }
            Clip::Audio(media_clip_data) => todo!(),
            Clip::Text(text_clip) => todo!(),
        }
    }
    return Ok(());
}

// fn locate
