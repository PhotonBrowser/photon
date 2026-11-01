//! JavaScript `alert`, `confirm` and `prompt` dialogs for one page, the
//! sign-in dialog a server asks for with HTTP authentication, and the question
//! before a link opens in another app.
//!
//! The page waits for an answer to every dialog it opens. These rules decide
//! when a dialog is shown and what the page receives when it is closed or
//! cannot be shown, following Ladybird's own browser UI.

/// The kind of dialog a page asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogKind {
    Alert,
    Confirm,
    Prompt {
        default_text: String,
    },
    /// A server asked for a username and password.
    SignIn {
        request_id: u64,
    },
    /// A page asked to open a link, such as `mailto:`, in another app.
    OpenInApp {
        request_id: u64,
    },
}

/// What the user entered when accepting a dialog.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DialogInput {
    /// A prompt's response, or a sign-in's username.
    pub text: String,
    /// A sign-in's password.
    pub password: String,
}

/// A username and password entered to sign in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
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
    /// The entered credentials, or `None` when signing in was cancelled.
    SignIn {
        request_id: u64,
        credentials: Option<Credentials>,
    },
    /// Whether to open the link in its app.
    OpenInApp {
        request_id: u64,
        open: bool,
    },
}

impl DialogKind {
    /// The reply for accepting with OK, using what was entered.
    pub fn accepted(&self, input: DialogInput) -> DialogReply {
        match self {
            Self::Alert => DialogReply::Alert,
            Self::Confirm => DialogReply::Confirm(true),
            Self::Prompt { .. } => DialogReply::Prompt(Some(input.text)),
            Self::SignIn { request_id } => DialogReply::SignIn {
                request_id: *request_id,
                credentials: Some(Credentials {
                    username: input.text,
                    password: input.password,
                }),
            },
            Self::OpenInApp { request_id } => DialogReply::OpenInApp {
                request_id: *request_id,
                open: true,
            },
        }
    }

    /// The reply for Cancel, Escape, or a dialog that is never shown.
    pub fn dismissed(&self) -> DialogReply {
        match self {
            Self::Alert => DialogReply::Alert,
            Self::Confirm => DialogReply::Confirm(false),
            Self::Prompt { .. } => DialogReply::Prompt(None),
            Self::SignIn { request_id } => DialogReply::SignIn {
                request_id: *request_id,
                credentials: None,
            },
            Self::OpenInApp { request_id } => DialogReply::OpenInApp {
                request_id: *request_id,
                open: false,
            },
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
    pub fn accept(&mut self, input: DialogInput) -> Option<DialogReply> {
        self.open.take().map(|request| request.kind.accepted(input))
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

    #[test]
    fn opening_in_an_app_needs_the_user() {
        let kind = DialogKind::OpenInApp { request_id: 7 };
        assert_eq!(
            kind.accepted(DialogInput::default()),
            DialogReply::OpenInApp {
                request_id: 7,
                open: true
            }
        );
        assert_eq!(
            kind.dismissed(),
            DialogReply::OpenInApp {
                request_id: 7,
                open: false
            }
        );
    }
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
            dialogs.accept(text("typed")),
            Some(DialogReply::Prompt(Some("typed".into())))
        );
        assert!(dialogs.open().is_none());

        dialogs.request(request(DialogKind::Confirm));
        assert_eq!(
            dialogs.accept(DialogInput::default()),
            Some(DialogReply::Confirm(true))
        );
    }

    fn text(text: &str) -> DialogInput {
        DialogInput {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn sign_in_answers_with_credentials_or_none() {
        let sign_in = DialogKind::SignIn { request_id: 7 };
        let mut dialogs = PageDialogs::default();
        dialogs.request(request(sign_in.clone()));
        assert_eq!(
            dialogs.accept(DialogInput {
                text: "ada".into(),
                password: "secret".into(),
            }),
            Some(DialogReply::SignIn {
                request_id: 7,
                credentials: Some(Credentials {
                    username: "ada".into(),
                    password: "secret".into(),
                }),
            })
        );
        dialogs.request(request(sign_in));
        assert_eq!(
            dialogs.navigation_started(),
            Some(DialogReply::SignIn {
                request_id: 7,
                credentials: None,
            })
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
