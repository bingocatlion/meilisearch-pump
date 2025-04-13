use meilisearch::standalone::try_alone_run;
#[test]
pub fn test_alone_server() {
    try_alone_run("/Users/tianlan/Documents/zoos/pump/meilisearch/config.toml");
}