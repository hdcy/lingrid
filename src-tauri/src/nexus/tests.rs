use super::graphql;

#[test]
fn parses_search_results_and_builds_page_urls() {
    let body = r#"{
      "data": { "mods": { "nodes": [
        { "modId": 2400, "name": "SMAPI", "summary": "The mod loader", "version": "4.5.2",
          "downloads": 123456, "author": "Pathoschild" },
        { "modId": 1, "name": "Sparse entry" }
      ] } }
    }"#;

    let mods = graphql::parse(body).unwrap();

    assert_eq!(mods.len(), 2);
    assert_eq!(mods[0].mod_id, 2400);
    assert_eq!(mods[0].name, "SMAPI");
    assert_eq!(mods[0].summary.as_deref(), Some("The mod loader"));
    assert_eq!(mods[0].downloads, Some(123456));
    assert_eq!(mods[0].author.as_deref(), Some("Pathoschild"));
    assert_eq!(
        mods[0].url,
        "https://www.nexusmods.com/stardewvalley/mods/2400"
    );
    // Missing fields are simply absent rather than failing the whole response.
    assert_eq!(mods[1].summary, None);
    assert_eq!(mods[1].downloads, None);
}

#[test]
fn graphql_errors_and_broken_payloads_are_reported() {
    assert_eq!(
        graphql::parse(r#"{"errors":[{"message":"nope"}]}"#)
            .unwrap_err()
            .code,
        "nexusQueryFailed"
    );
    assert_eq!(
        graphql::parse("not json").unwrap_err().code,
        "nexusQueryFailed"
    );
    assert_eq!(graphql::parse("{}").unwrap_err().code, "nexusQueryFailed");
    assert_eq!(
        graphql::parse(r#"{"data":{"mods":{"nodes":[]}}}"#)
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn parses_nxm_links_and_rejects_anything_else() {
    let link = super::nxm::parse(
        "nxm://stardewvalley/mods/2400/files/1234?key=abc&expires=1700000000&user_id=99",
    )
    .unwrap();
    assert_eq!(link.mod_id, 2400);
    assert_eq!(link.file_id, 1234);
    assert_eq!(link.key.as_deref(), Some("abc"));
    assert_eq!(link.expires, Some(1_700_000_000));

    // Premium links carry no key or expiry, and are still valid.
    let premium = super::nxm::parse("nxm://stardewvalley/mods/2400/files/1234").unwrap();
    assert_eq!(premium.key, None);
    assert_eq!(premium.expires, None);

    for bad in [
        "https://www.nexusmods.com/stardewvalley/mods/2400",
        "nxm://stardewvalley/mods/2400",
        "nxm://stardewvalley/collections/1/revisions/2",
        "nxm://stardewvalley/mods/abc/files/1",
        "nxm://stardewvalley/mods/1/files/abc",
        "",
    ] {
        assert!(super::nxm::parse(bad).is_err(), "{bad} must be rejected");
    }

    // A link for another game must not be turned into an API call.
    assert_eq!(
        super::nxm::parse("nxm://skyrimspecialedition/mods/2400/files/1")
            .unwrap_err()
            .code,
        "nxmUnsupportedGame"
    );

    // A mod-only link is the "open the page" form; the UI needs to tell the user that a
    // file has to be picked, so it gets its own code rather than a generic "invalid".
    assert_eq!(
        super::nxm::parse("nxm://stardewvalley/mods/5815")
            .unwrap_err()
            .code,
        "nxmNeedsFile"
    );
    assert_eq!(
        super::nxm::parse("nxm://stardewvalley/mods/5815/")
            .unwrap_err()
            .code,
        "nxmNeedsFile"
    );
}

#[test]
fn a_bare_number_is_treated_as_a_mod_id() {
    // Sharing a mod usually means sharing its id, so the search box accepts one.
    assert!(graphql::is_mod_id("5815"));
    assert!(graphql::is_mod_id("0"));
    assert!(!graphql::is_mod_id(""));
    assert!(!graphql::is_mod_id("smapi"));
    assert!(!graphql::is_mod_id("5815 extra"));
    assert!(!graphql::is_mod_id("mod-5815"));
    assert!(!graphql::is_mod_id(" 5815 "));
}
