//! Joy-Con HID report parsing

/// Button state from bytes 3 (right), 4 (shared), 5 (left) of a standard
/// input report, packed as `right | shared << 8 | left << 16`.
/// See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#standard-input-report---buttons>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buttons(pub u32);

impl std::ops::BitOr for Buttons {
    type Output = Buttons;

    fn bitor(self, rhs: Buttons) -> Buttons {
        Buttons(self.0 | rhs.0)
    }
}

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

    /// True if any bit of `other` is set in `self`.
    pub fn intersects(self, other: Buttons) -> bool {
        self.0 & other.0 != 0
    }

    /// Bits set in `self` but not in `earlier`: buttons that just went down.
    pub fn pressed_since(self, earlier: Buttons) -> Buttons {
        Buttons(self.0 & !earlier.0)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum ParseError {
    Empty,
    TooShort { id: u8, len: usize, need: usize },
    Unsupported(u8),
}

/// Bytes 0..=12: id, timer, battery, buttons, sticks, vibrator.
const STANDARD_LEN: usize = 13;
/// Standard header + ACK (13) + echoed subcommand id (14).
const REPLY_LEN: usize = 15;

fn need(buf: &[u8], need: usize) -> Result<(), ParseError> {
    if buf.len() < need {
        return Err(ParseError::TooShort {
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

pub fn parse_input(buf: &[u8]) -> Result<InputReport, ParseError> {
    let id = *buf.first().ok_or(ParseError::Empty)?;
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
        other => Err(ParseError::Unsupported(other)),
    }
}

// ---------- Output reports (host -> Joy-Con) ----------

/// Bluetooth length of an output report, including the report ID byte.
pub const OUTPUT_LEN: usize = 49;
/// Report ID for "rumble + subcommand".
const OUTPUT_RUMBLE_SUBCMD: u8 = 0x01;
/// Neutral rumble (left 4 bytes, right 4 bytes): no vibration.
const NEUTRAL_RUMBLE: [u8; 8] = [0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40];
/// Subcommand ID sits at byte 10, its arguments start at byte 11.
const SUBCMD_ID_AT: usize = 10;
const SUBCMD_ARGS_AT: usize = 11;

pub const SUBCMD_SET_INPUT_MODE: u8 = 0x03;
pub const SUBCMD_SET_PLAYER_LIGHTS: u8 = 0x30;
/// Argument to `SUBCMD_SET_INPUT_MODE`: standard full mode (0x30 reports).
pub const MODE_FULL: u8 = 0x30;

/// Builds `0x01` output reports and owns the packet counter.
/// Not `Clone`: two copies would reuse packet numbers.
#[derive(Debug, Default)]
pub struct SubcommandBuilder {
    counter: u8,
}

impl SubcommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build one output report and advance the counter (0..=15, wraps around like a clock).
    ///
    /// # Panics
    /// If `args` does not fit after byte 11.
    pub fn build(&mut self, subcommand: u8, args: &[u8]) -> [u8; OUTPUT_LEN] {
        assert!(
            args.len() <= OUTPUT_LEN - SUBCMD_ARGS_AT,
            "args too long: {} > {}",
            args.len(),
            OUTPUT_LEN - SUBCMD_ARGS_AT
        );

        let mut buf = [0u8; OUTPUT_LEN];
        buf[0] = OUTPUT_RUMBLE_SUBCMD;
        buf[1] = self.counter;
        buf[2..SUBCMD_ID_AT].copy_from_slice(&NEUTRAL_RUMBLE);
        buf[SUBCMD_ID_AT] = subcommand;
        buf[SUBCMD_ARGS_AT..SUBCMD_ARGS_AT + args.len()].copy_from_slice(args);

        self.counter = (self.counter + 1) & 0x0F;
        buf
    }

    pub fn set_input_mode(&mut self, mode: u8) -> [u8; OUTPUT_LEN] {
        self.build(SUBCMD_SET_INPUT_MODE, &[mode])
    }

    /// Low nibble: LEDs on. High nibble: LEDs flashing.
    pub fn set_player_lights(&mut self, lights: u8) -> [u8; OUTPUT_LEN] {
        self.build(SUBCMD_SET_PLAYER_LIGHTS, &[lights])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 49-byte report with the given id, battery byte and 3 button bytes.
    fn report(id: u8, battery: u8, buttons: [u8; 3]) -> [u8; 49] {
        let mut r = [0u8; 49];
        r[0] = id;
        r[2] = battery;
        r[3..6].copy_from_slice(&buttons);
        r
    }

    fn full(buf: &[u8]) -> Standard {
        match parse_input(buf) {
            Ok(InputReport::Full(s)) => s,
            other => panic!("expected Full, got {other:?}"),
        }
    }

    #[test]
    fn full_report_buttons_and_battery() {
        // 0x41 = Y + R; battery 0x8E = level 8, not charging, Joy-Con
        let s = full(&report(0x30, 0x8E, [0x41, 0x00, 0x00]));
        assert!(s.buttons.contains(Buttons::Y));
        assert!(s.buttons.contains(Buttons::R));
        assert!(!s.buttons.contains(Buttons::A));
        assert_eq!(
            s.battery,
            Battery {
                level: 8,
                charging: false
            }
        );
    }

    #[test]
    fn each_right_button_maps_to_its_bit() {
        let cases = [
            (0x01, Buttons::Y),
            (0x02, Buttons::X),
            (0x04, Buttons::B),
            (0x08, Buttons::A),
            (0x10, Buttons::SR),
            (0x20, Buttons::SL),
            (0x40, Buttons::R),
            (0x80, Buttons::ZR),
        ];
        for (bit, button) in cases {
            let s = full(&report(0x30, 0, [bit, 0, 0]));
            assert_eq!(s.buttons, button, "bit {bit:#04x}");
        }
    }

    #[test]
    fn shared_buttons() {
        let s = full(&report(0x30, 0, [0, 0x12, 0])); // Plus + Home
        assert!(s.buttons.contains(Buttons::PLUS));
        assert!(s.buttons.contains(Buttons::HOME));
        assert!(!s.buttons.contains(Buttons::MINUS));
    }

    #[test]
    fn charging_flag() {
        let s = full(&report(0x30, 0x91, [0; 3])); // high nibble 9 = 8 + charging
        assert_eq!(
            s.battery,
            Battery {
                level: 8,
                charging: true
            }
        );
    }

    #[test]
    fn subcommand_ack_and_nack() {
        // ID x21 is a subcommand reply
        // See: <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#standard-input-report-format>
        let mut r = report(0x21, 0x8E, [0; 3]);
        r[13] = 0x80; // the most significant bit MSB of byte 13 is set to 1, that's ACK
        r[14] = 0x03; // echo of "set input report mode", see <https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_subcommands_notes.md#subcommand-0x03-set-input-report-mode>
        let Ok(InputReport::SubcommandReply {
            ack, subcommand, ..
        }) = parse_input(&r)
        else {
            panic!("expected SubcommandReply");
        };
        assert!(ack);
        assert_eq!(subcommand, 0x03);

        r[13] = 0x00; // the most significant bit MSB of byte 13 is set to 0, that's NACK
        assert!(matches!(
            parse_input(&r),
            Ok(InputReport::SubcommandReply { ack: false, .. })
        ));
    }

    #[test]
    fn errors() {
        assert_eq!(parse_input(&[]), Err(ParseError::Empty));
        assert_eq!(
            parse_input(&[0x30, 0, 0]),
            Err(ParseError::TooShort {
                id: 0x30,
                len: 3,
                need: STANDARD_LEN
            })
        );
        assert_eq!(parse_input(&[0x3F; 12]), Err(ParseError::Unsupported(0x3F)));
    }

    fn expected(counter: u8, subcommand: u8, arg: u8) -> [u8; OUTPUT_LEN] {
        let mut e = [0u8; OUTPUT_LEN];
        e[..12].copy_from_slice(&[
            0x01, counter, // report id, packet counter
            0x00, 0x01, 0x40, 0x40, 0x00, 0x01, 0x40, 0x40, // neutral rumble
            subcommand, arg,
        ]);
        e
    }

    #[test]
    fn set_input_mode_bytes() {
        let mut b = SubcommandBuilder::new();
        assert_eq!(b.set_input_mode(MODE_FULL), expected(0, 0x03, 0x30));
    }

    #[test]
    fn player_lights_after_mode_uses_next_counter() {
        let mut b = SubcommandBuilder::new();
        b.set_input_mode(MODE_FULL);
        assert_eq!(b.set_player_lights(0x01), expected(1, 0x30, 0x01));
    }

    #[test]
    fn counter_wraps_after_15() {
        let mut b = SubcommandBuilder::new();
        for want in (0..16).chain(0..3) {
            assert_eq!(b.build(0x00, &[])[1], want);
        }
    }

    #[test]
    #[should_panic(expected = "args too long")]
    fn too_many_args_panics() {
        SubcommandBuilder::new().build(0x00, &[0; OUTPUT_LEN]);
    }
}
