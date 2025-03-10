#![deny(warnings)]
#![warn(unused_extern_crates)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![deny(clippy::await_holding_lock)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::trivially_copy_pass_by_ref)]

use std::net::TcpStream;
use std::sync::atomic::{AtomicU16, Ordering};

use kanidm_client::{KanidmClient, KanidmClientBuilder};
use kanidm_proto::v1::{Filter, Modify, ModifyList};
use kanidmd_core::config::{Configuration, IntegrationTestConfig};
use kanidmd_core::{create_server_core, CoreHandle};
use tokio::task;

pub const ADMIN_TEST_USER: &str = "admin";
pub const ADMIN_TEST_PASSWORD: &str = "integration test admin password";

pub const NOT_ADMIN_TEST_USERNAME: &str = "krab_test_user";
pub const NOT_ADMIN_TEST_PASSWORD: &str = "eicieY7ahchaoCh0eeTa";

pub static PORT_ALLOC: AtomicU16 = AtomicU16::new(18080);

pub use testkit_macros::test;

pub fn is_free_port(port: u16) -> bool {
    TcpStream::connect(("0.0.0.0", port)).is_err()
}

// Test external behaviours of the service.

// allowed because the use of this function is behind a test gate
#[allow(dead_code)]
pub async fn setup_async_test(mut config: Configuration) -> (KanidmClient, CoreHandle) {
    sketching::test_init();

    let mut counter = 0;
    let port = loop {
        let possible_port = PORT_ALLOC.fetch_add(1, Ordering::SeqCst);
        if is_free_port(possible_port) {
            break possible_port;
        }
        counter += 1;
        #[allow(clippy::panic)]
        if counter >= 5 {
            eprintln!("Unable to allocate port!");
            panic!();
        }
    };

    let int_config = Box::new(IntegrationTestConfig {
        admin_user: ADMIN_TEST_USER.to_string(),
        admin_password: ADMIN_TEST_PASSWORD.to_string(),
    });

    let addr = format!("http://localhost:{}", port);

    // Setup the address and origin..
    config.address = format!("127.0.0.1:{}", port);
    config.integration_test_config = Some(int_config);
    config.domain = "localhost".to_string();
    config.origin = addr.clone();

    let core_handle = match create_server_core(config, false).await {
        Ok(val) => val,
        #[allow(clippy::panic)]
        Err(_) => panic!("failed to start server core"),
    };
    // We have to yield now to guarantee that the elements are setup.
    task::yield_now().await;

    #[allow(clippy::panic)]
    let rsclient = match KanidmClientBuilder::new()
        .address(addr.clone())
        .no_proxy()
        .build()
    {
        Ok(val) => val,
        Err(_) => panic!("failed to build client"),
    };

    tracing::info!("Testkit server setup complete - {}", addr);

    (rsclient, core_handle)
}

/// creates a user (username: `id`) and puts them into a group, creating it if need be.
pub async fn create_user(rsclient: &KanidmClient, id: &str, group_name: &str) {
    #[allow(clippy::expect_used)]
    rsclient
        .idm_person_account_create(id, id)
        .await
        .expect("Failed to create the user");

    // Create group and add to user to test read attr: member_of
    #[allow(clippy::expect_used)]
    if rsclient
        .idm_group_get(group_name)
        .await
        .expect("Failed to get group")
        .is_none()
    {
        #[allow(clippy::expect_used)]
        rsclient
            .idm_group_create(group_name)
            .await
            .expect("Failed to create group");
    }

    #[allow(clippy::expect_used)]
    rsclient
        .idm_group_add_members(group_name, &[id])
        .await
        .expect("Failed to set group membership for user");
}

pub async fn create_user_with_all_attrs(
    rsclient: &KanidmClient,
    id: &str,
    optional_group: Option<&str>,
) {
    let group_format = format!("{}_group", id);
    let group_name = optional_group.unwrap_or(&group_format);

    create_user(rsclient, id, group_name).await;
    add_all_attrs(rsclient, id, group_name, Some(id)).await;
}

pub async fn add_all_attrs(
    rsclient: &KanidmClient,
    id: &str,
    group_name: &str,
    legalname: Option<&str>,
) {
    // Extend with posix attrs to test read attr: gidnumber and loginshell
    #[allow(clippy::expect_used)]
    rsclient
        .idm_person_account_unix_extend(id, None, Some("/bin/sh"))
        .await
        .expect("Failed to set shell to /bin/sh for user");
    #[allow(clippy::expect_used)]
    rsclient
        .idm_group_unix_extend(group_name, None)
        .await
        .expect("Failed to extend user group");

    for attr in ["ssh_publickey", "mail"].iter() {
        println!("Checking writable for {}", attr);
        #[allow(clippy::expect_used)]
        let res = is_attr_writable(rsclient, id, attr)
            .await
            .expect("Failed to get wriable status for attribute");
        assert!(res);
    }

    if let Some(legalname) = legalname {
        #[allow(clippy::expect_used)]
        let res = is_attr_writable(rsclient, legalname, "legalname")
            .await
            .expect("Failed to get writable status for legalname field");
        assert!(res);
    }

    // Write radius credentials
    if id != "anonymous" {
        login_account(rsclient, id).await;
        #[allow(clippy::expect_used)]
        let _ = rsclient
            .idm_account_radius_credential_regenerate(id)
            .await
            .expect("Failed to regen password for user");

        #[allow(clippy::expect_used)]
        rsclient
            .auth_simple_password(ADMIN_TEST_USER, ADMIN_TEST_PASSWORD)
            .await
            .expect("Failed to auth with password as admin!");
    }
}

pub async fn is_attr_writable(rsclient: &KanidmClient, id: &str, attr: &str) -> Option<bool> {
    println!("writing to attribute: {}", attr);
    match attr {
        "radius_secret" => Some(
            rsclient
                .idm_account_radius_credential_regenerate(id)
                .await
                .is_ok(),
        ),
        "primary_credential" => Some(
            rsclient
                .idm_person_account_primary_credential_set_password(id, "dsadjasiodqwjk12asdl")
                .await
                .is_ok(),
        ),
        "ssh_publickey" => Some(
            rsclient
                .idm_person_account_post_ssh_pubkey(
                    id,
                    "k1",
                    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAeGW1P6Pc2rPq0XqbRaDKBcXZUPRklo0\
                     L1EyR30CwoP william@amethyst",
                )
                .await
                .is_ok(),
        ),
        "unix_password" => Some(
            rsclient
                .idm_person_account_unix_cred_put(id, "dsadjasiodqwjk12asdl")
                .await
                .is_ok(),
        ),
        "legalname" => Some(
            rsclient
                .idm_person_account_set_attr(id, "legalname", &["test legal name"])
                .await
                .is_ok(),
        ),
        "mail" => Some(
            rsclient
                .idm_person_account_set_attr(id, "mail", &[&format!("{}@example.com", id)])
                .await
                .is_ok(),
        ),
        entry => {
            let new_value = match entry {
                "acp_receiver_group" => "00000000-0000-0000-0000-000000000011".to_string(),
                "acp_targetscope" => "{\"and\": [{\"eq\": [\"class\",\"access_control_profile\"]}, {\"andnot\": {\"or\": [{\"eq\": [\"class\", \"tombstone\"]}, {\"eq\": [\"class\", \"recycled\"]}]}}]}".to_string(),
                 _ => id.to_string(),
            };
            let m = ModifyList::new_list(vec![
                Modify::Purged(attr.to_string()),
                Modify::Present(attr.to_string(), new_value),
            ]);
            let f = Filter::Eq("name".to_string(), id.to_string());
            Some(rsclient.modify(f.clone(), m.clone()).await.is_ok())
        }
    }
}

pub async fn login_account(rsclient: &KanidmClient, id: &str) {
    #[allow(clippy::expect_used)]
    rsclient
        .idm_group_add_members(
            "idm_people_account_password_import_priv",
            &[ADMIN_TEST_USER],
        )
        .await
        .expect("Failed to add user to idm_people_account_password_import_priv");

    #[allow(clippy::expect_used)]
    rsclient
        .idm_group_add_members("idm_people_extend_priv", &[ADMIN_TEST_USER])
        .await
        .expect("Failed to add user to idm_people_extend_priv");

    #[allow(clippy::expect_used)]
    rsclient
        .idm_person_account_primary_credential_set_password(id, NOT_ADMIN_TEST_PASSWORD)
        .await
        .expect("Failed to set password for user");

    let _ = rsclient.logout().await;
    let res = rsclient
        .auth_simple_password(id, NOT_ADMIN_TEST_PASSWORD)
        .await;

    // Setup privs
    println!("{} logged in", id);
    assert!(res.is_ok());

    let res = rsclient
        .reauth_simple_password(NOT_ADMIN_TEST_PASSWORD)
        .await;
    println!("{} priv granted for", id);
    assert!(res.is_ok());
}

// Login to the given account, but first login with default admin credentials.
// This is necessary when switching between unprivileged accounts, but adds extra calls which
// create extra debugging noise, so should be avoided when unnecessary.
pub async fn login_account_via_admin(rsclient: &KanidmClient, id: &str) {
    let _ = rsclient.logout().await;

    #[allow(clippy::expect_used)]
    rsclient
        .auth_simple_password(ADMIN_TEST_USER, ADMIN_TEST_PASSWORD)
        .await
        .expect("Failed to login as admin!");
    login_account(rsclient, id).await
}

pub async fn test_read_attrs(rsclient: &KanidmClient, id: &str, attrs: &[&str], is_readable: bool) {
    println!("Test read to {}, is readable: {}", id, is_readable);
    #[allow(clippy::expect_used)]
    let rset = rsclient
        .search(Filter::Eq("name".to_string(), id.to_string()))
        .await
        .expect("Can't get user from search");

    #[allow(clippy::expect_used)]
    let e = rset.first().expect("Failed to get first user from set");

    for attr in attrs.iter() {
        println!("Reading {}", attr);
        #[allow(clippy::unwrap_used)]
        let is_ok = match *attr {
            "radius_secret" => rsclient
                .idm_account_radius_credential_get(id)
                .await
                .unwrap()
                .is_some(),
            _ => e.attrs.get(*attr).is_some(),
        };

        assert!(is_ok == is_readable)
    }
}

pub async fn test_write_attrs(
    rsclient: &KanidmClient,
    id: &str,
    attrs: &[&str],
    is_writeable: bool,
) {
    println!("Test write to {}, is writeable: {}", id, is_writeable);
    for attr in attrs.iter() {
        println!("Writing to {} - ex {}", attr, is_writeable);
        #[allow(clippy::unwrap_used)]
        let is_ok = is_attr_writable(rsclient, id, attr).await.unwrap();
        assert!(is_ok == is_writeable)
    }
}

pub async fn test_modify_group(
    rsclient: &KanidmClient,
    group_names: &[&str],
    can_be_modified: bool,
) {
    // need user test created to be added as test part
    for group in group_names.iter() {
        println!("Testing group: {}", group);
        for attr in ["description", "name"].iter() {
            #[allow(clippy::unwrap_used)]
            let is_writable = is_attr_writable(rsclient, group, attr).await.unwrap();
            assert!(is_writable == can_be_modified)
        }
        assert!(
            rsclient
                .idm_group_add_members(group, &[NOT_ADMIN_TEST_USERNAME])
                .await
                .is_ok()
                == can_be_modified
        );
    }
}

/// Logs in with the admin user and puts them in idm_admins so they can do admin things
pub async fn login_put_admin_idm_admins(rsclient: &KanidmClient) {
    #[allow(clippy::expect_used)]
    rsclient
        .auth_simple_password(ADMIN_TEST_USER, ADMIN_TEST_PASSWORD)
        .await
        .expect("Failed to authenticate as admin!");

    #[allow(clippy::expect_used)]
    rsclient
        .idm_group_add_members("idm_admins", &[ADMIN_TEST_USER])
        .await
        .expect("Failed to add admin user to idm_admins")
}
