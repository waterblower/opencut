use crate::video3::VideoFrame;
use anyhow::Result;
use std::{marker::PhantomData, rc::Rc};

pub struct RgbaImage {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Separate reusable scaling resources, owned by the video execution lane.
pub struct FrameConverter {
    _lane_local: PhantomData<Rc<()>>,
}

impl FrameConverter {
    pub fn new() -> Self {
        Self {
            _lane_local: PhantomData,
        }
    }

    /// Transfer hardware frames only when needed, apply color/rotation, and
    /// reconfigure from actual frame dimensions/format. No GPUI dependency.
    pub fn convert(&mut self, _frame: &VideoFrame) -> Result<RgbaImage> {
        todo!("V4: convert the selected frame with reusable scaling resources")
    }
}
