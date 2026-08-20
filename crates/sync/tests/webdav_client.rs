//! WebDAV client primitives against the in-process test server:
//! exclusive create, conditional replace, idempotent delete,
//! collection creation, the onboarding precondition probe, and credential
//! wiring. Everything runs on loopback; no real network.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::atomic::Ordering;

use common::WebdavServer;
use typvia_sync::{PutOutcome, TransportError, WebdavClient, WebdavCredentials};

fn client(server: &WebdavServer) -> WebdavClient {
    WebdavClient::new(&server.url, None).expect("client")
}

#[test]
fn exclusive_put_creates_once_and_refuses_the_second_claim() {
    let server = WebdavServer::start();
    let dav = client(&server);
    assert_eq!(
        dav.put_exclusive("typvia-sync/claim.json", b"first")
            .unwrap(),
        PutOutcome::Done
    );
    assert_eq!(
        dav.put_exclusive("typvia-sync/claim.json", b"second")
            .unwrap(),
        PutOutcome::PreconditionFailed
    );
    // The loser's bytes never landed.
    let fetched = dav.get("typvia-sync/claim.json").unwrap().expect("file");
    assert_eq!(fetched.bytes, b"first");
}

#[test]
fn concurrent_exclusive_claims_admit_exactly_one_winner() {
    let server = WebdavServer::start();
    let url = server.url.clone();
    let mut handles = Vec::new();
    for i in 0..4 {
        let url = url.clone();
        handles.push(std::thread::spawn(move || {
            let dav = WebdavClient::new(&url, None).expect("client");
            dav.put_exclusive("typvia-sync/seat.json", format!("racer-{i}").as_bytes())
                .expect("request")
        }));
    }
    let outcomes: Vec<PutOutcome> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let winners = outcomes.iter().filter(|o| **o == PutOutcome::Done).count();
    assert_eq!(
        winners, 1,
        "exactly one exclusive create may win: {outcomes:?}"
    );
}

#[test]
fn get_round_trips_bytes_and_etags_and_reports_absence_as_none() {
    let server = WebdavServer::start();
    let dav = client(&server);
    assert!(dav.get("typvia-sync/missing.json").unwrap().is_none());

    dav.put_exclusive("typvia-sync/file.json", b"v1-bytes")
        .unwrap();
    let first = dav.get("typvia-sync/file.json").unwrap().expect("file");
    assert_eq!(first.bytes, b"v1-bytes");
    let etag = first.etag.expect("etag");

    // A conditional replace with the fresh ETag lands; the stale one is
    // refused and leaves the winner's bytes intact.
    assert_eq!(
        dav.put_if_match("typvia-sync/file.json", &etag, b"v2-bytes")
            .unwrap(),
        PutOutcome::Done
    );
    assert_eq!(
        dav.put_if_match("typvia-sync/file.json", &etag, b"stale-write")
            .unwrap(),
        PutOutcome::PreconditionFailed
    );
    let latest = dav.get("typvia-sync/file.json").unwrap().expect("file");
    assert_eq!(latest.bytes, b"v2-bytes");
}

#[test]
fn delete_is_idempotent_and_mkcol_tolerates_existing_collections() {
    let server = WebdavServer::start();
    let dav = client(&server);
    dav.put_exclusive("typvia-sync/gone.json", b"x").unwrap();
    dav.delete("typvia-sync/gone.json").unwrap();
    dav.delete("typvia-sync/gone.json").unwrap();
    assert!(dav.get("typvia-sync/gone.json").unwrap().is_none());

    dav.mkcol_all("typvia-sync/devices/dev-a/records").unwrap();
    dav.mkcol_all("typvia-sync/devices/dev-a/records").unwrap();
}

#[test]
fn the_probe_accepts_honest_endpoints_and_refuses_precondition_strippers() {
    let server = WebdavServer::start();
    let dav = client(&server);
    dav.probe_preconditions("nonce-1")
        .expect("honest endpoint passes");

    server
        .state
        .strip_preconditions
        .store(true, Ordering::SeqCst);
    let err = dav
        .probe_preconditions("nonce-2")
        .expect_err("stripped preconditions refused");
    assert!(
        matches!(&err, TransportError::Api { code, .. } if code == "PRECONDITIONS_UNSUPPORTED"),
        "unexpected refusal: {err:?}"
    );
}

#[test]
fn credentials_travel_in_the_authorization_header_only() {
    let server = WebdavServer::start();
    let credentials = WebdavCredentials::parse("basic:FAKE_user:FAKE_dav_pw").unwrap();
    let dav = WebdavClient::new(&server.url, Some(&credentials)).expect("client");
    dav.put_exclusive("typvia-sync/authed.json", b"x").unwrap();
    let seen = server.state.last_authorization.lock().unwrap().clone();
    let expected = format!("Basic {}", {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode("FAKE_user:FAKE_dav_pw")
    });
    assert_eq!(seen.as_deref(), Some(expected.as_str()));
}

#[test]
fn a_failing_endpoint_surfaces_as_a_status_error_not_a_panic() {
    let server = WebdavServer::start();
    let dav = client(&server);
    server.state.fail_all.store(true, Ordering::SeqCst);
    let err = dav
        .get("typvia-sync/anything.json")
        .expect_err("500 surfaces");
    assert!(matches!(err, TransportError::Api { status: 500, .. }));
}
