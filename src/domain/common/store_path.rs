/// Extract package name from a full Nix store path (`/nix/store/hash-name` → `name`).
pub fn store_path_name(store_path: &str) -> Option<&str> {
    let base = store_path.rsplit('/').next()?;
    base.split_once('-')
        .map(|(_, name)| name)
        .filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_path_name_extracts_package_name() {
        assert_eq!(
            store_path_name("/nix/store/zj64jfhbxbync50az13gxr6k7bnqhcb3-codex-0.144.5"),
            Some("codex-0.144.5")
        );
    }

    #[test]
    fn store_path_name_returns_none_without_name() {
        assert_eq!(
            store_path_name("/nix/store/zj64jfhbxbync50az13gxr6k7bnqhcb3"),
            None
        );
        assert_eq!(store_path_name(""), None);
    }
}
