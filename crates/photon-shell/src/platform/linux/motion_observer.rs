//! The system reduced-motion preference, for shell animations.
//!
//! Follows GNOME's `enable-animations` setting through `gsettings`, which
//! GNOME and most GTK-based desktops honour. Desktops without it keep
//! GPUI's default.

use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

use futures::{StreamExt, channel::mpsc};
use gpui::App;

const SCHEMA: &str = "org.gnome.desktop.interface";
const KEY: &str = "enable-animations";

/// Keeps GPUI's reduce-motion flag in step with the desktop preference.
pub(crate) fn follow(cx: &mut App) {
    let (sender, mut changes) = mpsc::unbounded();
    let spawned = std::thread::Builder::new()
        .name("photon-reduced-motion".into())
        .spawn(move || watch(sender));
    if spawned.is_err() {
        return;
    }
    cx.spawn(async move |cx| {
        while let Some(reduce_motion) = changes.next().await {
            crate::platform::app_update::update_app(cx, |cx| cx.set_reduce_motion(reduce_motion))
                .await;
        }
    })
    .detach();
}

/// Sends the current preference, then each change until `gsettings` exits.
fn watch(sender: mpsc::UnboundedSender<bool>) {
    let Some(current) = Command::new("gsettings")
        .args(["get", SCHEMA, KEY])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| reduce_motion(&String::from_utf8_lossy(&output.stdout)))
    else {
        return;
    };
    if sender.unbounded_send(current).is_err() {
        return;
    }
    let Ok(mut monitor) = Command::new("gsettings")
        .args(["monitor", SCHEMA, KEY])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let Some(stdout) = monitor.stdout.take() else {
        return;
    };
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let Some(reduce) = reduce_motion(&line) else {
            continue;
        };
        if sender.unbounded_send(reduce).is_err() {
            break;
        }
    }
    let _ = monitor.kill();
}

/// Reads `true`/`false` from `gsettings` output such as
/// `enable-animations: false`; disabled animations mean reduced motion.
fn reduce_motion(output: &str) -> Option<bool> {
    match output.rsplit(':').next()?.trim() {
        "true" => Some(false),
        "false" => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::reduce_motion;

    #[test]
    fn reads_get_and_monitor_output() {
        assert_eq!(reduce_motion("false\n"), Some(true));
        assert_eq!(reduce_motion("true"), Some(false));
        assert_eq!(reduce_motion("enable-animations: false"), Some(true));
        assert_eq!(reduce_motion("No such key"), None);
    }
}
