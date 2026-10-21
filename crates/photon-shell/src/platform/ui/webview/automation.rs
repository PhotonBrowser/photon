//! BrowserBench hooks used by the authenticated automation endpoint.

use super::PhotonWebView;

impl PhotonWebView {
    pub(in crate::platform) fn probe_speedometer_result(
        &mut self,
        expected_document_generation: u64,
    ) -> bool {
        self.session
            .install_speedometer_result_observer(expected_document_generation)
    }

    pub(in crate::platform) fn probe_jetstream_result(
        &mut self,
        expected_document_generation: u64,
    ) -> bool {
        self.session
            .install_jetstream_result_observer(expected_document_generation)
    }

    pub(in crate::platform) fn request_jetstream_start(
        &mut self,
        expected_document_generation: u64,
    ) -> bool {
        self.session
            .request_jetstream_start(expected_document_generation)
    }

    pub(in crate::platform) fn has_completed_document_load(&self) -> bool {
        self.session.has_completed_document_load()
    }

    pub(in crate::platform) fn document_generation(&self) -> u64 {
        self.session.document_generation()
    }
}
