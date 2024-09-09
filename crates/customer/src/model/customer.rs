use async_graphql::{InputObject, SimpleObject};
use qm_entity::ids::{CustomerId, InfraId};
use serde::{Deserialize, Serialize};
use sqlx::types::uuid::Uuid;
use sqlx::FromRow;

use std::sync::Arc;

use time::PrimitiveDateTime;

#[derive(Debug, InputObject)]
pub struct QmCreateCustomerInput {
    pub id: Option<i64>,
    pub name: String,
    pub ty: Option<String>,
}

#[derive(Debug, InputObject)]
pub struct QmUpdateCustomerInput {
    pub name: String,
}

pub struct CustomerData(pub String, pub Option<String>, pub Option<i64>);

#[derive(Debug, Clone, SimpleObject, FromRow, Serialize, Deserialize)]
#[graphql(complex)]
pub struct QmCustomer {
    #[graphql(skip)]
    pub id: InfraId,
    pub name: Arc<str>,
    pub ty: Arc<str>,
    pub created_by: Uuid,
    pub created_at: PrimitiveDateTime,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerUpdate {
    pub id: InfraId,
    pub name: Arc<str>,
    pub ty: Arc<str>,
    pub created_by: Uuid,
    pub created_at: String,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<String>,
}

pub struct RemoveCustomerPayload {
    pub id: InfraId,
    pub name: Arc<str>,
}

impl From<CustomerUpdate> for RemoveCustomerPayload {
    fn from(value: CustomerUpdate) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl<'a> From<&'a QmCustomer> for RemoveCustomerPayload {
    fn from(value: &'a QmCustomer) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct QmCustomerList {
    pub items: Arc<[Arc<QmCustomer>]>,
    pub limit: Option<i64>,
    pub total: Option<i64>,
    pub page: Option<i64>,
}

impl<'a> From<&'a QmCustomer> for CustomerId {
    fn from(val: &'a QmCustomer) -> Self {
        (*val.id.as_ref()).into()
    }
}
