use super::*;
use crate::platform::ui::{appearance_preview::AppearancePreview, theme};
use gpui::{Context, Render, Subscription, Window, div};
use photon_core::{SpaceColors, SpaceTint, ThemeMode, WindowColor};
use std::{cell::Cell, rc::Rc};

struct PageBody {
    renders: Rc<Cell<usize>>,
    _settings: Subscription,
}
impl Render for PageBody {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full()
    }
}

#[gpui::test]
fn preview_frames_repaint_the_surface_without_rebuilding_the_page(cx: &mut gpui::TestAppContext) {
    let tint = SpaceTint {
        hue: 0,
        saturation: 40,
        lightness: 15,
    };
    let mut settings = BrowserSettings {
        theme: ThemeMode::Dark,
        transparency_percent: Some(50),
        ..Default::default()
    };
    let space = settings.spaces.active().id;
    settings
        .spaces
        .get_mut(space)
        .unwrap()
        .set_colors(SpaceColors::from_colors(&[tint]));
    cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
    let renders = Rc::new(Cell::new(0));
    let counter = renders.clone();
    let (frame, cx) = cx.add_window_view(|_, cx| {
        let body = cx.new(|cx| PageBody {
            renders: counter,
            _settings: cx.observe_global::<Settings>(|_, cx| cx.notify()),
        });
        PageFrame::new(body.into(), cx)
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let initial = renders.get();
    assert!(initial > 0);
    let owner = frame.entity_id();
    for hue in 0..60 {
        cx.update(|_, cx| {
            AppearancePreview {
                owner: Some(owner),
                tint: Some((
                    space,
                    WindowColor::Blue,
                    SpaceColors::from_colors(&[SpaceTint { hue, ..tint }]),
                )),
                transparency: Some(50 + hue as u8 / 2),
                ..Default::default()
            }
            .publish(cx)
        });
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    assert_eq!(
        renders.get(),
        initial,
        "live tint/opacity must not rebuild the settings form"
    );
    cx.update(|_, cx| {
        AppearancePreview {
            owner: Some(owner),
            tint: Some((
                space,
                WindowColor::Blue,
                SpaceColors::from_colors(&[SpaceTint {
                    lightness: 95,
                    ..tint
                }]),
            )),
            transparency: Some(50),
            ..Default::default()
        }
        .publish(cx)
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.update(|window, cx| {
        assert_eq!(
            frame.read(cx).foreground,
            Some(theme::page_palette(window, cx).text_primary)
        )
    });
    assert!(
        renders.get() > initial,
        "a contrast change must update page text"
    );
}
