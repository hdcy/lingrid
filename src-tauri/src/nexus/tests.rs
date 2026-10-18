use super::graphql;
use std::fs;
use tempfile::TempDir;

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
fn download_keys_are_validated_before_they_reach_the_api() {
    // The key is pasted into the request URL, so only the shape Nexus issues is accepted:
    // anything else could append parameters to the download request.
    for bad in [
        "nxm://stardewvalley/mods/1/files/2?key=a%26expires%3D9&expires=1",
        "nxm://stardewvalley/mods/1/files/2?key=abc",
        "nxm://stardewvalley/mods/1/files/2?key=&expires=1",
        "nxm://stardewvalley/mods/1/files/2?key=abc&expires=nope",
        "nxm://stardewvalley/mods/0/files/1",
    ] {
        assert_eq!(
            super::nxm::parse(bad).unwrap_err().code,
            "nxmInvalid",
            "{bad} must be rejected"
        );
    }

    // Unknown query parameters are ignored, and a percent-encoded key is decoded first.
    let link =
        super::nxm::parse("nxm://stardewvalley/mods/1/files/2?key=ab%63&expires=1&user_id=7")
            .unwrap();
    assert_eq!(link.key.as_deref(), Some("abc"));
    assert_eq!(link.expires, Some(1));
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

#[test]
fn only_nexus_hosts_are_accepted_for_downloads() {
    for host in [
        "nexusmods.com",
        "www.nexusmods.com",
        "cf-files.nexusmods.com",
        "files.nexus-cdn.com",
    ] {
        assert!(
            super::nxm::is_allowed_host(host),
            "{host} should be allowed"
        );
    }
    for host in [
        "evil-nexusmods.com",
        "nexusmods.com.evil.example",
        "example.com",
        "",
        "nexus-cdn.com.evil",
    ] {
        assert!(
            !super::nxm::is_allowed_host(host),
            "{host} must be rejected"
        );
    }
}

#[test]
fn a_download_is_only_treated_as_a_zip_when_it_starts_like_one() {
    let temp = TempDir::new().unwrap();
    let zip = temp.path().join("archive.zip");
    fs::write(&zip, b"PK\x03\x04rest").unwrap();
    assert!(super::nxm::starts_like_zip(&zip).unwrap());

    let text = temp.path().join("page.html");
    fs::write(&text, b"<html>login</html>").unwrap();
    assert!(!super::nxm::starts_like_zip(&text).unwrap());
}
