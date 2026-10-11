//! Joy-Con HID report parsing

/// Button state from bytes 3 (right), 4 (shared), 5 (left) of a standard
/// input report, packed as `right | shared << 8 | left << 16`.
/// See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#standard-input-report---buttons>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    /// Packs the three button bytes into one `u32` so any button can be
    /// tested with a single mask. Each byte is widened to `u32`, then shifted
    /// into its own 8-bit lane:
    ///
    /// ```text
    /// bits 23..16  15..8   7..0
    ///      left    shared  right
    /// ```
    ///
    /// The lanes never overlap, so `|` just places them side by side.
    /// Example with the doc's sample bytes `41 00 82`:
    /// `0x41 | 0x00 << 8 | 0x82 << 16 = 0x82_00_41`.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    /// 0 = empty, 2 = critical, 4 = low, 6 = medium, 8 = full.
    /// See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#standard-input-report-format>
    pub level: u8,
    pub charging: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standard {
    pub timer: u8,
    pub buttons: Buttons,
    pub battery: Battery,
}

#[derive(Debug)]
pub enum InputReport {
    /// 0x30: full mode, pushed at 60 Hz
    /// See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#input-0x30>
    Full(Standard),
    /// 0x21: the reply to a subcommand we sent.
    /// See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#standard-input-report-format>
    SubcommandReply {
        standard: Standard,
        ack: bool,
        subcommand: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserError {
    Empty,
    TooShort { id: u8, len: usize, need: usize },
    Unsupported(u8),
}

/// Bytes 0..=12: id, timer, battery, buttons, sticks, vibrator.
const STANDARD_LEN: usize = 13;
/// Standard header + ACK (13) + echoed subcommand id (14).
const REPLY_LEN: usize = 15;

fn need(buf: &[u8], need: usize) -> Result<(), ParserError> {
    if buf.len() < need {
        return Err(ParserError::TooShort {
            id: buf[0],
            len: buf.len(),
            need,
        });
    }
    Ok(())
}

/// Caller must have checked `buf.len() >= STANDARD_LEN`.
fn parse_standard(buf: &[u8]) -> Standard {
    // Byte 2 holds two 4-bit fields:
    //
    //   bit  7 6 5 4 | 3 2 1 0
    //        L L L C | connection info (ignored)
    //
    // `>> 4` drops the low nibble, leaving `0b0000_LLLC` in `hi`.
    // - `hi & 0x0E` (0b1110) keeps the level bits in place. Since the LSB is
    //   masked off, the level is always even (0, 2, 4, 6, 8), matching the
    //   documented values without needing another shift.
    // - `hi & 0x01` isolates the LSB, which is the charging flag.
    //
    // Example: byte 2 = 0x91 → hi = 0x9 = 0b1001 → level 8 (full), charging.
    let hi = buf[2] >> 4;
    Standard {
        timer: buf[1],
        buttons: Buttons::from_bytes(buf[3], buf[4], buf[5]),
        battery: Battery {
            level: hi & 0x0E,
            charging: hi & 0x01 != 0,
        },
    }
}

pub fn parse_input(buf: &[u8]) -> Result<InputReport, ParserError> {
    let id = *buf.first().ok_or(ParserError::Empty)?;
    match id {
        0x30 => {
            need(buf, STANDARD_LEN)?;
            Ok(InputReport::Full(parse_standard(buf)))
        }
        0x21 => {
            need(buf, REPLY_LEN)?;
            Ok(InputReport::SubcommandReply {
                standard: parse_standard(buf),
                // Byte 13: MSB (0x80) set = ACK, clear = NACK. The low 7
                // bits carry the reply data type (e.g. 0x82 = ACK + type 0x02),
                // which we don't need here.
                ack: buf[13] & 0x80 != 0,
                subcommand: buf[14],
            })
        }
        other => Err(ParserError::Unsupported(other)),
    }
}
