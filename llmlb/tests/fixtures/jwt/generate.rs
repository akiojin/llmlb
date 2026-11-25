// Isolated fixture tool: never linked into llmlb or its dependency graph.
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const SECRET: &str = "public-issue-948-compatibility-fixture-not-a-real-secret";

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Claims {
    sub: String,
    role: String,
    exp: usize,
    must_change_password: bool,
    password_changed_at: i64,
}

#[derive(Serialize, Deserialize)]
struct Fixture {
    claims: Claims,
    token: String,
}

fn main() {
    if let Some(path) = std::env::args().nth(1) {
        let fixtures: Vec<Fixture> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut roles: Vec<_> = fixtures
            .iter()
            .map(|fixture| fixture.claims.role.as_str())
            .collect();
        roles.sort_unstable();
        assert_eq!(
            roles,
            ["admin", "viewer"],
            "expected one admin and one viewer fixture"
        );
        for fixture in fixtures {
            let verified = decode::<Claims>(
                &fixture.token,
                &DecodingKey::from_secret(SECRET.as_bytes()),
                &Validation::default(),
            )
            .unwrap();
            assert_eq!(verified.claims, fixture.claims);
            assert_eq!(
                encode(
                    &Header::default(),
                    &fixture.claims,
                    &EncodingKey::from_secret(SECRET.as_bytes()),
                )
                .unwrap(),
                fixture.token
            );
        }
        println!("provider fixture verification: PASS (admin and viewer, all claims)");
        return;
    }

    let fixtures: Vec<_> = [("admin", true), ("viewer", false)]
        .into_iter()
        .map(|(role, must_change_password)| {
            let claims = Claims {
                sub: format!("issue-948-{role}"),
                role: role.into(),
                exp: 4_102_444_800, // 2100-01-01 UTC; independent of today's clock.
                must_change_password,
                password_changed_at: 1_793_010_000_000,
            };
            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(SECRET.as_bytes()),
            )
            .unwrap();
            Fixture { claims, token }
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&fixtures).unwrap());
}
