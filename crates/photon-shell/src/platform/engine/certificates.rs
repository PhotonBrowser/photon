//! The certificates sites presented, and letting the user proceed past a
//! host's certificate error.

use std::ffi::{CString, c_void};

use super::EngineRuntime;
use super::callbacks::engine_string;
use crate::platform::engine_ffi::embedder;

/// The certificate a host presented, as a certificate viewer shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Certificate {
    pub(in crate::platform) subject_common_name: String,
    pub(in crate::platform) subject_organization: String,
    pub(in crate::platform) issuer_common_name: String,
    pub(in crate::platform) issuer_organization: String,
    pub(in crate::platform) valid_from: String,
    pub(in crate::platform) valid_until: String,
    pub(in crate::platform) sha256_fingerprint: String,
    /// False once the user chose to proceed past a certificate error.
    pub(in crate::platform) verified: bool,
    /// The connection's protocol and cipher, such as `TLSv1.3`.
    pub(in crate::platform) protocol: String,
    pub(in crate::platform) cipher: String,
    /// Who each certificate the server sent is issued to, from the site's
    /// own towards its root.
    pub(in crate::platform) chain: Vec<String>,
}

type CertificateReceived = Box<dyn FnOnce(Option<Certificate>)>;

unsafe extern "C" fn on_engine_certificate(
    context: *mut c_void,
    certificate: *const embedder::PhotonCertificate,
) {
    if context.is_null() {
        return;
    }
    let done = unsafe { Box::from_raw(context.cast::<CertificateReceived>()) };
    let certificate = (!certificate.is_null()).then(|| {
        let certificate = unsafe { &*certificate };
        let text = |value| unsafe { engine_string(value) }.unwrap_or_default();
        Certificate {
            subject_common_name: text(certificate.subject_common_name),
            subject_organization: text(certificate.subject_organization),
            issuer_common_name: text(certificate.issuer_common_name),
            issuer_organization: text(certificate.issuer_organization),
            valid_from: text(certificate.valid_from),
            valid_until: text(certificate.valid_until),
            sha256_fingerprint: text(certificate.sha256_fingerprint),
            verified: certificate.verified,
            protocol: text(certificate.protocol),
            cipher: text(certificate.cipher),
            chain: if certificate.chain.is_null() {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(certificate.chain, certificate.chain_count) }
                    .iter()
                    .map(|&name| text(name))
                    .collect()
            },
        }
    });
    done(certificate);
}

impl EngineRuntime {
    /// Lets pages from `host` load although its certificate cannot be
    /// verified, until Photon quits.
    pub(in crate::platform) fn allow_certificate_errors_for_host(&self, host: &str) {
        if !self.is_available() {
            return;
        }
        let Ok(host) = CString::new(host) else {
            return;
        };
        unsafe {
            embedder::photon_runtime_allow_certificate_errors_for_host(self.runtime, host.as_ptr());
        }
    }

    /// Calls `done` on the main thread with the certificate `host` last
    /// presented, or `None` when Engine has not connected to it. `done` must
    /// not update GPUI entities synchronously.
    pub(in crate::platform) fn peer_certificate(
        &self,
        host: &str,
        done: impl FnOnce(Option<Certificate>) + 'static,
    ) {
        let Ok(host) = CString::new(host) else {
            done(None);
            return;
        };
        if !self.is_available() {
            done(None);
            return;
        }
        let done: Box<CertificateReceived> = Box::new(Box::new(done));
        unsafe {
            embedder::photon_runtime_peer_certificate(
                self.runtime,
                host.as_ptr(),
                Box::into_raw(done).cast(),
                Some(on_engine_certificate),
            );
        }
    }
}
