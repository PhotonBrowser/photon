//! A page's JavaScript dialogs.

use gpui::Context;
use photon_core::{DialogReply, DialogRequest};

use super::super::super::trace;
use super::PhotonWebView;

impl PhotonWebView {
    /// Shows a page's dialog, or answers at once when it cannot be shown.
    pub(in crate::platform) fn request_dialog(
        &mut self,
        request: DialogRequest,
        cx: &mut Context<Self>,
    ) {
        let kind = request.kind.clone();
        match self.dialogs.request(request) {
            Some(reply) => {
                trace(format_args!(
                    "dialog {kind:?} answered without showing: {reply:?}"
                ));
                self.session.reply_dialog(reply);
            }
            None => {
                trace(format_args!("dialog {kind:?} shown"));
                self.state_changed(cx);
            }
        }
    }

    /// Closes the open dialog with OK (using `text` for a prompt) or Cancel.
    pub(in super::super) fn close_dialog(
        &mut self,
        accepted: bool,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let reply = if accepted {
            self.dialogs.accept(text)
        } else {
            self.dialogs.dismiss()
        };
        self.send_dialog_reply(reply, cx);
    }

    pub(super) fn send_dialog_reply(&mut self, reply: Option<DialogReply>, cx: &mut Context<Self>) {
        if let Some(reply) = reply {
            self.session.reply_dialog(reply);
            self.state_changed(cx);
        }
    }
}
