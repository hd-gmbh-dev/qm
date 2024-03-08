use async_graphql::InputObject;
use async_graphql::SimpleObject;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

use async_graphql::{InputValueError, InputValueResult, Scalar, ScalarType, Value};
use serde::{Deserialize, Serialize};

use qm_mongodb::bson::oid::ObjectId;
pub type ID = Arc<ObjectId>;

pub const EMPTY_ID: &str = "000000000000000000000000";

fn parse_object_id(id: &str) -> anyhow::Result<Option<ID>> {
    if id == EMPTY_ID {
        Ok(None)
    } else {
        Ok(Some(Arc::new(ObjectId::from_str(id)?)))
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct EntityId {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<ID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oid: Option<ID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iid: Option<ID>,
}

impl EntityId {
    pub fn with_customer(mut self, cid: ID) -> Self {
        self.cid = Some(cid);
        self
    }

    pub fn as_customer_id(&self) -> Option<CustomerId> {
        self.id.clone().map(|id| CustomerId { id })
    }

    pub fn as_organization_id(&self) -> Option<OrganizationId> {
        self.cid
            .clone()
            .zip(self.id.clone())
            .map(|(cid, id)| OrganizationId { cid, id })
    }

    pub fn as_organization_unit_id(&self) -> Option<OrganizationUnitId> {
        if let Some(oid) = self.oid.clone() {
            self.cid.clone().zip(self.id.clone()).map(|(cid, id)| {
                OrganizationUnitId::Organization(OrganizationResourceId { cid, oid, id })
            })
        } else {
            self.cid
                .clone()
                .zip(self.id.clone())
                .map(|(cid, id)| OrganizationUnitId::Customer(CustomerResourceId { cid, id }))
        }
    }

    pub fn as_institution_id(&self) -> Option<InstitutionId> {
        self.cid
            .clone()
            .zip(self.oid.clone().zip(self.id.clone()))
            .map(|(cid, (oid, id))| InstitutionId { cid, oid, id })
    }
}

pub type EntityIds = Arc<[EntityId]>;

impl FromStr for EntityId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.len() {
            24 => Ok(Self {
                cid: parse_object_id(&s[0..24])?,
                oid: None,
                iid: None,
                id: None,
            }),
            48 => Ok(Self {
                cid: parse_object_id(&s[0..24])?,
                oid: parse_object_id(&s[24..48])?,
                iid: None,
                id: None,
            }),
            72 => Ok(Self {
                cid: parse_object_id(&s[0..24])?,
                oid: parse_object_id(&s[24..48])?,
                iid: parse_object_id(&s[48..72])?,
                id: None,
            }),
            96 => Ok(Self {
                cid: parse_object_id(&s[0..24])?,
                oid: parse_object_id(&s[24..48])?,
                iid: parse_object_id(&s[48..72])?,
                id: parse_object_id(&s[72..96])?,
            }),
            _ => Err(anyhow::anyhow!(
                "invalid length, EntityId should have 24, 48, 72 or 96 characters"
            )),
        }
    }
}

#[Scalar]
impl ScalarType for EntityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(
                EntityId::from_str(value)
                    .map_err(|err| InputValueError::custom(err.to_string()))?,
            )
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(
            [
                self.cid
                    .as_ref()
                    .map(|v| v.to_hex())
                    .as_deref()
                    .unwrap_or(EMPTY_ID),
                self.oid
                    .as_ref()
                    .map(|v| v.to_hex())
                    .as_deref()
                    .unwrap_or(EMPTY_ID),
                self.iid
                    .as_ref()
                    .map(|v| v.to_hex())
                    .as_deref()
                    .unwrap_or(EMPTY_ID),
                self.id
                    .as_ref()
                    .map(|v| v.to_hex())
                    .as_deref()
                    .unwrap_or(EMPTY_ID),
            ]
            .join(""),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct CustomerId {
    #[serde(rename = "_id")]
    pub id: ID,
}

impl From<CustomerId> for EntityId {
    fn from(value: CustomerId) -> Self {
        Self {
            id: Some(value.id),
            cid: None,
            oid: None,
            iid: None,
        }
    }
}

impl AsRef<ObjectId> for CustomerId {
    fn as_ref(&self) -> &ObjectId {
        &self.id
    }
}

impl std::fmt::Display for CustomerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id.to_hex())
    }
}

impl FromStr for CustomerId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 24 {
            anyhow::bail!("invalid length, CustomerId should have 24 characters");
        }
        Ok(Self {
            id: parse_object_id(&s[0..24])?
                .ok_or(anyhow::anyhow!("'id' is required on CustomerId"))?,
        })
    }
}

#[Scalar]
impl ScalarType for CustomerId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(CustomerId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.id.to_hex())
    }
}

#[derive(
    Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct CustomerResourceId {
    #[serde(rename = "_id")]
    pub id: ID,
    pub cid: ID,
}

impl From<CustomerResourceId> for EntityId {
    fn from(value: CustomerResourceId) -> Self {
        Self {
            id: Some(value.id),
            cid: Some(value.cid),
            oid: None,
            iid: None,
        }
    }
}

impl FromStr for CustomerResourceId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 48 {
            anyhow::bail!("invalid length, CustomerResourceId should have 48 characters");
        }
        Ok(Self {
            cid: parse_object_id(&s[0..24])?
                .ok_or(anyhow::anyhow!("'cid' is required on CustomerResourceId"))?,
            id: parse_object_id(&s[24..48])?
                .ok_or(anyhow::anyhow!("'oid' is required on CustomerResourceId"))?,
        })
    }
}

#[Scalar]
impl ScalarType for CustomerResourceId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(CustomerResourceId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String([self.cid.to_hex().as_str(), self.id.to_hex().as_str()].join(""))
    }
}

#[derive(
    Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct OrganizationResourceId {
    #[serde(rename = "_id")]
    pub id: ID,
    pub cid: ID,
    pub oid: ID,
}

impl From<OrganizationResourceId> for EntityId {
    fn from(value: OrganizationResourceId) -> Self {
        Self {
            id: Some(value.id),
            cid: Some(value.cid),
            oid: Some(value.oid),
            iid: None,
        }
    }
}

impl FromStr for OrganizationResourceId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 72 {
            anyhow::bail!("invalid length, OrganizationResourceId should have 72 characters");
        }
        Ok(Self {
            cid: parse_object_id(&s[0..24])?.ok_or(anyhow::anyhow!(
                "'cid' is required on OrganizationResourceId"
            ))?,
            oid: parse_object_id(&s[24..48])?.ok_or(anyhow::anyhow!(
                "'oid' is required on OrganizationResourceId"
            ))?,
            id: parse_object_id(&s[48..72])?.ok_or(anyhow::anyhow!(
                "'id' is required on OrganizationResourceId"
            ))?,
        })
    }
}

#[Scalar]
impl ScalarType for OrganizationResourceId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(OrganizationResourceId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(
            [
                self.cid.to_hex().as_str(),
                self.oid.to_hex().as_str(),
                self.id.to_hex().as_str(),
            ]
            .join(""),
        )
    }
}

#[derive(
    Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct InstitutionResourceId {
    #[serde(rename = "_id")]
    pub id: ID,
    pub cid: ID,
    pub oid: ID,
    pub iid: ID,
}

impl From<InstitutionResourceId> for EntityId {
    fn from(value: InstitutionResourceId) -> Self {
        Self {
            id: Some(value.id),
            cid: Some(value.cid),
            oid: Some(value.oid),
            iid: Some(value.iid),
        }
    }
}

impl FromStr for InstitutionResourceId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 96 {
            anyhow::bail!("invalid length, InstitutionResourceId should have 96 characters");
        }
        Ok(Self {
            cid: parse_object_id(&s[0..24])?.ok_or(anyhow::anyhow!(
                "'cid' is required on InstitutionResourceId"
            ))?,
            oid: parse_object_id(&s[24..48])?.ok_or(anyhow::anyhow!(
                "'oid' is required on InstitutionResourceId"
            ))?,
            iid: parse_object_id(&s[48..72])?.ok_or(anyhow::anyhow!(
                "'iid' is required on InstitutionResourceId"
            ))?,
            id: parse_object_id(&s[72..96])?
                .ok_or(anyhow::anyhow!("'id' is required on InstitutionResourceId"))?,
        })
    }
}

#[Scalar]
impl ScalarType for InstitutionResourceId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(InstitutionResourceId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(
            [
                self.cid.to_hex().as_str(),
                self.oid.to_hex().as_str(),
                self.iid.to_hex().as_str(),
                self.id.to_hex().as_str(),
            ]
            .join(""),
        )
    }
}

#[derive(
    Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum OrganizationUnitId {
    Customer(CustomerResourceId),
    Organization(OrganizationResourceId),
}

impl TryFrom<EntityId> for OrganizationUnitId {
    type Error = anyhow::Error;

    fn try_from(value: EntityId) -> Result<Self, Self::Error> {
        let cid = value.cid.ok_or(anyhow::anyhow!("cid is missing"))?;
        let uid = value.id.ok_or(anyhow::anyhow!("id is missing"))?;
        if let Some(oid) = value.oid {
            Ok(OrganizationUnitId::Organization(OrganizationResourceId {
                cid,
                oid,
                id: uid,
            }))
        } else {
            Ok(OrganizationUnitId::Customer(CustomerResourceId {
                cid,
                id: uid,
            }))
        }
    }
}

impl FromStr for OrganizationUnitId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 76 {
            return Ok(Self::Organization(OrganizationResourceId {
                cid: parse_object_id(&s[0..24])?.ok_or(anyhow::anyhow!(
                    "'cid' is required on OrganizationUnitId::Organization"
                ))?,
                oid: parse_object_id(&s[24..48])?.ok_or(anyhow::anyhow!(
                    "'oid' is required on OrganizationUnitId::Organization"
                ))?,
                id: parse_object_id(&s[48..72])?.ok_or(anyhow::anyhow!(
                    "'id' is required on OrganizationUnitId::Organization"
                ))?,
            }));
        }
        if s.len() == 48 {
            return Ok(Self::Customer(CustomerResourceId {
                cid: parse_object_id(&s[0..24])?.ok_or(anyhow::anyhow!(
                    "'cid' is required on OrganizationUnitId::Customer"
                ))?,
                id: parse_object_id(&s[24..48])?.ok_or(anyhow::anyhow!(
                    "'id' is required on OrganizationUnitId::Customer"
                ))?,
            }));
        }
        anyhow::bail!("invalid length, OrganizationUnitId should have 48 or 72 characters")
    }
}

#[Scalar]
impl ScalarType for OrganizationUnitId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(OrganizationUnitId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        match self {
            OrganizationUnitId::Customer(v) => v.to_value(),
            OrganizationUnitId::Organization(v) => v.to_value(),
        }
    }
}

#[derive(
    Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct OrganizationUnitResourceId {
    #[serde(rename = "_id")]
    pub id: ID,
    pub cid: ID,
    pub oid: Option<ID>,
    pub uid: ID,
}

impl From<OrganizationUnitResourceId> for EntityId {
    fn from(value: OrganizationUnitResourceId) -> Self {
        Self {
            id: Some(value.id),
            cid: Some(value.cid),
            oid: value.oid,
            iid: Some(value.uid),
        }
    }
}

impl FromStr for OrganizationUnitResourceId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 96 {
            return Ok(Self {
                cid: parse_object_id(&s[0..24])?.ok_or(anyhow::anyhow!(
                    "'cid' is required on OrganizationUnitResourceId"
                ))?,
                oid: Some(parse_object_id(&s[24..48])?.ok_or(anyhow::anyhow!(
                    "'oid' is required on OrganizationUnitResourceId"
                ))?),
                uid: parse_object_id(&s[48..72])?.ok_or(anyhow::anyhow!(
                    "'iid' is required on OrganizationUnitResourceId"
                ))?,
                id: parse_object_id(&s[72..96])?.ok_or(anyhow::anyhow!(
                    "'id' is required on OrganizationUnitResourceId"
                ))?,
            });
        }
        if s.len() == 72 {
            return Ok(Self {
                cid: parse_object_id(&s[0..24])?.ok_or(anyhow::anyhow!(
                    "'cid' is required on OrganizationUnitResourceId"
                ))?,
                oid: None,
                uid: parse_object_id(&s[24..48])?.ok_or(anyhow::anyhow!(
                    "'iid' is required on OrganizationUnitResourceId"
                ))?,
                id: parse_object_id(&s[48..72])?.ok_or(anyhow::anyhow!(
                    "'id' is required on OrganizationUnitResourceId"
                ))?,
            });
        }
        anyhow::bail!("invalid length, OrganizationUnitResourceId should have 72 or 96 characters")
    }
}

#[Scalar]
impl ScalarType for OrganizationUnitResourceId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(OrganizationUnitResourceId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        if let Some(oid) = self.oid.as_ref() {
            Value::String(
                [
                    self.cid.to_hex().as_str(),
                    oid.to_hex().as_str(),
                    self.uid.to_hex().as_str(),
                    self.id.to_hex().as_str(),
                ]
                .join(""),
            )
        } else {
            Value::String(
                [
                    self.cid.to_hex().as_str(),
                    self.uid.to_hex().as_str(),
                    self.id.to_hex().as_str(),
                ]
                .join(""),
            )
        }
    }
}

pub type OrganizationId = CustomerResourceId;
pub type InstitutionId = OrganizationResourceId;

impl From<EntityId> for CustomerId {
    fn from(value: EntityId) -> Self {
        Self {
            id: value.id.unwrap_or_default(),
        }
    }
}

impl From<EntityId> for OrganizationId {
    fn from(value: EntityId) -> Self {
        Self {
            cid: value.cid.unwrap_or_default(),
            id: value.id.unwrap_or_default(),
        }
    }
}

impl std::fmt::Display for OrganizationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.cid.to_hex(), self.id.to_hex())
    }
}

impl From<EntityId> for InstitutionId {
    fn from(value: EntityId) -> Self {
        Self {
            cid: value.cid.unwrap_or_default(),
            oid: value.oid.unwrap_or_default(),
            id: value.id.unwrap_or_default(),
        }
    }
}

impl std::fmt::Display for InstitutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.cid.to_hex(),
            self.oid.to_hex(),
            self.id.to_hex()
        )
    }
}

impl std::fmt::Display for OrganizationUnitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrganizationUnitId::Customer(v) => v.fmt(f),
            OrganizationUnitId::Organization(v) => v.fmt(f),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cid {
    #[serde(flatten)]
    cid: ID,
}
impl Cid {
    pub fn new(cid: ID) -> Self {
        Self { cid }
    }
}
impl Deref for Cid {
    type Target = ObjectId;

    fn deref(&self) -> &Self::Target {
        &self.cid
    }
}
impl AsRef<ID> for Cid {
    fn as_ref(&self) -> &ID {
        &self.cid
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct Oid {
    #[serde(flatten)]
    oid: ID,
}
impl Oid {
    pub fn new(oid: ID) -> Self {
        Self { oid }
    }
}
impl Deref for Oid {
    type Target = ObjectId;
    fn deref(&self) -> &Self::Target {
        &self.oid
    }
}
impl AsRef<ID> for Oid {
    fn as_ref(&self) -> &ID {
        &self.oid
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uid {
    #[serde(flatten)]
    uid: ID,
}
impl Uid {
    pub fn new(uid: ID) -> Self {
        Self { uid }
    }
}
impl Deref for Uid {
    type Target = ObjectId;
    fn deref(&self) -> &Self::Target {
        &self.uid
    }
}
impl AsRef<ID> for Uid {
    fn as_ref(&self) -> &ID {
        &self.uid
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct Iid {
    #[serde(flatten)]
    iid: ID,
}
impl Iid {
    pub fn new(iid: ID) -> Self {
        Self { iid }
    }
}
impl Deref for Iid {
    type Target = ObjectId;
    fn deref(&self) -> &Self::Target {
        &self.iid
    }
}
impl AsRef<ID> for Iid {
    fn as_ref(&self) -> &ID {
        &self.iid
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, InputObject)]
pub struct StrictCustomerId {
    #[graphql(flatten)]
    #[serde(rename = "_id")]
    cid: Cid,
}
impl AsRef<Cid> for StrictCustomerId {
    fn as_ref(&self) -> &Cid {
        &self.cid
    }
}
impl From<StrictCustomerId> for EntityId {
    fn from(value: StrictCustomerId) -> Self {
        Self {
            cid: None,
            oid: None,
            iid: None,
            id: Some(value.cid.cid),
        }
    }
}
pub type StrictCustomerIds = Arc<[StrictCustomerId]>;

#[derive(Debug, Clone, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct StrictOrganizationId {
    #[graphql(flatten)]
    cid: Cid,
    #[graphql(flatten)]
    #[serde(rename = "_id")]
    oid: Oid,
}
impl std::fmt::Display for StrictOrganizationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            self.cid.as_ref().to_hex(),
            self.oid.as_ref().to_hex()
        )
    }
}
impl AsRef<Cid> for StrictOrganizationId {
    fn as_ref(&self) -> &Cid {
        &self.cid
    }
}
impl AsRef<Oid> for StrictOrganizationId {
    fn as_ref(&self) -> &Oid {
        &self.oid
    }
}
impl From<StrictOrganizationId> for EntityId {
    fn from(value: StrictOrganizationId) -> Self {
        Self {
            cid: Some(value.cid.cid),
            oid: None,
            iid: None,
            id: Some(value.oid.oid),
        }
    }
}
impl From<StrictOrganizationId> for CustomerResourceId {
    fn from(value: StrictOrganizationId) -> Self {
        Self {
            cid: value.cid.cid.clone(),
            id: value.oid.oid.clone(),
        }
    }
}
pub type StrictOrganizationIds = Arc<[StrictOrganizationId]>;
#[derive(Debug, Clone, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct StrictOrganizationUnitId {
    #[graphql(flatten)]
    cid: Cid,
    oid: Option<Oid>,
    #[graphql(flatten)]
    #[serde(rename = "_id")]
    uid: Uid,
}

impl std::fmt::Display for StrictOrganizationUnitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(oid) = self.oid.as_ref() {
            write!(
                f,
                "{}{}{}",
                self.cid.as_ref().to_hex(),
                oid.to_hex(),
                self.uid.as_ref().to_hex()
            )
        } else {
            write!(
                f,
                "{}{}",
                self.cid.as_ref().to_hex(),
                self.uid.as_ref().to_hex()
            )
        }
    }
}

impl From<(ID, Option<ID>, ID)> for StrictOrganizationUnitId {
    fn from(value: (ID, Option<ID>, ID)) -> Self {
        Self {
            cid: Cid::new(value.0),
            oid: value.1.map(Oid::new),
            uid: Uid::new(value.2),
        }
    }
}

impl AsRef<Cid> for StrictOrganizationUnitId {
    fn as_ref(&self) -> &Cid {
        &self.cid
    }
}

impl AsRef<Uid> for StrictOrganizationUnitId {
    fn as_ref(&self) -> &Uid {
        &self.uid
    }
}

impl AsRef<Option<Oid>> for StrictOrganizationUnitId {
    fn as_ref(&self) -> &Option<Oid> {
        &self.oid
    }
}
impl TryFrom<EntityId> for StrictOrganizationUnitId {
    type Error = anyhow::Error;
    fn try_from(value: EntityId) -> Result<Self, Self::Error> {
        let cid = value.cid.ok_or(anyhow::anyhow!("cid is missing"))?;
        let uid = value.id.ok_or(anyhow::anyhow!("id is missing"))?;
        if let Some(oid) = value.oid {
            Ok(StrictOrganizationUnitId {
                cid: Cid { cid },
                oid: Some(Oid { oid }),
                uid: Uid { uid },
            })
        } else {
            Ok(StrictOrganizationUnitId {
                cid: Cid { cid },
                oid: None,
                uid: Uid { uid },
            })
        }
    }
}

pub type StrictOrganizationUnitIds = Arc<[StrictOrganizationUnitId]>;

#[derive(Debug, Clone, Serialize, Deserialize, InputObject, PartialEq, Eq, PartialOrd, Ord)]
pub struct StrictInstitutionId {
    #[graphql(flatten)]
    cid: Cid,
    #[graphql(flatten)]
    oid: Oid,
    #[graphql(flatten)]
    #[serde(rename = "_id")]
    iid: Iid,
}

impl std::fmt::Display for StrictInstitutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.cid.as_ref().to_hex(),
            self.oid.as_ref().to_hex(),
            self.iid.as_ref().to_hex()
        )
    }
}

impl From<(ID, ID, ID)> for StrictInstitutionId {
    fn from(value: (ID, ID, ID)) -> Self {
        Self {
            cid: Cid::new(value.0),
            oid: Oid::new(value.1),
            iid: Iid::new(value.2),
        }
    }
}

impl AsRef<Cid> for StrictInstitutionId {
    fn as_ref(&self) -> &Cid {
        &self.cid
    }
}
impl AsRef<Oid> for StrictInstitutionId {
    fn as_ref(&self) -> &Oid {
        &self.oid
    }
}
impl AsRef<Iid> for StrictInstitutionId {
    fn as_ref(&self) -> &Iid {
        &self.iid
    }
}

impl From<StrictInstitutionId> for EntityId {
    fn from(value: StrictInstitutionId) -> Self {
        Self {
            cid: Some(value.cid.cid),
            oid: Some(value.oid.oid),
            iid: None,
            id: Some(value.iid.iid),
        }
    }
}
impl From<StrictInstitutionId> for OrganizationResourceId {
    fn from(value: StrictInstitutionId) -> Self {
        Self {
            cid: value.cid.cid,
            oid: value.oid.oid,
            id: value.iid.iid,
        }
    }
}

pub type StrictInstitutionIds = Arc<[StrictInstitutionId]>;

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StrictEntityId {
    pub cid: ID,
    pub oid: ID,
    pub iid: ID,
    pub id: ID,
}

impl FromStr for StrictEntityId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 96 {
            anyhow::bail!("invalid length, LongEntityId should have 96 characters");
        }
        Ok(Self {
            cid: parse_object_id(&s[0..24])?
                .ok_or(anyhow::anyhow!("'cid' is required on StrictEntityId"))?,
            oid: parse_object_id(&s[24..48])?
                .ok_or(anyhow::anyhow!("'oid' is required on StrictEntityId"))?,
            iid: parse_object_id(&s[48..72])?
                .ok_or(anyhow::anyhow!("'iid' is required on StrictEntityId"))?,
            id: parse_object_id(&s[72..96])?
                .ok_or(anyhow::anyhow!("'id' is required on StrictEntityId"))?,
        })
    }
}

pub type StrictEntityIds = Arc<[StrictEntityId]>;

#[Scalar]
impl ScalarType for StrictEntityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(StrictEntityId::from_str(value)
                .map_err(|err| InputValueError::custom(err.to_string()))?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(
            [
                self.cid.to_hex(),
                self.oid.to_hex(),
                self.iid.to_hex(),
                self.id.to_hex(),
            ]
            .join(""),
        )
    }
}

#[derive(Default, Debug, Clone, SimpleObject, InputObject, Serialize, Deserialize)]
#[graphql(input_name = "MemberIdInput")]
pub struct MemberId {
    pub cid: ID,
    pub oid: ID,
    pub iid: ID,
}

impl<'a> From<&'a OrganizationResourceId> for CustomerResourceId {
    fn from(val: &'a OrganizationResourceId) -> Self {
        CustomerResourceId {
            cid: val.cid.clone(),
            id: val.oid.clone(),
        }
    }
}

impl PartialEq<CustomerResourceId> for OrganizationResourceId {
    fn eq(&self, other: &CustomerResourceId) -> bool {
        self.cid.eq(&other.cid) && self.oid.eq(&other.id)
    }
}