use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use crate::config::Config as KeycloakConfig;

use chrono::prelude::*;
pub use keycloak::types::{
    ClientRepresentation, CredentialRepresentation, GroupRepresentation, RealmRepresentation,
    RoleRepresentation, UserRepresentation,
};
pub use keycloak::{KeycloakAdmin, KeycloakError, KeycloakTokenSupplier};
use tokio::runtime::Builder;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::RwLock;
use tokio::task::LocalSet;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub realm: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RealmInfo {
    #[serde(default)]
    pub realm: Option<String>,
    #[serde(default)]
    pub public_key: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ParsedAccessToken {
    exp: usize,
    //:1677048774,
    iat: usize,
    //:1677048714,
    // auth_time: usize, //:1677047319,
    jti: Option<String>,
    //:"48ef7bc9-1a42-4e4f-b136-5fd74d4d6033",
    iss: Option<String>,
    //:"https://id.qm.local/realms/master",
    sub: Option<String>,
    //:"fe487690-8c65-4106-95a5-5b1dbb8e6bbd",
    typ: Option<String>,
    //:"Bearer",
    azp: Option<String>,
    //:"security-admin-console",
    nonce: Option<String>,
    //:"86e7e8a2-5af5-4fed-80e7-1da412e51070",
    session_state: Option<String>,
    //:"cdfaa367-5c30-4142-b31a-f770073e2051",
    acr: Option<String>,
    //:"0",
    allowed: Option<Vec<String>>,
    //origins":["https://keycloak.qm.local"],
    scope: Option<String>,
    //:"openid profile email",
    sid: Option<String>,
    //:"cdfaa367-5c30-4142-b31a-f770073e2051",
    email_verified: bool,
    //:false,
    preferred_username: Option<String>, //:"admin"
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct KeycloakSession {
    access_token: String,
    expires_in: usize,
    #[serde(rename = "not-before-policy")]
    not_before_policy: Option<usize>,
    refresh_expires_in: Option<usize>,
    refresh_token: Option<String>,
    scope: String,
    session_state: Option<String>,
    token_type: String,
    #[serde(skip)]
    parsed_access_token: Option<ParsedAccessToken>,
}

impl KeycloakSession {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    fn parse_access_token(mut token: KeycloakSession) -> KeycloakSession {
        use base64::engine::{general_purpose::STANDARD_NO_PAD, Engine};
        if let Some(parsed_access_token) = token
            .access_token
            .split('.')
            .nth(1)
            .and_then(|s| {
                STANDARD_NO_PAD
                    .decode(s)
                    .map_err(|e| {
                        log::error!("{e:#?}");
                        e
                    })
                    .ok()
            })
            .and_then(|b| {
                serde_json::from_slice::<ParsedAccessToken>(&b)
                    .map_err(|e| {
                        log::error!("{e:#?}");
                        e
                    })
                    .ok()
            })
        {
            token.parsed_access_token = Some(parsed_access_token);
        }
        token
    }

    pub async fn acquire(
        url: &str,
        username: &str,
        password: &str,
        client: &reqwest::Client,
    ) -> Result<KeycloakSession, KeycloakError> {
        Self::acquire_custom_realm(
            url,
            username,
            password,
            "master",
            "admin-cli",
            "password",
            client,
        )
        .await
        .map(KeycloakSession::parse_access_token)
    }

    pub async fn acquire_custom_realm(
        url: &str,
        username: &str,
        password: &str,
        realm: &str,
        client_id: &str,
        grant_type: &str,
        client: &reqwest::Client,
    ) -> Result<KeycloakSession, KeycloakError> {
        let response = client
            .post(&format!(
                "{url}/realms/{realm}/protocol/openid-connect/token",
            ))
            .form(&serde_json::json!({
                "username": username,
                "password": password,
                "client_id": client_id,
                "grant_type": grant_type
            }))
            .send()
            .await?;
        Ok(error_check(response).await?.json().await?)
    }

    pub async fn refresh(
        url: &str,
        refresh_token: &str,
        client: &reqwest::Client,
    ) -> Result<KeycloakSession, KeycloakError> {
        Self::refresh_custom_realm(url, "master", "admin-cli", refresh_token, client)
            .await
            .map(KeycloakSession::parse_access_token)
    }

    pub async fn refresh_custom_realm(
        url: &str,
        realm: &str,
        client_id: &str,
        refresh_token: &str,
        client: &reqwest::Client,
    ) -> Result<KeycloakSession, KeycloakError> {
        let response = client
            .post(&format!(
                "{url}/realms/{realm}/protocol/openid-connect/token",
            ))
            .form(&serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": refresh_token,
                "client_id": client_id,
            }))
            .send()
            .await?;
        Ok(error_check(response).await?.json().await?)
    }
}

async fn error_check(response: reqwest::Response) -> Result<reqwest::Response, KeycloakError> {
    if !response.status().is_success() {
        let status = response.status().into();
        let text = response.text().await?;
        return Err(KeycloakError::HttpFailure {
            status,
            body: serde_json::from_str(&text).ok(),
            text,
        });
    }

    Ok(response)
}
pub type InflightRequestFuture =
    Pin<Box<dyn Future<Output = Result<(), RecvError>> + Send + Sync + 'static>>;
#[derive(Clone)]
pub struct AdminTokenSupplier {
    username: Arc<String>,
    password: Arc<String>,
    token: Arc<RwLock<Option<KeycloakSession>>>,
    token_future: Arc<RwLock<Option<InflightRequestFuture>>>,
}

impl AdminTokenSupplier {
    pub async fn new(
        url: &str,
        username: &str,
        password: &str,
        client: &reqwest::Client,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            username: Arc::new(username.to_string()),
            password: Arc::new(password.to_string()),
            token: Arc::new(RwLock::new(Some(
                KeycloakSession::acquire(url, username, password, client).await?,
            ))),
            token_future: Default::default(),
        })
    }

    pub async fn refresh(
        &self,
        url: &str,
        refresh_token: &str,
        client: &reqwest::Client,
    ) -> Result<(), KeycloakError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.token_future
            .write()
            .await
            .replace(Box::pin(Box::new(rx)));
        let next_token = match KeycloakSession::refresh(url, refresh_token, client).await {
            Ok(next_token) => next_token,
            Err(err) => {
                if let KeycloakError::HttpFailure { status, .. } = &err {
                    if *status == 400 {
                        log::debug!(
                            "refresh token expired try to acquire new token with credentials"
                        );
                        KeycloakSession::acquire(url, &self.username, &self.password, client)
                            .await?
                    } else {
                        return Err(err);
                    }
                } else {
                    return Err(err);
                }
            }
        };
        self.token.write().await.replace(next_token);
        tx.send(()).ok();
        Ok(())
    }
}

#[async_trait::async_trait]
impl KeycloakTokenSupplier for AdminTokenSupplier {
    async fn get(&self, _url: &str) -> Result<String, KeycloakError> {
        if let Some(token_future) = self.token_future.write().await.take() {
            token_future.await.ok();
        }
        if let Some(token) = self.token.read().await.as_ref() {
            log::debug!("Access Token:");
            Ok(token.access_token.clone())
        } else {
            Err(KeycloakError::HttpFailure {
                status: 401,
                body: None,
                text: "Unauthorized".into(),
            })
        }
    }
}

struct Inner {
    url: Arc<str>,
    config: KeycloakConfig,
    client: reqwest::Client,
    token_supplier: AdminTokenSupplier,
    admin: KeycloakAdmin<AdminTokenSupplier>,
}

#[derive(Default)]
pub struct KeycloakBuilder {
    no_refresh: bool,
    env_prefix: Option<&'static str>,
}

impl KeycloakBuilder {
    pub fn with_no_refresh(mut self) -> Self {
        self.no_refresh = true;
        self
    }

    pub fn with_env_prefix(mut self, prefix: &'static str) -> Self {
        self.env_prefix = Some(prefix);
        self
    }

    pub async fn build(self) -> anyhow::Result<Keycloak> {
        let mut config_builder = KeycloakConfig::builder();
        if let Some(prefix) = self.env_prefix {
            config_builder = config_builder.with_prefix(prefix);
        }
        let config = config_builder.build()?;
        let refresh_token_enabled = !self.no_refresh;
        let url: Arc<str> = Arc::from(config.address().to_string());
        let username: Arc<str> = Arc::from(config.username().to_string());
        let password: Arc<str> = Arc::from(config.password().to_string());
        let client = reqwest::Client::new();
        let token_supplier =
            AdminTokenSupplier::new(url.as_ref(), username.as_ref(), password.as_ref(), &client)
                .await?;
        let token_supplier_refresh = token_supplier.clone();
        if refresh_token_enabled {
            let refresh_url = url.to_string();
            let refresh_client = client.clone();
            let _refrest_passowrd = password.to_string();
            let _refrest_username = username.to_string();
            log::debug!("start token supplier");
            std::thread::spawn(move || {
                let rt = Builder::new_current_thread().enable_all().build().unwrap();
                let local = LocalSet::new();
                log::debug!("spawn local set");
                local.spawn_local(async move {
                    let url = refresh_url;
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                    log::debug!("loop forever");
                    loop {
                        interval.tick().await;
                        let local: DateTime<Local> = Local::now();
                        let mut used_refresh_token = None;
                        {
                            if let Some((
                                (parsed_access_token, refresh_token),
                                _refresh_expires_in,
                            )) =
                                token_supplier_refresh
                                    .token
                                    .read()
                                    .await
                                    .as_ref()
                                    .and_then(|t| {
                                        t.parsed_access_token
                                            .as_ref()
                                            .zip(t.refresh_token.as_ref())
                                            .zip(t.refresh_expires_in)
                                    })
                            {
                                let t = local.timestamp();
                                let exp = parsed_access_token.exp as i64;
                                let d = exp - t;
                                log::debug!("Token expires in {d}");
                                if d <= 30 {
                                    used_refresh_token = Some(refresh_token.to_owned())
                                }
                            } else {
                                log::debug!("unable to get parsed access token");
                            }
                        }
                        if let Some(refresh_token) = used_refresh_token {
                            log::debug!(
                                "Token will be invalid in 30 sec, going to use refresh token"
                            );
                            if let Err(e) = token_supplier_refresh
                                .refresh(&url, &refresh_token, &refresh_client)
                                .await
                            {
                                log::error!("An error occured {e:#?}");
                                std::process::exit(1);
                            }
                        }
                    }
                });
                rt.block_on(local);
            });
        }
        Ok(Keycloak {
            inner: Arc::new(Inner {
                url: url.clone(),
                config,
                client: client.clone(),
                token_supplier: token_supplier.clone(),
                admin: KeycloakAdmin::new(&url, token_supplier, client),
            }),
        })
    }
}

#[derive(Clone)]
pub struct Keycloak {
    inner: Arc<Inner>,
}

impl Keycloak {
    pub fn builder() -> KeycloakBuilder {
        KeycloakBuilder::default()
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.inner.client
    }

    pub async fn new() -> anyhow::Result<Self> {
        KeycloakBuilder::default().build().await
    }

    pub fn public_url(&self) -> &str {
        &self.inner.config.public_url()
    }

    pub fn config(&self) -> &KeycloakConfig {
        &self.inner.config
    }

    pub async fn users(
        &self,
        realm: &str,
        offset: Option<i32>,
        page_size: Option<i32>,
        search_query: Option<String>,
    ) -> Result<Vec<UserRepresentation>, KeycloakError> {
        self.inner
            .admin
            .realm_users_get(
                realm,
                None,
                None,
                None,
                None,
                None,
                offset,
                None,
                None,
                None,
                None,
                page_size,
                None,
                search_query,
                None,
            )
            .await
    }

    pub async fn user_roles_by_id(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<String>, KeycloakError> {
        Ok(self
            .inner
            .admin
            .realm_users_with_id_role_mappings_realm_composite_get(realm, user_id, Some(true))
            .await?
            .into_iter()
            .filter_map(|r| r.name)
            .collect())
    }

    pub async fn users_count(
        &self,
        realm: &str,
        search_query: Option<String>,
    ) -> Result<i32, KeycloakError> {
        self.inner
            .admin
            .realm_users_count_get(realm, None, None, None, None, None, search_query, None)
            .await
    }

    pub async fn create_realm(
        &self,
        realm_representation: RealmRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .post(realm_representation)
            .await
            .map_err(|e| {
                log::error!("{e:#?}");
                e
            })?;

        Ok(())
    }

    pub async fn remove_realm(&self, realm: &str) -> Result<(), KeycloakError> {
        self.inner.admin.realm_delete(realm).await
    }

    pub async fn remove_group(&self, realm: &str, id: &str) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_groups_with_id_delete(realm, id)
            .await
    }

    pub async fn remove_group_by_path(&self, realm: &str, path: &str) -> Result<(), KeycloakError> {
        let group = self
            .inner
            .admin
            .realm_group_by_path_with_path_get(realm, path)
            .await?;
        self.remove_group(realm, group.id.as_deref().unwrap()).await
    }

    pub async fn remove_role(&self, realm: &str, role_name: &str) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_roles_with_role_name_delete(realm, role_name)
            .await
    }

    pub async fn remove_role_by_id(&self, realm: &str, role_id: &str) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_roles_by_id_with_role_id_delete(realm, role_id)
            .await
    }

    pub async fn realms(&self) -> Result<Vec<String>, KeycloakError> {
        let builder = self
            .inner
            .client
            .get(format!("{}admin/realms", &self.inner.url));
        let response = builder
            .bearer_auth(self.inner.token_supplier.get(&self.inner.url).await?)
            .send()
            .await?;
        Ok(error_check(response)
            .await?
            .json::<Vec<ServerInfo>>()
            .await?
            .into_iter()
            .filter_map(|r| {
                if let Some(r) = r.realm {
                    match r.as_str() {
                        "master" => None,
                        _ => Some(r),
                    }
                } else {
                    None
                }
            })
            .collect())
    }

    pub async fn realm_by_name(&self, realm: &str) -> Result<RealmRepresentation, KeycloakError> {
        self.inner.admin.realm_get(realm).await
    }

    pub async fn update_realm_by_name(
        &self,
        realm: &str,
        rep: RealmRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner.admin.realm_put(realm, rep).await
    }

    pub async fn roles(&self, realm: &str) -> Result<Vec<RoleRepresentation>, KeycloakError> {
        self.inner
            .admin
            .realm_roles_get(realm, Some(true), None, None, None)
            .await
    }

    pub async fn all_roles(&self, realm: &str) -> Result<Vec<RoleRepresentation>, KeycloakError> {
        let page_offset = 1000;
        let mut offset = 0;
        let mut roles = vec![];
        loop {
            let result = self
                .inner
                .admin
                .realm_roles_get(realm, Some(true), Some(offset), Some(page_offset), None)
                .await?;
            if result.is_empty() {
                break;
            }
            offset += page_offset;
            roles.extend(result);
        }
        Ok(roles)
    }

    pub async fn realm_role_by_name(
        &self,
        realm: &str,
        role_name: &str,
    ) -> Result<RoleRepresentation, KeycloakError> {
        self.inner
            .admin
            .realm_roles_with_role_name_get(realm, role_name)
            .await
    }

    pub async fn create_role(
        &self,
        realm: &str,
        rep: RoleRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner.admin.realm_roles_post(realm, rep).await
    }

    pub async fn groups(&self, realm: &str) -> Result<Vec<GroupRepresentation>, KeycloakError> {
        self.inner
            .admin
            .realm_groups_get(realm, Some(false), None, None, None, None, None)
            .await
    }

    pub async fn groups_with_subgroups(
        &self,
        realm: &str,
    ) -> Result<Vec<GroupRepresentation>, KeycloakError> {
        let mut result = vec![];
        let groups = self.groups(realm).await?;
        for group in groups {
            let group = self
                .inner
                .admin
                .realm_groups_with_id_get(realm, group.id.as_deref().unwrap())
                .await?;
            if let Some(sub_groups) = group.sub_groups {
                result.extend(sub_groups);
            }
        }
        Ok(result)
    }

    pub async fn create_group(
        &self,
        realm: &str,
        rep: GroupRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner.admin.realm_groups_post(realm, rep).await
    }

    pub async fn group_by_path(
        &self,
        realm: &str,
        path: &str,
    ) -> Result<GroupRepresentation, KeycloakError> {
        self.inner
            .admin
            .realm_group_by_path_with_path_get(realm, path)
            .await
    }

    pub async fn role_members(
        &self,
        realm: &str,
        role_name: &str,
    ) -> Result<Vec<UserRepresentation>, KeycloakError> {
        self.inner
            .admin
            .realm_roles_with_role_name_users_get(realm, role_name, None, None)
            .await
    }

    pub async fn group_members(
        &self,
        realm: &str,
        path: &str,
    ) -> Result<Vec<UserRepresentation>, KeycloakError> {
        let g = self
            .inner
            .admin
            .realm_group_by_path_with_path_get(realm, path)
            .await?;
        self.inner
            .admin
            .realm_groups_with_id_members_get(realm, g.id.as_deref().unwrap(), None, None, None)
            .await
    }

    pub async fn create_sub_group(
        &self,
        realm: &str,
        id: &str,
        rep: GroupRepresentation,
    ) -> Result<(), KeycloakError> {
        if let Some(parent) = self.group_by_path(realm, id).await.ok().and_then(|g| g.id) {
            self.inner
                .admin
                .realm_groups_with_id_children_post(realm, &parent, rep)
                .await?;
        }
        Ok(())
    }

    pub async fn create_sub_group_with_id(
        &self,
        realm: &str,
        parent_id: &str,
        rep: GroupRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_groups_with_id_children_post(realm, parent_id, rep)
            .await?;
        Ok(())
    }

    pub async fn user_groups(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<GroupRepresentation>, KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_groups_get(realm, user_id, None, None, None, None)
            .await
    }

    pub async fn realm_role_mappings_by_group_id(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<RoleRepresentation>, KeycloakError> {
        self.inner
            .admin
            .realm_groups_with_id_role_mappings_realm_get(realm, id)
            .await
    }

    pub async fn create_realm_role_mappings_by_group_id(
        &self,
        realm: &str,
        id: &str,
        roles: Vec<RoleRepresentation>,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_groups_with_id_role_mappings_realm_post(realm, id, roles)
            .await
    }

    pub async fn user_by_id(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Option<UserRepresentation>, KeycloakError> {
        Ok(self
            .inner
            .admin
            .realm_users_with_id_get(realm, id)
            .await
            .ok())
    }

    pub async fn user_by_role(
        &self,
        realm: &str,
        role_name: &str,
    ) -> Result<Option<UserRepresentation>, KeycloakError> {
        Ok(self
            .inner
            .admin
            .realm_roles_with_role_name_users_get(realm, role_name, None, None)
            .await
            .ok()
            .and_then(|mut v| {
                if !v.is_empty() {
                    Some(v.remove(0))
                } else {
                    None
                }
            }))
    }

    pub async fn user_by_username(
        &self,
        realm: &str,
        username: String,
    ) -> Result<Option<UserRepresentation>, KeycloakError> {
        Ok(self
            .inner
            .admin
            .realm_users_get(
                realm,
                Some(false),
                None,
                None,
                None,
                Some(true),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(username),
            )
            .await
            .ok()
            .and_then(|mut v| {
                if !v.is_empty() {
                    Some(v.remove(0))
                } else {
                    None
                }
            }))
    }

    pub async fn info(&self, realm: &str) -> Result<RealmInfo, KeycloakError> {
        let builder = self
            .inner
            .client
            .get(format!("{}/realms/{realm}", &self.inner.url));
        let response = builder.send().await?;
        Ok(error_check(response).await?.json().await?)
    }

    pub async fn get_client(
        &self,
        realm: &str,
    ) -> Result<Option<ClientRepresentation>, KeycloakError> {
        Ok(self
            .inner
            .admin
            .realm_clients_get(
                realm,
                Some("spa".to_owned()),
                None,
                None,
                None,
                Some(true),
                Some(false),
            )
            .await?
            .pop())
    }

    pub async fn create_client(
        &self,
        realm: &str,
        rep: ClientRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner.admin.realm_clients_post(realm, rep).await?;
        Ok(())
    }

    pub async fn update_client(
        &self,
        realm: &str,
        id: &str,
        rep: ClientRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_clients_with_id_put(realm, id, rep)
            .await
    }

    pub async fn create_user(
        &self,
        realm: &str,
        user: UserRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner.admin.realm_users_post(realm, user).await?;
        Ok(())
    }

    pub async fn update_password(
        &self,
        realm: &str,
        user_id: &str,
        credential: CredentialRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_reset_password_put(realm, user_id, credential)
            .await?;
        Ok(())
    }

    pub async fn update_user(
        &self,
        realm: &str,
        user_id: &str,
        user: &UserRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_put(realm, user_id, user.to_owned())
            .await?;
        Ok(())
    }

    pub async fn add_user_to_group(
        &self,
        realm: &str,
        user_id: &str,
        group_id: &str,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_groups_with_group_id_put(realm, user_id, group_id)
            .await?;
        Ok(())
    }

    pub async fn add_user_role(
        &self,
        realm: &str,
        user_id: &str,
        role: RoleRepresentation,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_role_mappings_realm_post(realm, user_id, vec![role])
            .await
    }

    pub async fn remove_user_from_group(
        &self,
        realm: &str,
        user_id: &str,
        group_id: &str,
    ) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_groups_with_group_id_delete(realm, user_id, group_id)
            .await?;
        Ok(())
    }

    pub async fn remove_user(&self, realm: &str, user_id: &str) -> Result<(), KeycloakError> {
        self.inner
            .admin
            .realm_users_with_id_delete(realm, user_id)
            .await?;
        Ok(())
    }
}
