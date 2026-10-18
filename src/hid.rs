//! Joy-Con discovery via IOKit's IOHIDManager (macOS only).

use std::ffi::{CStr, c_void};
use std::ptr::NonNull;

use objc2_core_foundation::{
    CFDictionary, CFNumber, CFRetained, CFRunLoop, CFString, kCFRunLoopDefaultMode,
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
fn cf_key (key: &CStr) -> CFRetained<CFString> {
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

fn product_name (device: &IOHIDDevice) -> String {
  device
    // `property` returns an `Option` (the key may be absent), so the rest of the
    // chain operates on that optional value rather than on a bare result.
    .property(&cf_key(kIOHIDProductIDKey))
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