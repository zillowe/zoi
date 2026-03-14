use zoi::pkg::offline;
use zoi::utils;

#[test]
fn test_offline_mode_toggle() {
    offline::set_offline(true);
    assert!(offline::is_offline());

    offline::set_offline(false);
}

#[test]
fn test_http_client_blocked_in_offline() {
    offline::set_offline(true);
    let client = utils::get_http_client();
    assert!(
        client.is_err(),
        "HTTP client should not be created in offline mode"
    );
}
