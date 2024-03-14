use std::hash::{Hash, Hasher};

use async_graphql::{ComplexObject, Context};
use async_graphql::{Enum, InputObject, SimpleObject};
use qm_entity::ctx::ContextFilterInput;

use qm_entity::error::EntityError;
use qm_entity::error::EntityResult;
use qm_entity::ids::{
    CustomerId, CustomerResourceId, EntityId, InstitutionId, OrganizationId,
    OrganizationResourceId, OrganizationUnitId,
};
use qm_entity::list::NewList;
use qm_entity::model::Modification;
use qm_entity::Create;
use qm_entity::UserId;
use qm_keycloak::UserRepresentation;
use qm_mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "ty", content = "entityId")]
pub enum Owner {
    Customer(EntityId),
    Organization(EntityId),
    Institution(EntityId),
    OrganizationUnit(EntityId),
}

impl From<ContextFilterInput> for Owner {
    fn from(value: ContextFilterInput) -> Self {
        match value {
            ContextFilterInput::Customer(v) => Owner::Customer(v.into()),
            ContextFilterInput::Organization(v) => Owner::Organization(v.into()),
            ContextFilterInput::Institution(v) => Owner::Institution(v.into()),
            ContextFilterInput::OrganizationUnit(v) => Owner::OrganizationUnit(v.into()),
        }
    }
}

impl Owner {
    pub fn customer(&self) -> Option<CustomerId> {
        match &self {
            Owner::Customer(EntityId { cid: Some(cid), .. }) => {
                Some(CustomerId { id: cid.clone() })
            }
            Owner::Organization(EntityId { cid: Some(cid), .. }) => {
                Some(CustomerId { id: cid.clone() })
            }
            Owner::OrganizationUnit(EntityId { cid: Some(cid), .. }) => {
                Some(CustomerId { id: cid.clone() })
            }
            Owner::Institution(EntityId { cid: Some(cid), .. }) => {
                Some(CustomerId { id: cid.clone() })
            }
            _ => None,
        }
    }
    pub fn organization(&self) -> Option<OrganizationId> {
        match &self {
            Owner::Organization(EntityId {
                cid: Some(cid),
                oid: Some(oid),
                ..
            }) => Some(OrganizationId {
                cid: cid.clone(),
                id: oid.clone(),
            }),
            Owner::OrganizationUnit(EntityId {
                cid: Some(cid),
                oid: Some(oid),
                ..
            }) => Some(OrganizationId {
                cid: cid.clone(),
                id: oid.clone(),
            }),
            Owner::Institution(EntityId {
                cid: Some(cid),
                oid: Some(oid),
                ..
            }) => Some(OrganizationId {
                cid: cid.clone(),
                id: oid.clone(),
            }),
            _ => None,
        }
    }
    pub fn organization_unit(&self) -> Option<OrganizationUnitId> {
        match &self {
            Owner::OrganizationUnit(EntityId {
                cid: Some(cid),
                oid: Some(oid),
                iid: Some(iid),
                ..
            }) => Some(OrganizationUnitId::Organization(OrganizationResourceId {
                id: iid.clone(),
                oid: oid.clone(),
                cid: cid.clone(),
            })),
            Owner::OrganizationUnit(EntityId {
                cid: Some(cid),
                oid: None,
                iid: Some(iid),
                ..
            }) => Some(OrganizationUnitId::Customer(CustomerResourceId {
                id: iid.clone(),
                cid: cid.clone(),
            })),
            _ => None,
        }
    }
    pub fn institution(&self) -> Option<InstitutionId> {
        match &self {
            Owner::Institution(EntityId {
                cid: Some(cid),
                oid: Some(oid),
                iid: Some(iid),
                ..
            }) => Some(InstitutionId {
                cid: cid.clone(),
                oid: oid.clone(),
                id: iid.clone(),
            }),
            _ => None,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Enum, Copy, Eq, PartialEq)]
pub enum RequiredUserAction {
    #[graphql(name = "UPDATE_PASSWORD")]
    UpdatePassword,
}

impl std::fmt::Display for RequiredUserAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RequiredUserAction::UpdatePassword => "UPDATE_PASSWORD",
        }
        .to_string();
        write!(f, "{}", str)
    }
}
use std::collections::HashMap;

use crate::cache::Cache;

use super::{Customer, Institution, Organization, OrganizationUnit};
fn get_attribute(
    attributes: Option<&HashMap<String, serde_json::Value>>,
    key: &'static str,
) -> Option<Arc<str>> {
    attributes.and_then(|a| {
        a.get(key).and_then(|v| match v {
            serde_json::Value::String(s) => Some(Arc::from(s.to_string())),
            serde_json::Value::Array(arr) => arr.first().and_then(|v| {
                if let serde_json::Value::String(s) = v {
                    Some(Arc::from(s.to_string()))
                } else {
                    None
                }
            }),
            _ => None,
        })
    })
}

#[derive(Default, serde::Deserialize, serde::Serialize, Debug, Clone, InputObject)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserInput {
    pub username: String,
    pub firstname: String,
    pub lastname: String,
    pub password: String,
    pub email: String,
    pub phone: Option<String>,
    pub salutation: Option<String>,
    pub room_number: Option<String>,
    pub job_title: Option<String>,
    pub enabled: Option<bool>,
    pub required_actions: Option<Vec<RequiredUserAction>>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct UserDetails {
    #[serde(rename = "_id")]
    #[graphql(name = "id")]
    pub user_id: Arc<Uuid>,
    pub firstname: Arc<str>,
    pub lastname: Arc<str>,
    pub username: Arc<str>,
    pub email: Arc<str>,
    pub phone: Option<Arc<str>>,
    pub salutation: Option<Arc<str>>,
    pub job_title: Option<Arc<str>>,
    pub enabled: bool,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, SimpleObject)]
#[serde(rename_all = "camelCase")]
#[graphql(complex)]
pub struct User {
    #[graphql(skip)]
    pub owner: Owner,
    #[serde(default)]
    #[graphql(skip)]
    pub groups: Vec<String>,
    #[graphql(skip)]
    pub access: String,
    #[serde(default)]
    #[graphql(skip)]
    pub custom_groups: Vec<String>, // TODO: implement custom groups
    #[serde(flatten)]
    #[graphql(flatten)]
    pub details: Arc<UserDetails>,
    #[graphql(skip)]
    pub created: Modification,
    #[graphql(skip)]
    pub modified: Option<Modification>,
}

#[ComplexObject]
impl User {
    async fn customer(&self, ctx: &Context<'_>) -> Option<Arc<Customer>> {
        if let Some((cache, id)) = ctx.data::<Cache>().ok().zip(self.owner.customer()) {
            cache.customer().customer_by_id(id.as_ref()).await
        } else {
            log::warn!("qm::customer::Cache is not installed in schema context");
            None
        }
    }

    async fn organization(&self, ctx: &Context<'_>) -> Option<Arc<Organization>> {
        if let Some((cache, id)) = ctx.data::<Cache>().ok().zip(self.owner.organization()) {
            cache.customer().organization_by_id(&id).await
        } else {
            log::warn!("qm::customer::Cache is not installed in schema context");
            None
        }
    }

    async fn institution(&self, ctx: &Context<'_>) -> Option<Arc<Institution>> {
        if let Some((cache, id)) = ctx.data::<Cache>().ok().zip(self.owner.institution()) {
            cache.customer().institution_by_id(&id).await
        } else {
            log::warn!("qm::customer::Cache is not installed in schema context");
            None
        }
    }

    async fn organization_unit(&self, ctx: &Context<'_>) -> Option<Arc<OrganizationUnit>> {
        if let Some((cache, id)) = ctx.data::<Cache>().ok().zip(self.owner.organization_unit()) {
            cache.customer().organization_unit_by_id(&id).await
        } else {
            log::warn!("qm::customer::Cache is not installed in schema context");
            None
        }
    }
}

impl AsMut<EntityId> for User {
    fn as_mut(&mut self) -> &mut EntityId {
        match &mut self.owner {
            Owner::Customer(v) => v,
            Owner::Organization(v) => v,
            Owner::Institution(v) => v,
            Owner::OrganizationUnit(v) => v,
        }
    }
}

pub struct UserData {
    pub owner: Owner,
    pub groups: Vec<String>,
    pub details: UserDetails,
    pub access: String,
}

impl<C> Create<User, C> for UserData
where
    C: UserId,
{
    fn create(self, c: &C) -> EntityResult<User> {
        let user_id = c.user_id().ok_or(EntityError::Forbidden)?.to_owned();
        Ok(User {
            owner: self.owner,
            groups: self.groups,
            access: self.access,
            custom_groups: Default::default(),
            details: Arc::new(self.details),
            created: Modification::new(user_id),
            modified: None,
        })
    }
}

impl TryFrom<UserRepresentation> for UserDetails {
    type Error = anyhow::Error;
    fn try_from(value: UserRepresentation) -> Result<Self, Self::Error> {
        let user_id = Arc::new(
            value
                .id
                .and_then(|id| Uuid::parse_str(id).ok())
                .ok_or(anyhow::anyhow!("unable to get user id"))?,
        );
        Ok(Self {
            user_id,
            firstname: Arc::from(value.first_name.unwrap_or_default()),
            lastname: Arc::from(value.last_name.unwrap_or_default()),
            username: Arc::from(value.username.unwrap_or_default()),
            email: Arc::from(value.email.unwrap_or_default()),
            phone: get_attribute(value.attributes.as_ref(), "phone"),
            salutation: get_attribute(value.attributes.as_ref(), "salutation"),
            job_title: get_attribute(value.attributes.as_ref(), "job-title"),
            enabled: value.enabled.unwrap_or_default(),
        })
    }
}

// #[ComplexObject]
// impl User {
//     pub async fn customer(&self, ctx: &Context<'_>) -> Option<Arc<Customer>> {
//         let store = ctx.data_unchecked::<CustomerCache>();
//         let Some(cid) = self.rid.cid.as_deref() else {
//             return None;
//         };
//         store.customer_by_id(cid).await
//     }

//     pub async fn organization(&self, ctx: &Context<'_>) -> Option<Arc<Organization>> {
//         let store = ctx.data_unchecked::<CustomerStore>();
//         let Some(cid) = self.rid.cid.clone() else {
//             return None;
//         };
//         let Some(oid) = self.rid.oid.clone() else {
//             return None;
//         };
//         store
//             .organization_by_id(&CustomerShardedId { cid, id: oid })
//             .await
//     }

//     pub async fn institution(&self, ctx: &Context<'_>) -> Option<Arc<Institution>> {
//         let store = ctx.data_unchecked::<CustomerStore>();
//         let Some(cid) = self.rid.cid.clone() else {
//             return None;
//         };
//         let Some(oid) = self.rid.oid.clone() else {
//             return None;
//         };
//         let Some(iid) = self.rid.iid.clone() else {
//             return None;
//         };
//         store
//             .institution_by_id(&OrganizationShardedId { cid, oid, id: iid })
//             .await
//     }

//     // TODO: deliver access level
//     // pub async fn access(
//     //     &self,
//     //     ctx: &Context<'_>,
//     // ) -> Result<AccessLevel>, async_graphql::FieldError> {
//     //     let store = ctx.data_unchecked::<UserStore>();
//     //     Ok(store.user_by_uid(&self.user_id).await)
//     // }

//     // TODO: deliver group information
//     // pub async fn groups(
//     //     &self,
//     //     ctx: &Context<'_>,
//     // ) -> Result<AccessLevel>, async_graphql::FieldError> {
//     //     let store = ctx.data_unchecked::<UserStore>();
//     //     Ok(store.user_by_uid(&self.user_id).await)
//     // }
// }

impl Hash for User {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.details.username.hash(state);
    }
}

#[derive(Default, Debug, Clone, SimpleObject, Serialize, Deserialize)]
pub struct UserList {
    pub items: Vec<User>,
    pub limit: Option<i64>,
    pub total: Option<i64>,
    pub page: Option<i64>,
}

#[derive(Debug)]
pub struct CreateUserPayload {
    pub user: CreateUserInput,
    pub group: String,
    pub access: String,
    pub context: ContextFilterInput,
}

impl NewList<User> for UserList {
    fn new(items: Vec<User>, limit: Option<i64>, total: Option<i64>, page: Option<i64>) -> Self {
        Self {
            items,
            limit,
            total,
            page,
        }
    }
}
