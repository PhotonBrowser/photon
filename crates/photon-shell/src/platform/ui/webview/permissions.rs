//! Engine permission requests crossing into the window permission coordinator.

use gpui::Context;

use crate::platform::permission_coordinator::PermissionRequest;

use super::PhotonWebView;

impl PhotonWebView {
    pub(in crate::platform) fn answer_permission_request(
        &mut self,
        request_id: u64,
        document_id: &str,
        granted: bool,
    ) {
        self.session
            .permission_request_completed(request_id, document_id, granted);
    }

    pub(in crate::platform) fn request_permission_prompt(
        &mut self,
        request: PermissionRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(window) = self.window else {
            self.answer_permission_request(request.engine_request_id, &request.document_id, false);
            return;
        };
        let weak_view = cx.entity().downgrade();
        let Some(browser) = window.downcast::<crate::platform::ui::window::BrowserWindow>() else {
            self.answer_permission_request(request.engine_request_id, &request.document_id, false);
            return;
        };
        let queued = browser.update(cx, |browser, window, cx| {
            browser.queue_permission_request(request.clone(), weak_view, window, cx)
        });
        if !queued.unwrap_or(false) {
            self.answer_permission_request(request.engine_request_id, &request.document_id, false);
        }
    }

    pub(in crate::platform) fn cancel_permission_prompt(
        &mut self,
        request_id: u64,
        document_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(window) = self.window else { return };
        let view_id = self.session.permission_view_id();
        let document_id = document_id.to_owned();
        let Some(browser) = window.downcast::<crate::platform::ui::window::BrowserWindow>() else {
            return;
        };
        browser
            .update(cx, |browser, window, cx| {
                browser.cancel_permission_request(view_id, request_id, &document_id, window, cx);
            })
            .ok();
    }
}
