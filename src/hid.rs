//! Joy-Con discovery via IOKit's IOHIDManager (macOS only).

use std::ffi::{CStr, c_void};
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use objc2_core_foundation::{
    CFDictionary, CFIndex, CFNumber, CFRetained, CFRunLoop, CFString, Type, kCFRunLoopDefaultMode,
};
use objc2_io_kit::{
    IOHIDDevice, IOHIDManager, IOHIDReportType, IOReturn, kIOHIDOptionsTypeNone,
    kIOHIDOptionsTypeSeizeDevice, kIOHIDProductIDKey, kIOHIDProductKey, kIOHIDVendorIDKey,
    kIOReturnSuccess,
};

use crate::protocol::{
    InputReport, MODE_FULL, OUTPUT_LEN, SUBCMD_SET_INPUT_MODE, SubcommandBuilder, parse_input,
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

/// Report ID of the simple HID mode macOS leaves the Joy-Con in.
const REPORT_SIMPLE: u32 = 0x3F;
/// Player-lights argument: LED 1 on, nothing flashing.
const PLAYER_1: u8 = 0x01;
/// Don't re-send set-mode more often than this.
const MODE_RETRY: Duration = Duration::from_secs(1);

/// Per-device state. Lives on the heap; IOKit gets a raw pointer to it.
struct JoyCon {
    device: CFRetained<IOHIDDevice>,
    builder: SubcommandBuilder,
    last_mode_request: Option<Instant>,
    handler: ReportHandler,
    /// True between sending set-mode and receiving its ACK.
    awaiting_mode_ack: bool,
}

impl JoyCon {
    /// Send one output report synchronously and log the result.
    fn send(&self, mut report: [u8; OUTPUT_LEN], what: &str) {
        let report_id = CFIndex::from(report[0]);
        let ptr = NonNull::from(&mut report).cast::<u8>();
        // SAFETY: `ptr` is valid for OUTPUT_LEN bytes for this whole
        // (synchronous) call; `device` is retained by `self`.
        let ret = unsafe {
            self.device.set_report(
                IOHIDReportType::Output,
                report_id,
                ptr,
                OUTPUT_LEN as CFIndex,
            )
        };
        if ret == kIOReturnSuccess {
            println!("sent: {what}");
        } else {
            eprintln!("send {what} failed: {ret:#x}");
        }
    }

    fn on_report(&mut self, report_id: u32, bytes: &[u8]) {
        if report_id == REPORT_SIMPLE {
            self.request_full_mode();
            return;
        }
        // Our set-mode was ACKed: now set the LED.
        if let Ok(InputReport::SubcommandReply {
            ack: true,
            subcommand: SUBCMD_SET_INPUT_MODE,
            ..
        }) = parse_input(bytes)
            && self.awaiting_mode_ack
        {
            self.awaiting_mode_ack = false;
            let report = self.builder.set_player_lights(PLAYER_1);
            self.send(report, "player lights 1");
        }
    }

    /// Ask for 0x30 mode, at most once per MODE_RETRY.
    fn request_full_mode(&mut self) {
        let due = self
            .last_mode_request
            .is_none_or(|t| t.elapsed() >= MODE_RETRY);
        if !due {
            return;
        }
        self.last_mode_request = Some(Instant::now());
        self.awaiting_mode_ack = true;
        let report = self.builder.set_input_mode(MODE_FULL);
        self.send(report, "set input mode 0x30");
    }
}

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

    // SAFETY: `context` is the `*mut ReportHandler` created in `run`.
    let handler = unsafe { *context.cast::<ReportHandler>() };
    let joycon = Box::new(JoyCon {
        device: device.retain(),
        builder: SubcommandBuilder::new(),
        last_mode_request: None,
        handler,
        awaiting_mode_ack: false,
    });
    // Both leaked on purpose for now (freed on disconnect in Task 9).
    // Kept as two allocations: IOKit writes `buf` while we hold `&mut JoyCon`.
    let joycon = Box::into_raw(joycon).cast::<c_void>();
    let buf: &'static mut [u8; REPORT_BUF_LEN] = Box::leak(Box::new([0; REPORT_BUF_LEN]));
    let ptr = NonNull::from(buf).cast::<u8>();

    // SAFETY: `ptr` is valid for REPORT_BUF_LEN bytes forever; `joycon`
    // points to a leaked JoyCon, also alive forever.
    unsafe {
        device.register_input_report_callback(
            ptr,
            REPORT_BUF_LEN as CFIndex,
            Some(on_report),
            joycon,
        );
    }
}

/// Called by IOKit for each input report.
unsafe extern "C-unwind" fn on_report(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    _type: IOHIDReportType,
    report_id: u32,
    report: NonNull<u8>,
    len: CFIndex,
) {
    let len = usize::try_from(len).unwrap_or(0);
    // SAFETY: IOKit wrote `len` valid bytes at `report` (the leaked buffer).
    let bytes = unsafe { std::slice::from_raw_parts(report.as_ptr(), len) };
    // SAFETY: `context` is the leaked JoyCon from `on_match`. Callbacks all
    // run on the one run loop thread, so no other `&mut` to it exists.
    let joycon = unsafe { &mut *context.cast::<JoyCon>() };
    (joycon.handler)(report_id, bytes);
    joycon.on_report(report_id, bytes);
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
pub fn run(handler: ReportHandler) -> Result<(), IOReturn> {
    let manager = IOHIDManager::new(None, kIOHIDOptionsTypeNone);
    let matching = matching_dict();
    let run_loop = CFRunLoop::current().expect("thread has a run loop");
    // SAFETY: Apple guarantees this global exists.
    let mode = unsafe { kCFRunLoopDefaultMode }.expect("default mode exists");
    // Put the handler on the heap and hand IOKit a raw pointer to it.
    // Leaked on purpose: callbacks may use it for the whole program.
    let context = Box::into_raw(Box::new(handler)).cast::<c_void>();

    // SAFETY: dict holds CFString keys / CFNumber values as IOKit expects;
    // callbacks match IOHIDDeviceCallback and ignore the (null) context.
    unsafe {
        manager.set_device_matching(Some(matching.as_opaque()));
        manager.register_device_matching_callback(Some(on_match), context);
        manager.register_device_removal_callback(Some(on_remove), std::ptr::null_mut());
        manager.schedule_with_run_loop(&run_loop, mode);
    }

    // Seize: exclusive access, so macOS's game-controller stack can't
    // reset the input mode. Side effect: games can't use the Joy-Con
    // while ukyo-clicker runs.
    let ret = manager.open(kIOHIDOptionsTypeSeizeDevice);
    if ret != kIOReturnSuccess {
        return Err(ret);
    }

    println!("waiting for Joy-Con (R)... Ctrl-C to quit");
    CFRunLoop::run(); // blocks; callbacks fire from in here
    Ok(())
}
