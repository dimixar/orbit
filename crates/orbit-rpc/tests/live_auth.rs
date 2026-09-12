//! Live check: the `auth.*` RPC capability on a running pi.
//!
//! Skips when pi does not implement `auth.*` (an unpatched build answers
//! "Unknown command: auth.list"), so this test is green on stock pi and
//! meaningful once the server side is present — see
//! `crates/orbit-rpc/docs/auth-rpc.md` and `contrib/pi-auth-rpc/`.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use orbit_rpc::{AuthProvider, CommandBody, Event, PiClient};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[test]
fn live_auth_capability_round_trip() {
    let client = PiClient::spawn(&repo_root(), Some(Path::new("/tmp/orbit-pi-auth-sessions")))
        .expect("spawn pi");

    let rx = client.send(CommandBody::AuthList).expect("send auth.list");
    let response = rx
        .recv_timeout(Duration::from_secs(15))
        .expect("auth.list response");
    let Event::Response {
        success,
        data,
        error,
        ..
    } = response
    else {
        panic!("expected a response, got {response:?}");
    };
    if !success {
        // Stock pi: prove the client can still observe the rejection, then skip.
        let message = error.unwrap_or_default();
        assert!(
            message.to_ascii_lowercase().contains("unknown command")
                || message.to_ascii_lowercase().contains("unsupported"),
            "unexpected auth.list failure: {message}"
        );
        eprintln!("pi has no auth.* RPC ({message}); skipping live auth assertions");
        return;
    }

    let providers: Vec<AuthProvider> = data
        .as_ref()
        .and_then(|data| data.get("providers"))
        .and_then(serde_json::Value::as_array)
        .expect("providers array")
        .iter()
        .filter_map(AuthProvider::from_value)
        .collect();
    assert!(
        !providers.is_empty(),
        "a supporting pi must advertise at least one provider"
    );

    // At least one provider offers a login method, and every method id is
    // non-empty. This is exactly what the UI renders Connect buttons from.
    let mut method_count = 0;
    for provider in &providers {
        assert!(!provider.id.is_empty());
        for method in &provider.methods {
            assert!(
                !method.id.is_empty(),
                "{} has an empty method id",
                provider.id
            );
            method_count += 1;
        }
    }
    assert!(method_count > 0, "no login methods advertised");

    // auth.status round-trips for a real, advertised provider.
    let provider = providers
        .iter()
        .find(|provider| !provider.methods.is_empty())
        .expect("provider with methods");
    let rx = client
        .send(CommandBody::AuthStatus {
            provider: provider.id.clone(),
        })
        .expect("send auth.status");
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Event::Response { success, .. }) => assert!(success, "auth.status failed"),
        other => panic!("expected auth.status response, got {other:?}"),
    }
}
