//! Joy-Con HID report parsing

/// Button state from bytes 3 (right), 4 (shared), 5 (left) of a standard
/// input report, packed as `right | shared << 8 | left << 16`.
/// See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#standard-input-report---buttons>
#[derive(Clone, Copy)]
pub struct Buttons(pub u32);

impl Buttons {
    // Byte 3: right Joy-Con
    pub const Y: Buttons = Buttons(0x00_00_01);
    pub const X: Buttons = Buttons(0x00_00_02);
    pub const B: Buttons = Buttons(0x00_00_04);
    pub const A: Buttons = Buttons(0x00_00_08);
    pub const SR: Buttons = Buttons(0x00_00_10);
    pub const SL: Buttons = Buttons(0x00_00_20);
    pub const R: Buttons = Buttons(0x00_00_40);
    pub const ZR: Buttons = Buttons(0x00_00_80);
    // Byte 4: shared
    pub const MINUS: Buttons = Buttons(0x00_01_00);
    pub const PLUS: Buttons = Buttons(0x00_02_00);
    pub const RSTICK: Buttons = Buttons(0x00_04_00);
    pub const HOME: Buttons = Buttons(0x00_10_00);

    pub fn from_bytes(right: u8, shared: u8, left: u8) -> Self {
        Buttons(u32::from(right) | u32::from(shared) << 8 | u32::from(left) << 16)
    }

    /// True if every bit of `other` is set in `self`.
    ///
    /// Example: Y and R are pressed (`0x41`), asking for R alone (`0x40`):
    ///
    /// ```text
    ///   0b0100_0001   self  (Y + R)
    /// & 0b0100_0000   other (R)
    /// = 0b0100_0000   == other → true
    /// ```
    ///
    /// ```
    /// use ukyo_clicker::protocol::Buttons;
    ///
    /// let state = Buttons::from_bytes(0x41, 0, 0); // Y + R
    /// assert!(state.contains(Buttons::R));
    /// assert!(!state.contains(Buttons::A));
    /// ```
    pub fn contains(self, other: Buttons) -> bool {
        self.0 & other.0 == other.0
    }
}
