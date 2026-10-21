//! Turns button state into slide actions. Pure: no I/O.

use crate::protocol::Buttons;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Next,
    Prev,
}

/// Edge detector: fires once per press, never on hold or release.
#[derive(Debug)]
pub struct Mapper {
    next: Buttons,
    prev: Buttons,
    last: Buttons,
}

impl Mapper {
    pub fn new(next: Buttons, prev: Buttons) -> Self {
        Mapper { next, prev, last: Buttons::default() }
    }

    /// Feed the latest button state; returns the actions it triggers.
    pub fn update(&mut self, now: Buttons) -> Vec<Action> {
        let pressed = now.pressed_since(self.last);
        self.last = now;

        let mut actions = Vec::new();
        if pressed.intersects(self.next) {
            actions.push(Action::Next);
        }
        if pressed.intersects(self.prev) {
            actions.push(Action::Prev);
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::{Next, Prev};

    fn mapper() -> Mapper {
        Mapper::new(Buttons::A | Buttons::R, Buttons::B)
    }

    #[test]
    fn press_fires_once_hold_and_release_do_not() {
        let mut m = mapper();
        assert_eq!(m.update(Buttons::A), [Next]); // press
        assert_eq!(m.update(Buttons::A), []); // hold
        assert_eq!(m.update(Buttons::default()), []); // release
        assert_eq!(m.update(Buttons::A), [Next]); // press again
    }

    #[test]
    fn two_next_buttons_together_fire_one_next() {
        let mut m = mapper();
        assert_eq!(m.update(Buttons::A | Buttons::R), [Next]);
    }

    #[test]
    fn pressing_while_another_is_held() {
        let mut m = mapper();
        m.update(Buttons::B);
        assert_eq!(m.update(Buttons::B | Buttons::A), [Next]);
    }

    #[test]
    fn next_and_prev_in_same_report() {
        let mut m = mapper();
        assert_eq!(m.update(Buttons::A | Buttons::B), [Next, Prev]);
    }

    #[test]
    fn unmapped_buttons_do_nothing() {
        let mut m = mapper();
        assert_eq!(m.update(Buttons::X | Buttons::HOME), []);
    }
}
