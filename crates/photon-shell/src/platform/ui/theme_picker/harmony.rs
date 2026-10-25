//! Small, deterministic hue harmonies. Colours remain the persisted source of truth.

use super::*;

const COMPLEMENTARY: &[u16] = &[180];
const SINGLE_ANALOGOUS: &[u16] = &[310];
const TRIADIC: &[u16] = &[120, 240];
const ANALOGOUS: &[u16] = &[50, 310];
const SPLIT_COMPLEMENTARY: &[u16] = &[150, 210];

pub(super) fn analogous_offsets() -> &'static [u16] {
    ANALOGOUS
}

pub(super) fn default_offsets(count: usize) -> &'static [u16] {
    match count {
        2 => COMPLEMENTARY,
        3 => TRIADIC,
        _ => &[],
    }
}

impl PickerState {
    pub(super) fn apply_harmony(&mut self, offsets: &[u16], cx: &App) {
        let primary = self.resolved();
        for (handle, offset) in self
            .handles
            .iter_mut()
            .filter(|h| !h.removing && h.id != self.selected)
            .zip(offsets)
        {
            handle.tint = SpaceTint {
                hue: (primary.hue + offset) % 360,
                ..primary
            };
        }
        self.arrange_followers(true, cx);
        self.dirty_tint = true;
    }

    pub(super) fn cycle_harmony(&mut self, cx: &mut Context<Self>) {
        let colors = self.colors();
        let choices: &[&[u16]] = match colors.colors().len() {
            2 => &[COMPLEMENTARY, SINGLE_ANALOGOUS],
            3 => &[TRIADIC, ANALOGOUS, SPLIT_COMPLEMENTARY],
            _ => return,
        };
        let primary = self.resolved();
        let current = choices.iter().position(|offsets| {
            self.handles
                .iter()
                .filter(|h| !h.removing && h.id != self.selected)
                .zip(offsets.iter())
                .all(|(h, offset)| {
                    let delta =
                        (i32::from(h.tint.hue) - i32::from(primary.hue) - i32::from(*offset) + 180)
                            .rem_euclid(360)
                            - 180;
                    delta.abs() <= 1
                })
        });
        self.apply_harmony(
            choices[current.map_or(0, |index| (index + 1) % choices.len())],
            cx,
        );
        haptics::play(cx, Feedback::StateChange);
        self.flush(cx);
        cx.notify();
    }
}
