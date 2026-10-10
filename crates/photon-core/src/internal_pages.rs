//! Addresses of the browser's own pages, such as `photon://settings`. The
//! scheme comes from `photon-brand`.

use photon_brand::PAGE_SCHEME;

/// The page name in an internal address, such as `settings` in
/// `photon://settings/`, or `None` for any other address.
pub fn internal_page_name(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case(PAGE_SCHEME) {
        return None;
    }
    let name = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    (!name.is_empty()).then_some(name)
}

/// The address of the internal page called `name`.
pub fn internal_page_url(name: &str) -> String {
    format!("{PAGE_SCHEME}://{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_page_name() {
        assert_eq!(internal_page_name("photon://settings"), Some("settings"));
        assert_eq!(
            internal_page_name("PHOTON://settings/privacy"),
            Some("settings")
        );
        assert_eq!(internal_page_name("photon://"), None);
        assert_eq!(internal_page_name("https://settings"), None);
        assert_eq!(internal_page_url("newtab"), "photon://newtab");
    }
}
