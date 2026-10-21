//! End-to-end without hardware: raw report bytes -> parser -> mapper -> sender.

use ukyo_clicker::config::Config;
use ukyo_clicker::keys::KeySender;
use ukyo_clicker::mapper::{Action, Mapper};
use ukyo_clicker::protocol::{InputReport, parse_input};

/// Fake sender: remembers what it was asked to do.
#[derive(Default)]
struct Recorder {
    sent: Vec<Action>,
}

impl KeySender for Recorder {
    fn send(&mut self, action: Action) {
        self.sent.push(action);
    }
}

/// A 0x30 report with the given right-side button byte.
fn full_report(right_buttons: u8) -> [u8; 49] {
    let mut r = [0u8; 49];
    r[0] = 0x30;
    r[2] = 0x8E;
    r[3] = right_buttons;
    r
}

fn feed(reports: &[[u8; 49]], sender: &mut dyn KeySender) {
    let config = Config::default();
    let mut mapper = Mapper::new(config.next, config.prev);
    for bytes in reports {
        if let Ok(InputReport::Full(state)) = parse_input(bytes) {
            for action in mapper.update(state.buttons) {
                sender.send(action);
            }
        }
    }
}

#[test]
fn held_a_then_b_gives_one_next_one_prev() {
    const A: u8 = 0x08;
    const B: u8 = 0x04;
    let reports = [
        full_report(0),
        full_report(A), // press A ...
        full_report(A), // ... held for a few reports
        full_report(A),
        full_report(0), // release
        full_report(B), // press B
        full_report(0),
    ];
    let mut recorder = Recorder::default();
    feed(&reports, &mut recorder);
    assert_eq!(recorder.sent, [Action::Next, Action::Prev]);
}
