//! Test-only subprocess fixture. No fault switches or test CA enter release builds.
use crate::{BoundServer, GameProfile, OidcConfig, ServerConfig, StorageConfig};
use std::{env, io::Write, time::Duration};
use worldwright_core::ids::{AccountId, ActivationId, CharacterId, PublicationId, WorldId, WriterId};
use worldwright_persistence::{PostgresStore, SeedAccount, SeedWorld};
use worldwright_release::{DeploymentTarget, GameContract, PackageOptions, PublicationKind, ReviewDecision, package_bytes};

#[tokio::test]
#[ignore = "subprocess fixture requires scripts/r2-live.py and isolated PostgreSQL schema"]
async fn r2_fixture_process() {
    assert_eq!(env::var("WORLDWRIGHT_R2_FIXTURE").as_deref(), Ok("isolated-test-only"));
    let url = env::var("WORLDWRIGHT_TEST_DATABASE_URL").unwrap();
    let parsed = reqwest::Url::parse(&url).unwrap();
    assert!(parsed.path().contains("worldwright_test"));
    assert!(parsed.query_pairs().any(|(k,v)| k == "options" && v.starts_with("-csearch_path=ww_r2_")));
    let expedition = env::var("WORLDWRIGHT_R2_GAME").unwrap() == "expedition";
    let game = if expedition { GameContract::Expedition } else { GameContract::Reference };
    let bytes: &[u8] = if expedition { include_bytes!("../../../examples/expedition/world.json") }
        else { include_bytes!("../../../examples/two-room/world.json") };
    let content = worldwright_content::load_bytes("R2 fixture", bytes, Default::default()).unwrap();
    let world = WorldId::from_u128(1).unwrap();
    let store = PostgresStore::connect(&url).await.unwrap();
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('worldwright_worlds') IS NOT NULL")
        .fetch_one(&sqlx::PgPool::connect(&url).await.unwrap()).await.unwrap();
    if !exists {
        store.migrate().await.unwrap();
        store.verify_migrations().await.unwrap();
        let mut accounts = Vec::new();
        for (id, username, name) in [(1u128,"alice","Alice"),(2,"bob","Bob")] {
            accounts.push(SeedAccount { account: AccountId::from_u128(id).unwrap(), username:username.into(),
                password_hash:crate::auth::make_hash(b"r2-disposable-only-password").unwrap(),
                character:CharacterId::from_u128(id).unwrap(),character_name:name.into() });
        }
        store.seed(&SeedWorld { world,content_namespace:content.namespace().into(),content_schema_version:content.schema_version(),
            game,entry_room_definition:content.entry_room().as_str().into(),accounts }).await.unwrap();
        let artifact = package_bytes("R2 fixture",bytes,PackageOptions { release_id:"r2-live".parse().unwrap(),
            source_revision:"r2-live-fixture".into(),builder_identity:"r2-test-only".into(),game }).unwrap();
        let publication = PublicationId::from_u128(1).unwrap();
        store.submit_publication(publication,world,&artifact,PublicationKind::Activate,DeploymentTarget::Staging,None,"r2-requester").await.unwrap();
        store.review_publication(publication,"r2-reviewer",ReviewDecision::Approve,"disposable integration fixture").await.unwrap();
        let plan = store.begin_activation(publication,ActivationId::from_u128(1).unwrap(),WriterId::from_u128(1).unwrap(),"r2-operator").await.unwrap();
        store.complete_activation(plan).await.unwrap();
    }
    let origin = env::var("WORLDWRIGHT_R2_ORIGIN").unwrap();
    let oidc = env::var("WORLDWRIGHT_R2_ISSUER").ok().map(|issuer| OidcConfig { issuer,
        client_id:"r2-client".into(),client_secret:"r2-disposable-client-secret".into(),
        redirect_uri:format!("{origin}/api/v1/oidc/callback") });
    let server = BoundServer::bind(ServerConfig { bind:env::var("WORLDWRIGHT_R2_BIND").unwrap().parse().unwrap(),
        allowed_origin:origin,storage:StorageConfig::Postgres { database_url:url },allow_insecure_http:true,
        behind_tls_ingress:false,game:if expedition { GameProfile::Expedition } else { GameProfile::Reference },operator:None,oidc }).await.unwrap();
    println!("WORLDWRIGHT_R2_READY={}",server.local_addr());
    std::io::stdout().flush().unwrap();
    server.serve(tokio::time::sleep(Duration::from_secs(180))).await.unwrap();
}
