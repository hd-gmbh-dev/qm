use std::collections::HashMap;
use std::env;

use keycloak::types::{AuthenticatorConfigRepresentation, TypeMap};
use serde_json::Value;

use crate::{ClientRepresentation, RealmRepresentation};

use crate::validation::context::ValidationContext as Ctx;
use crate::validation::model::RealmConfigErrorInput;
use crate::validation::realm_errors;
pub async fn update_for_errors(
    ctx: &Ctx<'_>,
    errors: Vec<RealmConfigErrorInput>,
) -> anyhow::Result<()> {
    let realm = ctx.cfg().realm();
    let mut actions = errors;
    update_realm_settings(
        ctx,
        realm,
        actions
            .iter()
            .filter(|e| e.id.starts_with(realm_errors::REALM_PREFIX))
            .cloned()
            .collect(),
    )
    .await?;

    // Removing entries with the prefix
    // Could be simplified with nightly api [`drain_filter`](https://doc.rust-lang.org/std/vec/struct.DrainFilter.html)
    actions.retain(|e| !e.id.starts_with(realm_errors::REALM_PREFIX));

    update_client_settings(
        ctx,
        realm,
        actions
            .iter()
            .filter(|e| e.id.starts_with(realm_errors::CLIENTS_CLIENT_PREFIX))
            .cloned()
            .collect(),
    )
    .await?;
    actions.retain(|e| !e.id.starts_with(realm_errors::CLIENTS_CLIENT_PREFIX));

    update_autentication_flows(
        ctx,
        realm,
        actions
            .iter()
            .filter(|e| {
                e.id.starts_with(realm_errors::REALM_AUTHENTICATION_FLOWS_PREFIX)
            })
            .cloned()
            .collect(),
    )
    .await?;
    actions.retain(|e| {
        !e.id
            .starts_with(realm_errors::REALM_AUTHENTICATION_FLOWS_PREFIX)
    });

    update_browser_flow(
        ctx,
        realm,
        actions
            .iter()
            .filter(|e| e.id.starts_with(realm_errors::REALM_BROWSER_FLOW_PREFIX))
            .cloned()
            .collect(),
    )
    .await?;
    actions.retain(|e| !e.id.starts_with(realm_errors::REALM_BROWSER_FLOW_PREFIX));

    if !actions.is_empty() {
        log::error!(
            "Some unknown errors could not be resolved. Remaining: {:?}",
            actions
        );
        return Err(anyhow::Error::msg("Could not resolve all errors"));
    }

    Ok(())
}

async fn update_realm_settings(
    ctx: &Ctx<'_>,
    realm: &str,
    errors: Vec<RealmConfigErrorInput>,
) -> anyhow::Result<()> {
    if errors.is_empty() {
        log::info!("No realm errors in realm '{}'", realm);
        return Ok(());
    }

    let mut rep: RealmRepresentation = ctx.keycloak().realm_by_name(realm).await?;

    errors.iter().for_each(|e| match e.id.as_str() {
        realm_errors::REALM_DEFAULT_LOCALE_INVALID_ID
        | realm_errors::REALM_DEFAULT_LOCALE_MISSING_ID => {
            log::trace!("Setting 'default_locale' for realm '{}'", realm);
            rep.default_locale = Some("de".to_string());
        }
        realm_errors::REALM_INTERNATIONALIZATION_ENABLED_ID => {
            log::trace!(
                "Setting 'internationalization_enabled' for realm '{}'",
                realm
            );
            rep.internationalization_enabled = Some(true);
        }
        realm_errors::REALM_LOGIN_THEME_INVALID_ID | realm_errors::REALM_LOGIN_THEME_MISSING_ID => {
            log::trace!("Setting 'login_theme' for realm '{}'", realm);
            rep.login_theme = Some(ctx.cfg().keycloak().theme().to_string());
        }
        realm_errors::REALM_EMAIL_THEME_INVALID_ID | realm_errors::REALM_EMAIL_THEME_MISSING_ID => {
            log::trace!("Setting 'email_theme' for realm '{}'", realm);
            rep.email_theme = Some(ctx.cfg().keycloak().email_theme().to_string());
        }
        realm_errors::REALM_PASSWORD_POLICY_LENGTH_ID => {
            log::trace!(
                "Adding 'password_policy' value 'length(8)' for realm '{}'",
                realm
            );
            let new_policy = match &rep.password_policy {
                Some(s) => format!("{} and length(8)", s),
                None => "length(8)".to_string(),
            };
            rep.password_policy = Some(new_policy)
        }
        realm_errors::REALM_PASSWORD_POLICY_SYMBOL_ID => {
            log::trace!(
                "Adding 'password_policy' value 'specialChars(1)' for realm '{}'",
                realm
            );
            let new_policy = match &rep.password_policy {
                Some(s) => format!("{} and specialChars(1)", s),
                None => "specialChars(1)".to_string(),
            };
            rep.password_policy = Some(new_policy)
        }
        realm_errors::REALM_PASSWORD_POLICY_UPPERCASE_ID => {
            log::trace!(
                "Adding 'password_policy' value 'upperCase(1)' for realm '{}'",
                realm
            );
            let new_policy = match &rep.password_policy {
                Some(s) => format!("{} and upperCase(1)", s),
                None => "upperCase(1)".to_string(),
            };
            rep.password_policy = Some(new_policy)
        }
        realm_errors::REALM_PASSWORD_POLICY_LOWERCASE_ID => {
            log::trace!(
                "Adding 'password_policy' value 'lowerCase(1)' for realm '{}'",
                realm
            );
            let new_policy = match &rep.password_policy {
                Some(s) => format!("{} and lowerCase(1)", s),
                None => "lowerCase(1)".to_string(),
            };
            rep.password_policy = Some(new_policy)
        }
        realm_errors::REALM_PASSWORD_POLICY_DIGIT_ID => {
            log::trace!(
                "Adding 'password_policy' value 'digits(1)' for realm '{}'",
                realm
            );
            let new_policy = match &rep.password_policy {
                Some(s) => format!("{} and digits(1)", s),
                None => "digits(1)".to_string(),
            };
            rep.password_policy = Some(new_policy)
        }
        realm_errors::REALM_PASSWORD_POLICY_MISSING_ID => {
            log::trace!("Setting 'password_policy' for realm '{}'", realm);
            rep.password_policy = Some(
                "length(8) and specialChars(1) and upperCase(1) and lowerCase(1) and digits(1)"
                    .to_string(),
            )
        }
        realm_errors::REALM_REMEMBER_ME_ID => {
            log::trace!("Setting 'remember_me' for realm '{}'", realm);
            rep.remember_me = Some(true);
        }
        realm_errors::REALM_REGISTRATION_ALLOWED_ID => {
            log::trace!("Setting 'registration_allowed' for realm '{}'", realm);
            rep.registration_allowed = Some(false);
        }
        realm_errors::REALM_RESET_PASSWORD_ALLOWED_ID => {
            log::trace!("Setting 'reset_password_allowed' for realm '{}'", realm);
            rep.reset_password_allowed = Some(true);
        }
        realm_errors::REALM_SUPPORTED_LOCALES_INVALID_ID
        | realm_errors::REALM_SUPPORTED_LOCALES_MISSING_ID => {
            log::trace!("Setting 'supported_locales' for realm '{}'", realm);
            rep.supported_locales = Some(vec!["de".to_string()]);
        }
        realm_errors::REALM_SMTP_SERVER_MISSING_ID => {
            log::trace!("Setting 'smtp_server' for realm '{}'", realm);
            rep.smtp_server = get_smtp_server_defaults(ctx)
        }
        realm_errors::REALM_SMTP_SERVER_REPLY_TO_DISPLAY_NAME_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_REPLY_TO_DISPLAY_NAME_MISMATCHED_ID => {
            log::trace!(
                "Setting 'smtp_server.replyToDisplayName' for realm '{}'",
                realm
            );
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("replyToDisplayName"),
                ctx.cfg()
                    .keycloak()
                    .smtp_reply_to_display_name()
                    .unwrap()
                    .to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_STARTTLS_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_STARTTLS_MISMATCHED_ID
        | realm_errors::REALM_SMTP_SERVER_STARTTLS_INVALID_ID => {
            log::trace!("Setting 'smtp_server.starttls' for realm '{}'", realm);
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("starttls"),
                ctx.cfg().keycloak().smtp_starttls().unwrap().to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_PORT_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_PORT_MISMATCHED_ID
        | realm_errors::REALM_SMTP_SERVER_PORT_INVALID_ID => {
            log::trace!("Setting 'smtp_server.port' for realm '{}'", realm);
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("port"),
                ctx.cfg().keycloak().smtp_port().unwrap().to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_HOST_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_HOST_MISMATCHED_ID
        | realm_errors::REALM_SMTP_SERVER_HOST_INVALID_ID => {
            log::trace!("Setting 'smtp_server.host' for realm '{}'", realm);
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("host"),
                ctx.cfg().keycloak().smtp_host().unwrap().to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_REPLY_TO_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_REPLY_TO_MISMATCHED_ID => {
            log::trace!("Setting 'smtp_server.replyTo' for realm '{}'", realm);
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("replyTo"),
                ctx.cfg().keycloak().smtp_reply_to().unwrap().to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_FROM_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_FROM_MISMATCHED_ID
        | realm_errors::REALM_SMTP_SERVER_FROM_INVALID_ID => {
            log::trace!("Setting 'smtp_server.from' for realm '{}'", realm);
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("from"),
                ctx.cfg().keycloak().smtp_from().unwrap().to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_FROM_DISPLAY_NAME_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_FROM_DISPLAY_NAME_MISMATCHED_ID => {
            log::trace!(
                "Setting 'smtp_server.fromDisplayName' for realm '{}'",
                realm
            );
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("fromDisplayName"),
                ctx.cfg()
                    .keycloak()
                    .smtp_from_display_name()
                    .unwrap()
                    .to_string(),
            );
        }
        realm_errors::REALM_SMTP_SERVER_SSL_MISSING_ID
        | realm_errors::REALM_SMTP_SERVER_SSL_MISMATCHED_ID
        | realm_errors::REALM_SMTP_SERVER_SSL_INVALID_ID => {
            log::trace!("Setting 'smtp_server.ssl' for realm '{}'", realm);
            rep.smtp_server.as_mut().unwrap().insert(
                String::from("ssl"),
                ctx.cfg().keycloak().smtp_ssl().unwrap().to_string(),
            );
        }
        _ => log::warn!("Unknown realm error id '{}'. No action taken.", e.id),
    });

    log::info!(
        "Updating the realm '{}' with the following representation: {:?}",
        realm,
        rep
    );
    ctx.keycloak().update_realm_by_name(realm, rep).await?;
    Ok(())
}

async fn update_autentication_flows(
    ctx: &Ctx<'_>,
    realm: &str,
    errors: Vec<RealmConfigErrorInput>,
) -> anyhow::Result<()> {
    if errors.is_empty() {
        log::info!("No autentication_flows errors in realm '{}'", realm);
        return Ok(());
    }

    for e in errors {
        match e.id.as_str() {
            realm_errors::REALM_AUTHENTICATION_FLOWS_MISSING_ID
            | realm_errors::REALM_AUTHENTICATION_FLOWS_MISSING_KEY => {
                log::info!(
                    "Setting autentication_flow 'browser_email_otp' for realm '{}'",
                    realm
                );

                // 1) Duplicate browser flow
                let mut body_duplicate = TypeMap::new();
                body_duplicate.insert("newName".to_string(), "browser_email_otp".to_string());
                ctx.keycloak()
                    .copy_authentication_flow(realm, "browser", body_duplicate)
                    .await?;

                // 2) Get executions for flow "browser_email_otp"
                let executions = ctx
                    .keycloak()
                    .get_flow_executions(realm, "browser_email_otp")
                    .await?;

                let browser_conditional_otp_id = executions
                    .iter()
                    .find(|&execution| {
                        execution.display_name
                            == Some("browser_email_otp Browser - Conditional OTP".to_string())
                    })
                    .unwrap()
                    .id
                    .as_deref()
                    .unwrap_or("");

                // 3) Remove Execution "browser_email_otp Browser - Conditional OTP"

                ctx.keycloak()
                    .remove_execution(realm, browser_conditional_otp_id)
                    .await?;

                // 4) Create "Email_2FA" subflow in "browser_email_otp forms"
                let mut body_subflow: HashMap<String, Value> = HashMap::new();
                body_subflow.insert(
                    "alias".to_string(),
                    serde_json::Value::String("Email_2FA".to_string()),
                );
                body_subflow.insert(
                    "description".to_string(),
                    serde_json::Value::String("Email_2FA".to_string()),
                );
                body_subflow.insert(
                    "provider".to_string(),
                    serde_json::Value::String("registration-page-form".to_string()),
                );
                body_subflow.insert(
                    "type".to_string(),
                    serde_json::Value::String("basic-flow".to_string()),
                );
                ctx.keycloak()
                    .create_subflow(realm, "browser_email_otp%20forms", body_subflow)
                    .await?;

                // 5) Get executions for flow ""browser_email_otp"

                let executions2 = ctx
                    .keycloak()
                    .get_flow_executions(realm, "browser_email_otp")
                    .await?;

                // 6) Change requirement of "browser_email_otp_forms"
                let mut browser_email_otp_form_execution = executions2
                    .iter()
                    .find(|&execution| {
                        execution.display_name == Some("browser_email_otp forms".to_string())
                    })
                    .unwrap()
                    .clone();
                browser_email_otp_form_execution.requirement = Some("REQUIRED".to_string());

                ctx.keycloak()
                    .modify_flow_execution(
                        realm,
                        "browser_email_otp",
                        browser_email_otp_form_execution,
                    )
                    .await?;

                // 7) Change requirement of "Email_2FA" execution
                let mut email_2fa_execution = executions2
                    .iter()
                    .find(|&execution| execution.display_name == Some("Email_2FA".to_string()))
                    .unwrap()
                    .clone();
                email_2fa_execution.requirement = Some("REQUIRED".to_string());
                ctx.keycloak()
                    .modify_flow_execution(realm, "browser_email_otp", email_2fa_execution)
                    .await?;

                // 8) Create execution "emailotp-authenticator" in "Email_2FA"
                let mut body_execution: HashMap<String, Value> = HashMap::new();
                body_execution.insert(
                    "provider".to_string(),
                    serde_json::Value::String("emailotp-authenticator".to_string()),
                );
                ctx.keycloak()
                    .create_flow_execution(realm, "Email_2FA", body_execution)
                    .await?;

                // 9) Get executions for flow "browser_email_otp"
                let executions3 = ctx
                    .keycloak()
                    .get_flow_executions(realm, "browser_email_otp")
                    .await?;

                // 10) Change requirement of "emailotp_autenticator"  execution
                let mut email_totp_auth_execution = executions3
                    .iter()
                    .find(|&execution| {
                        execution.display_name == Some("Email TOTP Authentication".to_string())
                    })
                    .unwrap()
                    .clone();
                let email_totp_exec_id = email_totp_auth_execution.id.as_deref().unwrap();

                email_totp_auth_execution.requirement = Some("REQUIRED".to_string());
                ctx.keycloak()
                    .modify_flow_execution(
                        realm,
                        "browser_email_otp",
                        email_totp_auth_execution.clone(),
                    )
                    .await?;

                // 11) Add configuration to "browser_email_otp" execution

                let mut config: HashMap<String, String> = HashMap::new();
                config.insert("default.reference.value".to_string(), "".to_string());
                config.insert("default.reference.maxAge".to_string(), "".to_string());
                config.insert("simulation".to_string(), "false".to_string());
                config.insert(
                    "emailSubject".to_string(),
                    ctx.cfg()
                        .keycloak()
                        .authenticator_email_subject()
                        .unwrap_or("Temporary Authentication Code")
                        .to_string(),
                );
                config.insert("length".to_string(), "6".to_string());
                config.insert("ttl".to_string(), "300".to_string());
                config.insert("maxRetries".to_string(), "3".to_string());
                config.insert("allowUppercase".to_string(), "true".to_string());
                config.insert("true".to_string(), "true".to_string());
                config.insert("true".to_string(), "true".to_string());

                let body_config = AuthenticatorConfigRepresentation {
                    alias: Some("email_otp_flow".to_string()),
                    config: Some(config),
                    ..AuthenticatorConfigRepresentation::default()
                };
                ctx.keycloak()
                    .add_authenticator_config(realm, email_totp_exec_id, body_config)
                    .await?;
            }
            _ => log::warn!(
                "Unknown create_authentication_flow error id '{}'. No action taken.",
                e.id
            ),
        }
    }
    Ok(())
}

async fn update_browser_flow(
    ctx: &Ctx<'_>,
    realm: &str,
    errors: Vec<RealmConfigErrorInput>,
) -> anyhow::Result<()> {
    if errors.is_empty() {
        log::info!("No realm errors in realm '{}'", realm);
        return Ok(());
    }

    let mut rep: RealmRepresentation = ctx.keycloak().realm_by_name(realm).await?;

    errors.iter().for_each(|e| match e.id.as_str() {
        realm_errors::REALM_BROWSER_FLOW_INVALID_ID
        | realm_errors::REALM_BROWSER_FLOW_MISSING_ID => {
            log::trace!("Setting 'browser_flow' for realm '{}'", realm);
            rep.browser_flow = Some(ctx.cfg().keycloak().browser_flow().to_string());
        }
        _ => log::warn!("Unknown browser_flow error id '{}'. No action taken.", e.id),
    });
    ctx.keycloak().update_realm_by_name(realm, rep).await?;
    Ok(())
}

async fn update_client_settings(
    ctx: &Ctx<'_>,
    realm: &str,
    errors: Vec<RealmConfigErrorInput>,
) -> anyhow::Result<()> {
    if errors.is_empty() {
        log::info!("No client errors in realm '{}'", realm);
        return Ok(());
    }

    let mut client: Option<ClientRepresentation> = ctx
        .keycloak()
        .get_client(realm) // Hardcoded only gets `spa`
        .await?;

    if let Some(rep) = client.as_mut() {
        rep.direct_access_grants_enabled = Some(true);
        errors.iter().for_each(|e| {
            match e.id.as_str() {
                realm_errors::CLIENTS_CLIENT_ATTRIBUTES_OAUTH2_DEVICE_AUTHORIZATION_GRANT_ENABLED_INVALID_ID
                | realm_errors::CLIENTS_CLIENT_ATTRIBUTES_OAUTH2_DEVICE_AUTHORIZATION_GRANT_ENABLED_MISSING_ID
                | realm_errors::CLIENTS_CLIENT_ATTRIBUTES_MISSING_ID
                | realm_errors::CLIENTS_CLIENT_ATTRIBUTES_BACKCHANNEL_LOGOUT_DISABLED_ID => {
                    if let Some(attributes) = rep.attributes.as_mut() {
                        match e.id.as_str() {
                            realm_errors::CLIENTS_CLIENT_ATTRIBUTES_BACKCHANNEL_LOGOUT_DISABLED_ID => {
                                log::trace!("Setting attribute 'backchannel.logout.url' for client 'spa' in realm '{}'", realm);
                                let backchannel_logout_url = env::var("BACKCHANNEL_LOGOUT_URL").unwrap_or("http://qm-backend:10220/api/logout".to_string());
                                attributes.insert("backchannel.logout.url".to_string(), backchannel_logout_url.to_string());
                            },
                            _ => {
                                log::trace!("Setting attribute 'oauth2.device.authorization.grant.enabled' for client 'spa' in realm '{}'", realm);
                                attributes.insert("oauth2.device.authorization.grant.enabled".to_string(), "false".to_string());}
                            }
                    } else {
                        rep.attributes = Some(HashMap::from_iter(vec![("oauth2.device.authorization.grant.enabled".to_string(), "false".to_string()),
                        ("backchannel.logout.url".to_string(), "http://qm-backend:10220/api/logout".to_string())]))
                    }
                }
                realm_errors::CLIENTS_CLIENT_BASE_URL_INVALID_ID
                | realm_errors::CLIENTS_CLIENT_BASE_URL_MISSING_ID => {
                    log::trace!("Setting 'registration_allowed' for client 'spa' in realm '{}'", realm);
                    rep.base_url = Some(ctx.cfg().public_url().trim_end_matches('/').to_string());
                }
                realm_errors::CLIENTS_CLIENT_CLIENT_ID_ID => {
                    log::trace!("Setting 'client_id' for client 'spa' in realm '{}'", realm);
                    rep.client_id = Some("spa".to_string());
                }
                realm_errors::CLIENTS_CLIENT_CONSENT_REQUIRED_ID => {
                    log::trace!("Setting 'consent_required' for client 'spa' in realm '{}'", realm);
                    rep.consent_required = Some(false);
                }
                realm_errors::CLIENTS_CLIENT_DIRECT_ACCESS_GRANT_ENABLED_ID => {
                    log::trace!("Setting 'direct_access_grants_enabled' for client 'spa' in realm '{}'", realm);
                    rep.direct_access_grants_enabled = Some(false);
                }
                realm_errors::CLIENTS_CLIENT_ENABLED_ID => {
                    log::trace!("Setting 'enabled'");
                    rep.enabled = Some(true);
                }
                realm_errors::CLIENTS_CLIENT_IMPLICIT_FLOW_ENABLED_ID => {
                    log::trace!("Setting 'implicit_flow_enabled' for client 'spa' in realm '{}'", realm);
                    rep.implicit_flow_enabled = Some(false);
                }
                realm_errors::CLIENTS_CLIENT_PUBLIC_CLIENT_ID => {
                    log::trace!("Setting 'public_client' for client 'spa' in realm '{}'", realm);
                    rep.public_client = Some(true);
                }
                realm_errors::CLIENTS_CLIENT_REDIRECT_URIS_INVALID_ID
                | realm_errors::CLIENTS_CLIENT_REDIRECT_URIS_MISSING_ID => {
                    log::trace!("Adding 'redirect_uris' for configured value for client 'spa' in realm '{}'", realm);
                    if let Some(uris) = rep.redirect_uris.as_mut() {
                        uris.clear();
                        uris.push(ctx.cfg().public_url().to_string());
                        uris.push(format!("{}*", ctx.cfg().public_url()));
                    } else {
                        rep.redirect_uris = Some(vec![format!("{}*", ctx.cfg().public_url())]);
                    }
                }
                realm_errors::CLIENTS_CLIENT_ROOT_URL_INVALID_ID
                | realm_errors::CLIENTS_CLIENT_ROOT_URL_MISSING_ID => {
                    log::trace!("Setting 'root_url' for client 'spa' in realm '{}'", realm);
                    rep.root_url = Some(ctx.cfg().public_url().trim_end_matches('/').to_string());
                }
                realm_errors::CLIENTS_CLIENT_SERVICE_ACCOUNTS_ENABLED_ID => {
                    log::trace!("Setting 'service_accounts_enabled' for client 'spa' in realm '{}'", realm);
                    rep.service_accounts_enabled = Some(false);
                }
                realm_errors::CLIENTS_CLIENT_STANDARD_FLOW_ENABLED_ID => {
                    log::trace!("Setting 'standard_flow_enabled' for client 'spa' in realm '{}'", realm);
                    rep.standard_flow_enabled = Some(true);
                }
                realm_errors::CLIENTS_CLIENT_FRONTCHANNEL_LOGOUT_ENABLED_ID => {
                    log::trace!("Setting 'front_channel_logout' for client 'spa' in realm '{}'", realm);
                    rep.frontchannel_logout = Some(false);
                }
                _ => log::warn!("Unknown client error id '{}'. No action taken.", e.id),
            }
        });

        log::info!(
            "Updating the client 'spa' for realm '{}' with the following representation: {:?}",
            realm,
            rep
        );
        ctx.keycloak()
            .update_client(realm, rep.id.as_ref().unwrap(), rep.clone())
            .await?;
    } else {
        let rep = ClientRepresentation {
            attributes: Some(HashMap::from_iter(vec![
                (
                    "oauth2.device.authorization.grant.enabled".to_string(),
                    "false".to_string(),
                ),
                (
                    "backchannel.logout.url".to_string(),
                    "http://qm-backend:10220/api/logout".to_string(),
                ),
            ])),
            base_url: Some(ctx.cfg().public_url().trim_end_matches('/').to_string()),
            client_id: Some("spa".to_string()),
            consent_required: Some(false),
            direct_access_grants_enabled: Some(true),
            enabled: Some(true),
            implicit_flow_enabled: Some(false),
            public_client: Some(true),
            redirect_uris: Some(vec![format!("{}*", ctx.cfg().public_url())]),
            root_url: Some(ctx.cfg().public_url().trim_end_matches('/').to_string()),
            service_accounts_enabled: Some(false),
            standard_flow_enabled: Some(true),
            frontchannel_logout: Some(false),
            ..ClientRepresentation::default()
        };

        log::info!(
            "Could not find required client 'spa' for realm '{}'. Creating with the following representation: {:?}",
            realm,
            rep
        );
        ctx.keycloak().create_client(realm, rep).await?;
    }
    Ok(())
}

pub fn get_smtp_server_defaults(ctx: &Ctx<'_>) -> Option<HashMap<String, String>> {
    let mut defaults: HashMap<String, String> = HashMap::new();

    if let Some(configured_starttls) = ctx.cfg().keycloak().smtp_starttls() {
        defaults.insert(String::from("starttls"), configured_starttls.to_string());
    } else {
        defaults.insert(String::from("starttls"), "false".to_string());
    }
    if let Some(configured_port) = ctx.cfg().keycloak().smtp_port() {
        defaults.insert(String::from("port"), configured_port.to_string());
    } else {
        defaults.insert(String::from("port"), "1025".to_string());
    }
    if let Some(configured_host) = ctx.cfg().keycloak().smtp_host() {
        defaults.insert(String::from("host"), configured_host.to_string());
    } else {
        defaults.insert(String::from("host"), "smtp".to_string());
    }
    if let Some(configured_from) = ctx.cfg().keycloak().smtp_from() {
        defaults.insert(String::from("from"), configured_from.to_string());
    } else {
        defaults.insert(String::from("from"), "noreply@qm.local".to_string());
    }
    if let Some(configured_from) = ctx.cfg().keycloak().smtp_from_display_name() {
        defaults.insert(String::from("fromDisplayName"), configured_from.to_string());
    } else {
        defaults.insert(String::from("fromDisplayName"), "qm".to_string());
    }
    if let Some(configured_ssl) = ctx.cfg().keycloak().smtp_ssl() {
        defaults.insert(String::from("ssl"), configured_ssl.to_owned().to_string());
    } else {
        defaults.insert(String::from("ssl"), "false".to_string());
    }

    Some(defaults)
}
