use std::time::Duration;

use gpui::{
    App, Context, Render, SpringConfig, Task, Window, WindowOptions, div, prelude::*, px, rgb,
};
use gpui_motion::{
    AnimateIn, AnimateSpring, Entrance, MotionConfig, Presence, Speed, SpringSpec, Tween,
};
use gpui_platform::application;

struct MotionGallery {
    visible: bool,
    target: f32,
    tween: Tween,
    presence: Presence<bool>,
    tween_task: Option<Task<()>>,
}

impl MotionGallery {
    fn new() -> Self {
        Self {
            visible: true,
            target: 1.0,
            tween: Tween::at(1.0),
            presence: Presence::default(),
            tween_task: None,
        }
    }
}

impl Render for MotionGallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = MotionConfig::default();
        let entrance = Entrance::new()
            .fade_from(0.0)
            .slide_up(16.0)
            .scale_from(0.96)
            .blur_from(6.0)
            .duration(Duration::from_millis(240));
        let presence = self
            .presence
            .sync(self.visible.then_some(true), entrance, &config, cx);
        let tween_width = 80.0 + self.tween.value() * 100.0;

        let toggle = div()
            .id("toggle-presence")
            .px_4()
            .py_2()
            .rounded_md()
            .bg(rgb(0x334155))
            .text_color(rgb(0xffffff))
            .child(if self.visible {
                "Hide presence item"
            } else {
                "Show presence item"
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.visible = !this.visible;
                cx.notify();
            }));

        let reverse = div()
            .id("reverse-tween")
            .px_4()
            .py_2()
            .rounded_md()
            .bg(rgb(0x334155))
            .text_color(rgb(0xffffff))
            .child("Reverse tween")
            .on_click(cx.listener(move |this, _, _, cx| {
                this.target = if this.target < 0.5 { 1.0 } else { 0.0 };
                this.tween.animate_to(this.target, Speed::Standard, &config);
                this.tween_task = Some(cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                        let Ok(running) = this.update(cx, |view, cx| {
                            cx.notify();
                            view.tween.is_running()
                        }) else {
                            break;
                        };
                        if !running {
                            break;
                        }
                    }
                }));
            }));

        let spring = div()
            .id("spring-example")
            .size(px(36.0))
            .rounded_md()
            .bg(rgb(0x38bdf8))
            .animate_spring(
                "gallery-spring",
                0.0,
                1.0,
                SpringSpec {
                    config: SpringConfig::new(420.0, 32.0, 1.0),
                    epsilon: 0.001,
                },
                |element, progress| element.opacity(progress),
            );

        let mut content = div()
            .id("motion-gallery")
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .bg(rgb(0x0f172a))
            .text_color(rgb(0xe2e8f0))
            .child("GPUI Motion Gallery")
            .child(div().flex().gap_3().child(toggle).child(reverse))
            .child(
                div()
                    .id("tween-track")
                    .w(px(tween_width))
                    .h(px(12.0))
                    .rounded_md()
                    .bg(rgb(0xa78bfa)),
            )
            .child(spring);

        if let Some((_, transition)) = presence {
            let item = div()
                .id("presence-item")
                .p_4()
                .rounded_md()
                .bg(rgb(0x1e293b))
                .child("Fade, slide, scale, and blur; toggle to see exit presence.")
                .animate("presence-item-motion", entrance, transition, &config);
            content = content.child(item);
        }

        content
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.open_window(WindowOptions::new().focus(true), |_, cx| {
            cx.new(|_| MotionGallery::new())
        })
        .unwrap();
        cx.activate(true);
    });
}
