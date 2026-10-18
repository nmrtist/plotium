use super::*;
#[test]
fn stdio_config_preserves_absolute_unicode_paths_without_tokens_or_shell() {
    let path = Path::new("/Users/test name/数据/plotium-mcp");
    let config = stdio_config(path);
    assert_eq!(
        config["mcpServers"]["plotium"]["command"],
        path.to_str().unwrap()
    );
    assert_eq!(
        config["mcpServers"]["plotium"]["args"],
        serde_json::json!([])
    );
    assert!(!config.to_string().contains("token"));
}
