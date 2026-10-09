//! JavaScript `alert`, `confirm` and `prompt` dialogs for one page.
//!
//! The page waits for an answer to every dialog it opens. These rules decide
//! when a dialog is shown and what the page receives when it is closed or
//! cannot be shown, following Ladybird's own browser UI.

/// The kind of dialog a page asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogKind {
    Alert,
    Confirm,
    Prompt { default_text: String },
}

/// A dialog a page asked to show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogRequest {
    pub kind: DialogKind,
    /// Who is asking: the page's origin, or its scheme when it has none.
    pub title: String,
    pub message: String,
}

/// The answer returned to the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogReply {
    Alert,
    Confirm(bool),
    /// The entered text, or `None` when the prompt was cancelled.
    Prompt(Option<String>),
}

impl DialogKind {
    /// The reply for accepting with OK, using `text` for a prompt.
    pub fn accepted(&self, text: String) -> DialogReply {
        match self {
            Self::Alert => DialogReply::Alert,
            Self::Confirm => DialogReply::Confirm(true),
            Self::Prompt { .. } => DialogReply::Prompt(Some(text)),
        }
    }

    /// The reply for Cancel, Escape, or a dialog that is never shown.
    pub fn dismissed(&self) -> DialogReply {
        match self {
            Self::Alert => DialogReply::Alert,
            Self::Confirm => DialogReply::Confirm(false),
            Self::Prompt { .. } => DialogReply::Prompt(None),
        }
    }
}

/// The dialog a page has open, and whether its dialogs are suppressed.
#[derive(Clone, Debug, Default)]
pub struct PageDialogs {
    open: Option<DialogRequest>,
    /// Set when the browser navigates away, so the outgoing page cannot hold
    /// the navigation up with more dialogs; cleared when a page commits.
    suppressed: bool,
}

impl PageDialogs {
    /// The dialog to show, if any.
    pub fn open(&self) -> Option<&DialogRequest> {
        self.open.as_ref()
    }

    /// Records a page's request. Returns the reply to send at once when the
    /// dialog cannot be shown, or `None` when it is now open.
    pub fn request(&mut self, request: DialogRequest) -> Option<DialogReply> {
        if self.suppressed || self.open.is_some() {
            return Some(request.kind.dismissed());
        }
        self.open = Some(request);
        None
    }

    /// Closes the open dialog with OK, returning the reply for the page.
    pub fn accept(&mut self, text: String) -> Option<DialogReply> {
        self.open.take().map(|request| request.kind.accepted(text))
    }

    /// Closes the open dialog with Cancel, returning the reply for the page.
    pub fn dismiss(&mut self) -> Option<DialogReply> {
        self.open.take().map(|request| request.kind.dismissed())
    }

    /// The browser is starting a navigation: dismisses the open dialog and
    /// suppresses new ones until the next page commits.
    pub fn navigation_started(&mut self) -> Option<DialogReply> {
        self.suppressed = true;
        self.dismiss()
    }

    /// A page committed or finished loading, so it may open dialogs again.
    pub fn navigation_committed(&mut self) {
        self.suppressed = false;
    }

    /// The page process was replaced, so its requests no longer need replies.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: DialogKind) -> DialogRequest {
        DialogRequest {
            kind,
            title: "https://example.com".into(),
            message: "Hello".into(),
        }
    }

    fn prompt() -> DialogKind {
        DialogKind::Prompt {
            default_text: "default".into(),
        }
    }

    #[test]
    fn accepting_answers_by_kind() {
        let mut dialogs = PageDialogs::default();
        assert_eq!(dialogs.request(request(prompt())), None);
        assert_eq!(dialogs.open().unwrap().message, "Hello");
        assert_eq!(
            dialogs.accept("typed".into()),
            Some(DialogReply::Prompt(Some("typed".into())))
        );
        assert!(dialogs.open().is_none());

        dialogs.request(request(DialogKind::Confirm));
        assert_eq!(
            dialogs.accept(String::new()),
            Some(DialogReply::Confirm(true))
        );
    }

    #[test]
    fn dismissing_answers_by_kind() {
        let mut dialogs = PageDialogs::default();
        dialogs.request(request(DialogKind::Alert));
        assert_eq!(dialogs.dismiss(), Some(DialogReply::Alert));
        dialogs.request(request(DialogKind::Confirm));
        assert_eq!(dialogs.dismiss(), Some(DialogReply::Confirm(false)));
        dialogs.request(request(prompt()));
        assert_eq!(dialogs.dismiss(), Some(DialogReply::Prompt(None)));
        assert_eq!(dialogs.dismiss(), None);
    }

    #[test]
    fn navigation_dismisses_and_suppresses_until_commit() {
        let mut dialogs = PageDialogs::default();
        dialogs.request(request(DialogKind::Confirm));
        assert_eq!(
            dialogs.navigation_started(),
            Some(DialogReply::Confirm(false))
        );
        assert_eq!(
            dialogs.request(request(prompt())),
            Some(DialogReply::Prompt(None))
        );
        assert!(dialogs.open().is_none());

        dialogs.navigation_committed();
        assert_eq!(dialogs.request(request(DialogKind::Alert)), None);
    }

    #[test]
    fn a_second_request_is_dismissed_while_one_is_open() {
        let mut dialogs = PageDialogs::default();
        dialogs.request(request(DialogKind::Alert));
        assert_eq!(
            dialogs.request(request(DialogKind::Confirm)),
            Some(DialogReply::Confirm(false))
        );
        assert_eq!(dialogs.open().unwrap().kind, DialogKind::Alert);
    }

    #[test]
    fn reset_forgets_everything() {
        let mut dialogs = PageDialogs::default();
        dialogs.request(request(DialogKind::Alert));
        dialogs.navigation_started();
        dialogs.reset();
        assert!(dialogs.open().is_none());
        assert_eq!(dialogs.request(request(DialogKind::Alert)), None);
    }
}
