//! Reusable, in-memory permission request lifecycle for the Photon shell.
//!
//! This coordinator records request context and enforces single resolution.
//! Presentation and Engine responses are handled by the window and embedder
//! adapters respectively.

use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PermissionRequest {
    pub(super) view_id: u64,
    pub(super) engine_request_id: u64,
    pub(super) document_id: String,
    pub(super) requesting_origin: String,
    pub(super) top_level_origin: String,
    pub(super) capability: String,
}

#[derive(Default)]
pub(super) struct PermissionCoordinator {
    next_id: u64,
    pending: HashMap<u64, PermissionRequest>,
}

impl PermissionCoordinator {
    pub(super) fn begin(&mut self, request: PermissionRequest) -> Option<u64> {
        if request.capability != "microphone"
            || request.document_id.is_empty()
            || request.requesting_origin.is_empty()
            || request.top_level_origin.is_empty()
            || self.pending.values().any(|pending| pending == &request)
        {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        while self.pending.contains_key(&self.next_id) {
            self.next_id = self.next_id.wrapping_add(1).max(1);
        }
        let id = self.next_id;
        self.pending.insert(id, request);
        Some(id)
    }

    pub(super) fn get(&self, id: u64) -> Option<&PermissionRequest> {
        self.pending.get(&id)
    }

    pub(super) fn resolve(&mut self, id: u64) -> Option<PermissionRequest> {
        self.pending.remove(&id)
    }

    pub(super) fn cancel_request(
        &mut self,
        view_id: u64,
        engine_request_id: u64,
        document_id: &str,
    ) -> Option<(u64, PermissionRequest)> {
        let token = self.pending.iter().find_map(|(&token, request)| {
            (request.view_id == view_id
                && request.engine_request_id == engine_request_id
                && request.document_id == document_id)
                .then_some(token)
        })?;
        self.resolve(token).map(|request| (token, request))
    }

    pub(super) fn cancel_view(&mut self, view_id: u64) -> Vec<(u64, PermissionRequest)> {
        let tokens: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(&token, request)| (request.view_id == view_id).then_some(token))
            .collect();
        tokens
            .into_iter()
            .filter_map(|token| self.resolve(token).map(|request| (token, request)))
            .collect()
    }

    pub(super) fn cancel_all(&mut self) -> Vec<(u64, PermissionRequest)> {
        self.pending.drain().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionCoordinator, PermissionRequest};

    fn request(view_id: u64, engine_request_id: u64, document_id: &str) -> PermissionRequest {
        PermissionRequest {
            view_id,
            engine_request_id,
            document_id: document_id.into(),
            requesting_origin: "https://voice.example.net".into(),
            top_level_origin: "https://example.com".into(),
            capability: "microphone".into(),
        }
    }

    #[test]
    fn preserves_both_origins_for_cross_origin_requests() {
        let mut coordinator = PermissionCoordinator::default();
        let token = coordinator.begin(request(4, 9, "frame-document")).unwrap();
        let pending = coordinator.get(token).unwrap();

        assert_eq!(pending.requesting_origin, "https://voice.example.net");
        assert_eq!(pending.top_level_origin, "https://example.com");
    }

    #[test]
    fn retains_same_origin_request_context_without_creating_a_second_origin() {
        let mut coordinator = PermissionCoordinator::default();
        let mut same_origin = request(4, 10, "main-document");
        same_origin.requesting_origin = "https://example.com".into();
        same_origin.top_level_origin = "https://example.com".into();
        let token = coordinator.begin(same_origin).unwrap();
        let pending = coordinator.get(token).unwrap();

        assert_eq!(pending.requesting_origin, "https://example.com");
        assert_eq!(pending.top_level_origin, pending.requesting_origin);
    }

    #[test]
    fn duplicate_decisions_and_stale_document_cancellations_are_ignored() {
        let mut coordinator = PermissionCoordinator::default();
        let token = coordinator.begin(request(4, 9, "document-a")).unwrap();

        assert!(coordinator.cancel_request(4, 9, "document-b").is_none());
        assert!(coordinator.resolve(token).is_some());
        assert!(coordinator.resolve(token).is_none());
    }

    #[test]
    fn invalidation_dismisses_the_request_before_a_racing_allow_can_resolve() {
        let mut coordinator = PermissionCoordinator::default();
        let token = coordinator.begin(request(4, 11, "child-document")).unwrap();

        let (_, dismissed) = coordinator
            .cancel_request(4, 11, "child-document")
            .expect("the matching prompt must be dismissed");
        assert_eq!(dismissed.document_id, "child-document");
        assert!(coordinator.resolve(token).is_none());
    }

    #[test]
    fn navigation_or_view_closure_cancels_only_matching_requests() {
        let mut coordinator = PermissionCoordinator::default();
        let first = coordinator.begin(request(4, 9, "document-a")).unwrap();
        let second = coordinator.begin(request(5, 9, "document-b")).unwrap();

        let cancelled = coordinator.cancel_view(4);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].0, first);
        assert!(coordinator.get(second).is_some());
    }

    #[test]
    fn window_teardown_clears_every_pending_permission_prompt() {
        let mut coordinator = PermissionCoordinator::default();
        coordinator.begin(request(4, 9, "document-a")).unwrap();
        coordinator.begin(request(5, 10, "document-b")).unwrap();

        assert_eq!(coordinator.cancel_all().len(), 2);
        assert!(coordinator.cancel_all().is_empty());
    }

    #[test]
    fn rejects_unsupported_capabilities_and_duplicate_requests() {
        let mut coordinator = PermissionCoordinator::default();
        let original = request(4, 9, "document-a");
        assert!(coordinator.begin(original.clone()).is_some());
        assert!(coordinator.begin(original).is_none());

        let mut unsupported = request(4, 10, "document-b");
        unsupported.capability = "camera".into();
        assert!(coordinator.begin(unsupported).is_none());
    }
}
