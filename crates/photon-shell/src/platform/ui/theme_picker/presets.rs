//! Preset families use the same colour values for painting, selection, and saving.

use super::*;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum PresetPage {
    #[default]
    Colours,
    Harmonies,
    Dark,
    DarkHarmonies,
    Grayscale,
}
impl PresetPage {
    const ALL: [Self; 5] = [
        Self::Colours,
        Self::Harmonies,
        Self::Dark,
        Self::DarkHarmonies,
        Self::Grayscale,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Colours => "Colours",
            Self::Harmonies => "Colour harmonies",
            Self::Dark => "Dark colours",
            Self::DarkHarmonies => "Dark harmonies",
            Self::Grayscale => "Grayscale",
        }
    }
    pub fn next(self, direction: i16) -> Self {
        let index = Self::ALL.iter().position(|page| *page == self).unwrap_or(0);
        Self::ALL[(index as i16 + direction).rem_euclid(Self::ALL.len() as i16) as usize]
    }
    pub fn colors(self, color: WindowColor) -> SpaceColors {
        let mut primary = theme::preset_tint(color);
        if matches!(self, Self::Dark | Self::DarkHarmonies) {
            primary.lightness = (tokens().preset_dark_lightness * 100.0).round() as u8;
        }
        if self == Self::Grayscale {
            let index = WindowColor::ALL
                .iter()
                .position(|preset| *preset == color)
                .unwrap_or(0);
            primary.saturation = 0;
            primary.hue = 0;
            primary.lightness = (index * 100 / (WindowColor::ALL.len() - 1)) as u8;
        }
        if matches!(self, Self::Harmonies | Self::DarkHarmonies) {
            let offsets = super::harmony::analogous_offsets();
            SpaceColors::from_colors(&[
                primary,
                SpaceTint {
                    hue: (primary.hue + offsets[0]) % 360,
                    ..primary
                },
                SpaceTint {
                    hue: (primary.hue + offsets[1]) % 360,
                    ..primary
                },
            ])
        } else {
            SpaceColors::from_colors(&[primary])
        }
    }
}
impl PickerState {
    pub(super) fn choose(&mut self, color: WindowColor, cx: &mut Context<Self>) {
        if self.colors().colors().is_empty() {
            return;
        }
        let chosen = self.preset_page.colors(color);
        let tint = chosen.primary().expect("a preset has a primary colour");
        self.color = color;
        self.set_tint(tint, true, cx);
        let motion = Settings::motion(cx);
        self.handles.retain(|handle| !handle.removing);
        for handle in self
            .handles
            .iter_mut()
            .filter(|h| h.id != self.selected)
            .skip(chosen.colors().len() - 1)
        {
            handle.removing = true;
            handle.scale.animate_to(0.0, Speed::Standard, &motion);
            handle.presence.animate_to(0.0, Speed::Standard, &motion);
        }
        while self.colors().colors().len() < chosen.colors().len() {
            let mut handle = ColorHandle::new(self.next_id, tint, false);
            self.next_id += 1;
            handle.scale.jump_to(0.0);
            handle.presence.jump_to(0.0);
            handle.presence.animate_to(1.0, Speed::Standard, &motion);
            self.handles.push(handle);
        }
        if chosen.colors().len() > 1 {
            self.apply_harmony(super::harmony::analogous_offsets(), cx);
        }
        self.resize_handles(false, cx);
        self.dirty_tint = true;
        haptics::play(cx, Feedback::StateChange);
        self.flush(cx);
        cx.notify();
    }

    pub(super) fn change_preset_page(&mut self, direction: i16, cx: &mut Context<Self>) {
        if self.colors().colors().is_empty() {
            return;
        }
        self.preset_page = self.preset_page.next(direction);
        haptics::play(cx, Feedback::Alignment);
        cx.notify();
    }
}
