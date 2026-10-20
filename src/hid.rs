//! Joy-Con discovery via IOKit's IOHIDManager (macOS only).

use std::ffi::{CStr, c_void};
use std::ptr::NonNull;

use objc2_core_foundation::{
    CFDictionary, CFIndex, CFNumber, CFRetained, CFRunLoop, CFString, kCFRunLoopDefaultMode,
};

use objc2_io_kit::{
    IOHIDDevice, IOHIDManager, IOReturn, kIOHIDOptionsTypeNone, kIOHIDProductIDKey,
    kIOHIDProductKey, kIOHIDVendorIDKey, kIOReturnSuccess,
};

// For a Bluetooth-paired right Joy-Con,
// you can grab these with the following command on your MacOS:
// ```bash
// system_profiler SPBluetoothDataType| rg --after-context=5 'Joy-Con \(R\)'
// ```
/// Nintendo's USB vendor ID.
pub const NINTENDO_VID: i32 = 0x057E;
/// Right Joy-Con product ID.
pub const JOYCON_R_PID: i32 = 0x2007;

/// IOKit gives key names as C strings; CF APIs want CFStrings.
fn cf_key(key: &CStr) -> CFRetained<CFString> {
    CFString::from_str(key.to_str().expect("IOKit keys are ASCII"))
}

/// `{ "VendorID": 0x057E, "ProductID": 0x2007 }`
fn matching_dict() -> CFRetained<CFDictionary<CFString, CFNumber>> {
    let vid_key = cf_key(kIOHIDVendorIDKey);
    let pid_key = cf_key(kIOHIDProductIDKey);
    let vid = CFNumber::new_i32(NINTENDO_VID);
    let pid = CFNumber::new_i32(JOYCON_R_PID);
    CFDictionary::from_slices(&[&*vid_key, &*pid_key], &[&*vid, &*pid])
}

fn product_name(device: &IOHIDDevice) -> String {
    device
        // `property` returns an `Option` (the key may be absent), so the rest of the
        // chain operates on that optional value rather than on a bare result.
        .property(&cf_key(kIOHIDProductKey))
        // `and_then` runs only if the previous step was `Some`; it lets us return
        // another `Option`, flattening instead of nesting `Option<Option<_>>`.
        // `downcast_ref` is a checked runtime cast from the generic CoreFoundation
        // value to a concrete `CFString`; it yields `None` if the type doesn't match.
        .and_then(|value| value.downcast_ref::<CFString>().map(|s| s.to_string()))
        // `unwrap_or_else` supplies a fallback when any step above produced `None`.
        // The closure form is lazy: the `String` is only allocated if it's needed,
        // unlike `unwrap_or`, which would build it eagerly every call.
        .unwrap_or_else(|| "<unknown>".to_string())
}

/// Receives every input report: the report ID IOKit saw, and the bytes.
pub type ReportHandler = fn(report_id: u32, bytes: &[u8]);

/// Must be >= the device's MaxInputReportSize (see `ioreg`).
const REPORT_BUF_LEN: usize = 512;

/// Called by IOKit on the run loop thread when a matching device appears.
unsafe extern "C-unwind" fn on_match(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    device: NonNull<IOHIDDevice>,
) {
    // SAFETY: IOKit passes a valid device that lives for this call.
    let device = unsafe { device.as_ref() };
    println!("Connected: {}", product_name(device));

    // IOKit writes every report into this buffer for as long as the device
    // exists, so it must never move or be freed. Leak it (fixed in Task 9).
    let buf: &'static mut [u8; REPORT_BUF_LEN] = Box::leak(Box::new([0; REPORT_BUF_LEN]));
    let ptr = NonNull::from(buf).cast::<u8>();

    // SAFETY: `ptr` is valid for REPORT_BUF_LEN bytes forever; `context` is
    // the boxed ReportHandler from `run`, also alive forever.
    device.register_input_report_callback(
        ptr,
        REPORT_BUF_LEN as CFIndex,
        on_report, // TODO: To be written
        context,
    );
}

unsafe extern "C-unwind" fn on_remove(
    _context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    device: NonNull<IOHIDDevice>,
) {
    // SAFETY: IOKit passes a valid device that lives for this call.
    let device = unsafe { device.as_ref() };
    println!("Disconnected: {}", product_name(device));
}

/// Watch for the right Joy-Con forever (blocks the current thread).
pub fn run() -> Result<(), IOReturn> {
    let manager = IOHIDManager::new(None, kIOHIDOptionsTypeNone);
    let matching = matching_dict();
    let run_loop = CFRunLoop::current().expect("thread has a run loop");
    // SAFETY: Apple guarantees this global exists.
    let mode = unsafe { kCFRunLoopDefaultMode }.expect("default mode exists");

    // SAFETY: dict holds CFString keys / CFNumber values as IOKit expects;
    // callbacks match IOHIDDeviceCallback and ignore the (null) context.
    unsafe {
        manager.set_device_matching(Some(matching.as_opaque()));
        manager.register_device_matching_callback(Some(on_match), std::ptr::null_mut());
        manager.register_device_removal_callback(Some(on_remove), std::ptr::null_mut());
        manager.schedule_with_run_loop(&run_loop, mode);
    }

    let ret = manager.open(kIOHIDOptionsTypeNone);
    if ret != kIOReturnSuccess {
        return Err(ret);
    }

    println!("waiting for Joy-Con (R)... Ctrl-C to quit");
    CFRunLoop::run(); // blocks; callbacks fire from in here
    Ok(())
}
