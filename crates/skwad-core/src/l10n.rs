//! Localization lookup shim. Backed by a static table for now; a real
//! catalog (fluent or a keyed table per locale) replaces the body later
//! without changing this signature.

pub fn t(key: &str) -> String {
    lookup(key).unwrap_or(key).to_owned()
}

fn lookup(key: &str) -> Option<&'static str> {
    let value = match key {
        "app.name" => "Skwad",
        _ => return None,
    };

    Some(value)
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn known_key_resolves() {
        assert_eq!(t("app.name"), "Skwad");
    }

    #[test]
    fn unknown_key_returns_key() {
        assert_eq!(t("does.not.exist"), "does.not.exist");
    }
}
