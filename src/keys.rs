//! Slide keystrokes for the frontmost app (presenterm: Right = next, Left = prev).

use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFBoolean, CFDictionary};
use objc2_core_graphics::{CGEvent, CGEventTapLocation, CGKeyCode};

use crate::mapper::Action;

/// macOS virtual key codes (Carbon `kVK_RightArrow` / `kVK_LeftArrow`).
const KEY_RIGHT_ARROW: CGKeyCode = 0x7C;
const KEY_LEFT_ARROW: CGKeyCode = 0x7B;

/// Anything that can perform a slide action.
pub trait KeySender {
    fn send(&mut self, action: Action);
}

pub fn keycode(action: Action) -> CGKeyCode {
    match action {
        Action::Next => KEY_RIGHT_ARROW,
        Action::Prev => KEY_LEFT_ARROW,
    }
}

/// Posts real key events via Quartz. Needs Accessibility permission.
pub struct CGEventSender;

impl KeySender for CGEventSender {
    fn send(&mut self, action: Action) {
        let key = keycode(action);
        // A key press is two events: down, then up.
        for key_down in [true, false] {
            match CGEvent::new_keyboard_event(None, key, key_down) {
                Some(event) => CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event)),
                None => eprintln!("could not create key event for {action:?}"),
            }
        }
    }
}

/// Sends nothing. Used by `--dump`.
pub struct DryRun;

impl KeySender for DryRun {
    fn send(&mut self, _action: Action) {}
}

/// Whether this process may post events. With `prompt`, macOS shows its
/// "would like to control this computer" dialog if permission is missing.
pub fn accessibility_trusted(prompt: bool) -> bool {
    // SAFETY: Apple-defined global CFString, valid for the whole program.
    let key = unsafe { kAXTrustedCheckOptionPrompt };
    let options = CFDictionary::from_slices(&[key], &[CFBoolean::new(prompt)]);
    // SAFETY: the dictionary maps CFString -> CFBoolean, as the API documents.
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_is_right_arrow_prev_is_left_arrow() {
        assert_eq!(keycode(Action::Next), 0x7C);
        assert_eq!(keycode(Action::Prev), 0x7B);
    }
}
