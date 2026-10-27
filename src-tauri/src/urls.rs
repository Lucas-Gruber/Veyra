use url::Url;

pub fn validate_website(input: &str) -> Result<Url, String> {
    if input.len() > 4096 || input.chars().any(char::is_control) {
        return Err("Invalid website URL.".into());
    }
    let url = Url::parse(input).map_err(|_| "Invalid website URL.".to_owned())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only HTTPS websites without embedded credentials may be opened.".into());
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_schemes_credentials_and_malformed_urls() {
        for input in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "http://github.com",
            "https://user:secret@github.com",
            "https://user@github.com",
            "https://github.com\n",
            "not a url",
        ] {
            assert!(validate_website(input).is_err(), "accepted unsafe input");
        }
    }

    #[test]
    fn accepts_and_normalizes_a_standard_https_website() {
        let url = validate_website("https://github.com/settings").unwrap();
        assert_eq!(url.host_str(), Some("github.com"));
        assert_eq!(url.path(), "/settings");
    }
}
