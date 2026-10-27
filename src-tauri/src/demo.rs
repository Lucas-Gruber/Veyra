//! Canned, public demonstration values. Never substitute real credentials here.

use crate::vaultwarden::Field;
use zeroize::Zeroizing;

pub fn field(item_id: &str, field: Field) -> Result<Zeroizing<String>, String> {
    let username = match item_id {
        "github" | "linear" | "vercel" | "proton" | "figma" | "tailscale" | "notion"
        | "cloudflare" => "alex@veyra.example",
        _ => return Err("Unknown demonstration item.".into()),
    };
    let value = match field {
        Field::Username => username,
        Field::Password => "Veyra-demo-only!2026",
        Field::Totp => "123456",
    };
    Ok(Zeroizing::new(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_item_cannot_be_used_for_credential_access() {
        assert!(field("../../anything", Field::Password).is_err());
        assert!(field("", Field::Username).is_err());
    }

    #[test]
    fn sample_codes_are_explicitly_static_not_real_totp() {
        assert_eq!(&*field("github", Field::Totp).unwrap(), "123456");
    }
}
