#[derive(Clone, Copy, Debug)]
pub(crate) struct PinchGesture {
    pub(crate) magnification: f64,
    pub(crate) location_y: f64,
    pub(crate) ended: bool,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::PinchGesture;
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventPhase};
    use std::{ptr::NonNull, sync::Mutex, sync::Once};

    static INSTALL: Once = Once::new();
    static PENDING: Mutex<Option<PinchGesture>> = Mutex::new(None);

    pub(crate) fn install() {
        INSTALL.call_once(|| {
            let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
                // SAFETY: AppKit guarantees that the event pointer passed to a local
                // event monitor remains valid for the duration of this callback.
                let event_ref = unsafe { event.as_ref() };
                let magnification = event_ref.magnification();
                let location = event_ref.locationInWindow();
                let ended = event_ref
                    .phase()
                    .intersects(NSEventPhase::Ended | NSEventPhase::Cancelled);
                let mut pending = PENDING.lock().unwrap_or_else(|error| error.into_inner());
                if let Some(pending) = pending.as_mut() {
                    pending.magnification += magnification;
                    pending.location_y = location.y;
                    pending.ended |= ended;
                } else {
                    *pending = Some(PinchGesture {
                        magnification,
                        location_y: location.y,
                        ended,
                    });
                }
                event.as_ptr()
            });

            // SAFETY: The block returns the original valid NSEvent pointer. The
            // monitor is intentionally retained for the lifetime of the process.
            if let Some(monitor) = unsafe {
                NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                    NSEventMask::Magnify,
                    &handler,
                )
            } {
                std::mem::forget(monitor);
            }
        });
    }

    pub(crate) fn take() -> Option<PinchGesture> {
        PENDING
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }
}

#[cfg(target_os = "macos")]
pub(crate) use platform::{install, take};

#[cfg(not(target_os = "macos"))]
pub(crate) fn install() {}

#[cfg(not(target_os = "macos"))]
pub(crate) fn take() -> Option<PinchGesture> {
    None
}
