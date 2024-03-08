use crate::cleanup::cleanup_roles;
use crate::cleanup::CleanupTaskType;
use crate::context::RelatedAccessLevel;
use crate::context::RelatedAuth;
use crate::context::RelatedPermission;
use crate::context::RelatedResource;
use crate::context::RelatedStorage;
use crate::marker::Marker;
use crate::model::Institution;
use crate::model::Organization;
use crate::model::OrganizationUnit;
use crate::schema::customer::CustomerDB;
use crate::schema::institution::InstitutionDB;
use crate::schema::organization::OrganizationDB;
use crate::schema::organization_unit::OrganizationUnitDB;
use crate::schema::user::UserDB;
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use qm_entity::ids::Iid;
use qm_entity::ids::Oid;
use qm_entity::ids::OrganizationUnitId;
use qm_entity::ids::StrictInstitutionId;
use qm_entity::ids::StrictInstitutionIds;
use qm_entity::ids::StrictOrganizationId;
use qm_entity::ids::StrictOrganizationIds;
use qm_entity::ids::StrictOrganizationUnitIds;
use qm_entity::ids::{
    Cid, CustomerResourceId, OrganizationResourceId, StrictOrganizationUnitId, Uid,
};
use qm_entity::utils::select_ids;
use qm_kafka::producer::EventNs;
use qm_mongodb::bson::doc;
use qm_mongodb::bson::Document;
use qm_mongodb::bson::Uuid;
use qm_mongodb::ClientSession;
use qm_mongodb::DB;
use qm_redis::AsyncWorker;
pub use qm_redis::Producer;
use qm_redis::Work;
use qm_redis::WorkerContext;
use qm_redis::Workers;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;

use crate::cleanup::CleanupTask;
use crate::cleanup::CustomerIds;

pub const PREFIX: &str = "cleanup_tasks";

pub trait CleanupTaskProducer {
    fn cleanup_task_producer(&self) -> &qm_redis::Producer;
}

#[derive(Clone)]
pub struct CleanupProducer {
    inner: Arc<Producer>,
}

impl CleanupProducer {
    pub fn new(redis: Arc<deadpool_redis::Pool>) -> Self {
        Self {
            inner: Arc::new(Producer::new_with_client(redis, PREFIX)),
        }
    }
}

impl AsRef<Producer> for CleanupProducer {
    fn as_ref(&self) -> &Producer {
        self.inner.as_ref()
    }
}

pub struct CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission> {
    pub store: Store,
    _marker: Marker<Auth, Store, AccessLevel, Resource, Permission>,
}

impl<Auth, Store, AccessLevel, Resource, Permission>
    CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>
{
    pub fn new(store: Store) -> Self {
        Self {
            store,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<Auth, Store, AccessLevel, Resource, Permission> Clone
    for CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>
where
    Store: RelatedStorage,
{
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            _marker: self._marker,
        }
    }
}

async fn extend_roles<T>(
    collection: &qm_mongodb::Collection<T>,
    roles: &mut BTreeSet<String>,
    session: &mut ClientSession,
    query: &Document,
    cb: impl Fn(T) -> anyhow::Result<Vec<String>>,
) -> anyhow::Result<()>
where
    T: DeserializeOwned,
{
    let mut items = collection
        .find_with_session(query.clone(), None, session)
        .await?;
    let mut s = items.stream(session);
    while let Some(v) = s.next().await {
        if let Ok(v) = v {
            roles.extend(cb(v)?);
        }
    }
    anyhow::Ok(())
}

async fn remove_documents(
    db: &DB,
    session: &mut ClientSession,
    collection: &str,
    query: &Document,
) -> anyhow::Result<u64> {
    let result = db
        .get()
        .collection::<Document>(collection)
        .delete_many_with_session(query.clone(), None, session)
        .await?;
    Ok(result.deleted_count)
}

async fn remove_users(
    db: &impl UserDB,
    session: &mut ClientSession,
    query: &Document,
) -> anyhow::Result<u64> {
    let mut q = doc! {};
    for (k, v) in query.clone().into_iter() {
        q.insert(&format!("owner.entityId.{k}"), v);
    }
    let result = db
        .users()
        .as_ref()
        .delete_many_with_session(q, None, session)
        .await?;
    Ok(result.deleted_count)
}

async fn update_organization_units(
    db: &impl OrganizationUnitDB,
    session: &mut ClientSession,
    v: &StrictInstitutionId,
) -> anyhow::Result<()> {
    let cid: &Cid = v.as_ref();
    let oid: &Oid = v.as_ref();
    let iid: &Iid = v.as_ref();
    db.organization_units()
        .as_ref()
        .update_many_with_session(
            doc! { "members.cid": **cid, "members.oid": **oid },
            doc! { "$pull": { "members": { "cid": **cid, "oid": **oid, "iid": **iid } }},
            None,
            session,
        )
        .await?;
    Ok(())
}

async fn cleanup_customers<Auth, Store, AccessLevel, Resource, Permission>(
    worker_ctx: WorkerContext<CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>>,
    ty: &str,
    id: Uuid,
    cids: &CustomerIds,
) -> anyhow::Result<()>
where
    Auth: RelatedAuth<AccessLevel, Resource, Permission>,
    Store: RelatedStorage,
    AccessLevel: RelatedAccessLevel,
    Resource: RelatedResource,
    Permission: RelatedPermission,
{
    let store: &Store = &worker_ctx.ctx().store;
    let db: &DB = store.as_ref();
    let mut session = db.session().await?;
    let mut roles = BTreeSet::new();
    for cid in cids.iter() {
        roles.insert(
            qm_role::Access::new(AccessLevel::customer())
                .with_fmt_id(Some(cid))
                .to_string(),
        );
    }
    let ids: Vec<_> = cids.iter().map(|v| (v.as_ref())).collect();
    let query = doc! {
        "cid": {
            "$in": &ids
        }
    };
    extend_roles::<OrganizationUnit>(
        worker_ctx.ctx().store.organization_units().as_ref(),
        &mut roles,
        &mut session,
        &query,
        |v| {
            Ok(vec![qm_role::Access::new(AccessLevel::organization_unit())
                .with_fmt_id(v.id.as_organization_unit_id().as_ref())
                .to_string()])
        },
    )
    .await?;
    extend_roles::<Organization>(
        worker_ctx.ctx().store.organizations().as_ref(),
        &mut roles,
        &mut session,
        &query,
        |v| {
            Ok(vec![qm_role::Access::new(AccessLevel::organization())
                .with_fmt_id(v.id.as_organization_id().as_ref())
                .to_string()])
        },
    )
    .await?;
    extend_roles::<Institution>(
        worker_ctx.ctx().store.institutions().as_ref(),
        &mut roles,
        &mut session,
        &query,
        |v| {
            Ok(vec![qm_role::Access::new(AccessLevel::institution())
                .with_fmt_id(v.id.as_institution_id().as_ref())
                .to_string()])
        },
    )
    .await?;
    for collection in db
        .get()
        .list_collection_names_with_session(None, &mut session)
        .await?
    {
        if collection == UserDB::collection(store) {
            remove_users(store, &mut session, &query).await?;
        } else {
            log::debug!("remove all organization related resources from db {collection}");
            remove_documents(db, &mut session, &collection, &query).await?;
        }
    }
    log::debug!("cleanup roles");
    cleanup_roles(
        store,
        store.redis().as_ref(),
        store.keycloak(),
        store.cache().user(),
        roles,
        &mut session,
    )
    .await?;
    log::debug!("trigger reload event user_cache");
    store
        .cache()
        .user()
        .reload_users(store.keycloak(), store, Some(store.redis().as_ref()))
        .await?;
    log::debug!("trigger reload event customer_cache");
    store
        .cache()
        .customer()
        .reload(store, Some(store.redis().as_ref()))
        .await?;
    // Emit the Kafka event
    if let Some(producer) = store.mutation_event_producer() {
        producer
            .delete_event(&EventNs::Customer, CustomerDB::collection(store), cids)
            .await?;
    }
    worker_ctx.complete().await?;
    log::debug!("finished cleanup task '{ty}' with id '{id}'");
    Ok(())
}

async fn cleanup_organizations<Auth, Store, AccessLevel, Resource, Permission>(
    worker_ctx: WorkerContext<CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>>,
    ty: &str,
    id: Uuid,
    strict_oids: &StrictOrganizationIds,
) -> anyhow::Result<()>
where
    Auth: RelatedAuth<AccessLevel, Resource, Permission>,
    Store: RelatedStorage,
    AccessLevel: RelatedAccessLevel,
    Resource: RelatedResource,
    Permission: RelatedPermission,
{
    let store: &Store = &worker_ctx.ctx().store;
    let db: &DB = store.as_ref();
    let mut session = db.session().await?;
    let mut roles = BTreeSet::new();
    for v in strict_oids.iter() {
        roles.insert(
            qm_role::Access::new(AccessLevel::organization())
                .with_fmt_id(Some(&v))
                .to_string(),
        );
    }
    let cids = select_ids::<StrictOrganizationId, Cid>(strict_oids);
    let oids = select_ids::<StrictOrganizationId, Oid>(strict_oids);
    let query = doc! {
        "cid": {
            "$in": &cids
        },
        "oid": {
            "$in": &oids
        }
    };
    let institution_ids: StrictInstitutionIds = async {
        let mut items = store
            .institutions()
            .as_ref()
            .find_with_session(query.clone(), None, &mut session)
            .await?;
        let s = items.stream(&mut session);
        let s: Vec<Institution> = s.try_collect().await?;
        anyhow::Ok(s.into_iter().filter_map(|v| v.try_into().ok()).collect())
    }
    .await?;
    for id in institution_ids.iter() {
        update_organization_units(store, &mut session, id).await?;
    }
    extend_roles::<OrganizationUnit>(
        worker_ctx.ctx().store.organization_units().as_ref(),
        &mut roles,
        &mut session,
        &query,
        |v| {
            Ok(vec![qm_role::Access::new(AccessLevel::organization_unit())
                .with_fmt_id(v.id.as_organization_unit_id().as_ref())
                .to_string()])
        },
    )
    .await?;
    for collection in db
        .get()
        .list_collection_names_with_session(None, &mut session)
        .await?
    {
        if collection == UserDB::collection(store) {
            remove_users(store, &mut session, &query).await?;
        } else {
            log::debug!("remove all organization related resources from db {collection}");
            remove_documents(db, &mut session, &collection, &query).await?;
        }
    }
    log::debug!("cleanup roles");
    cleanup_roles(
        store,
        store.redis().as_ref(),
        store.keycloak(),
        store.cache().user(),
        roles,
        &mut session,
    )
    .await?;
    log::debug!("trigger reload event user_cache");
    store
        .cache()
        .user()
        .reload_users(store.keycloak(), store, Some(store.redis().as_ref()))
        .await?;
    log::debug!("trigger reload event customer_cache");
    store
        .cache()
        .customer()
        .reload(store, Some(store.redis().as_ref()))
        .await?;
    // Emit the Kafka event
    if let Some(producer) = store.mutation_event_producer() {
        producer
            .delete_event(
                &EventNs::Organization,
                OrganizationDB::collection(store),
                cids,
            )
            .await?;
    }
    worker_ctx.complete().await?;
    log::debug!("finished cleanup task '{ty}' with id '{id}'");
    Ok(())
}

async fn cleanup_institutions<Auth, Store, AccessLevel, Resource, Permission>(
    worker_ctx: WorkerContext<CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>>,
    ty: &str,
    id: Uuid,
    strict_iids: &StrictInstitutionIds,
) -> anyhow::Result<()>
where
    Auth: RelatedAuth<AccessLevel, Resource, Permission>,
    Store: RelatedStorage,
    AccessLevel: RelatedAccessLevel,
    Resource: RelatedResource,
    Permission: RelatedPermission,
{
    let store: &Store = &worker_ctx.ctx().store;
    let db: &DB = store.as_ref();
    let mut session = db.session().await?;
    let mut roles = BTreeSet::new();

    for id in strict_iids.iter() {
        roles.insert(
            qm_role::Access::new(AccessLevel::institution())
                .with_fmt_id(Some(&id))
                .to_string(),
        );
        update_organization_units(store, &mut session, id).await?;
    }
    let cids = select_ids::<StrictInstitutionId, Cid>(strict_iids);
    let oids = select_ids::<StrictInstitutionId, Oid>(strict_iids);
    let iids = select_ids::<StrictInstitutionId, Iid>(strict_iids);

    let query = doc! {
        "cid": {
            "$in": &cids
        },
        "oid": {
            "$in": &oids
        },
        "iid": {
            "$in": &iids
        }
    };
    for collection in db
        .get()
        .list_collection_names_with_session(None, &mut session)
        .await?
    {
        if collection == UserDB::collection(store) {
            remove_users(store, &mut session, &query).await?;
        } else {
            log::debug!("remove all organization related resources from db {collection}");
            remove_documents(db, &mut session, &collection, &query).await?;
        }
    }
    log::debug!("cleanup roles");
    cleanup_roles(
        store,
        store.redis().as_ref(),
        store.keycloak(),
        store.cache().user(),
        roles,
        &mut session,
    )
    .await?;
    log::debug!("trigger reload event user_cache");
    store
        .cache()
        .user()
        .reload_users(store.keycloak(), store, Some(store.redis().as_ref()))
        .await?;
    log::debug!("trigger reload event customer_cache");
    store
        .cache()
        .customer()
        .reload(store, Some(store.redis().as_ref()))
        .await?;
    // Emit the Kafka event
    if let Some(producer) = store.mutation_event_producer() {
        producer
            .delete_event(
                &EventNs::Institution,
                InstitutionDB::collection(store),
                strict_iids,
            )
            .await?;
    }
    worker_ctx.complete().await?;
    log::debug!("finished cleanup task '{ty}' with id '{id}'");
    Ok(())
}

async fn cleanup_organization_units<Auth, Store, AccessLevel, Resource, Permission>(
    worker_ctx: WorkerContext<CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>>,
    ty: &str,
    id: Uuid,
    strict_uids: &StrictOrganizationUnitIds,
) -> anyhow::Result<()>
where
    Auth: RelatedAuth<AccessLevel, Resource, Permission>,
    Store: RelatedStorage,
    AccessLevel: RelatedAccessLevel,
    Resource: RelatedResource,
    Permission: RelatedPermission,
{
    let store: &Store = &worker_ctx.ctx().store;
    let db: &DB = store.as_ref();
    let mut session = db.session().await?;
    let mut roles = BTreeSet::new();
    for id in strict_uids.iter() {
        let cid: &Cid = id.as_ref();
        let oid: &Option<Oid> = id.as_ref();
        let uid: &Uid = id.as_ref();
        let id = if let Some(oid) = oid.as_ref() {
            OrganizationUnitId::Organization(OrganizationResourceId {
                cid: cid.as_ref().clone(),
                oid: oid.as_ref().clone(),
                id: uid.as_ref().clone(),
            })
        } else {
            OrganizationUnitId::Customer(CustomerResourceId {
                cid: cid.as_ref().clone(),
                id: uid.as_ref().clone(),
            })
        };
        roles.insert(
            qm_role::Access::new(AccessLevel::organization_unit())
                .with_fmt_id(Some(&id))
                .to_string(),
        );
    }
    let cids = select_ids::<StrictOrganizationUnitId, Cid>(strict_uids);
    let oids = select_ids::<StrictOrganizationUnitId, Uid>(strict_uids);
    let query = doc! {
        "cid": &cids,
        "oid": &oids,
    };
    remove_users(store, &mut session, &query).await?;
    log::debug!("cleanup roles");
    cleanup_roles(
        store,
        store.redis().as_ref(),
        store.keycloak(),
        store.cache().user(),
        roles,
        &mut session,
    )
    .await?;
    log::debug!("trigger reload event user_cache");
    store
        .cache()
        .user()
        .reload_users(store.keycloak(), store, Some(store.redis().as_ref()))
        .await?;
    log::debug!("trigger reload event customer_cache");
    store
        .cache()
        .customer()
        .reload(store, Some(store.redis().as_ref()))
        .await?;
    // Emit the Kafka event
    if let Some(producer) = store.mutation_event_producer() {
        producer
            .delete_event(
                &EventNs::OrganizationUnit,
                OrganizationUnitDB::collection(store),
                strict_uids,
            )
            .await?;
    }
    worker_ctx.complete().await?;
    log::debug!("finished cleanup task '{ty}' with id '{id}'");
    Ok(())
}

pub struct CleanupWorker;

#[async_trait::async_trait]
impl<Auth, Store, AccessLevel, Resource, Permission>
    Work<CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>, CleanupTask>
    for CleanupWorker
where
    Auth: RelatedAuth<AccessLevel, Resource, Permission>,
    Store: RelatedStorage,
    AccessLevel: RelatedAccessLevel,
    Resource: RelatedResource,
    Permission: RelatedPermission,
{
    async fn run(
        &self,
        ctx: WorkerContext<CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>>,
        item: CleanupTask,
    ) -> anyhow::Result<()> {
        log::debug!(
            "start cleanup task '{}' with id '{}'",
            item.ty.as_ref(),
            item.id
        );
        match &item.ty {
            CleanupTaskType::Customers(ids) => {
                cleanup_customers(ctx, item.ty.as_ref(), item.id, ids).await?;
            }
            CleanupTaskType::Organizations(ids) => {
                cleanup_organizations(ctx, item.ty.as_ref(), item.id, ids).await?;
            }
            CleanupTaskType::Institutions(ids) => {
                cleanup_institutions(ctx, item.ty.as_ref(), item.id, ids).await?;
            }
            CleanupTaskType::OrganizationUnits(ids) => {
                cleanup_organization_units(ctx, item.ty.as_ref(), item.id, ids).await?;
            }
            CleanupTaskType::None => {
                ctx.complete().await?;
            }
        }
        Ok(())
    }
}

pub async fn run<Auth, Store, AccessLevel, Resource, Permission>(
    workers: &Workers,
    ctx: CleanupWorkerCtx<Auth, Store, AccessLevel, Resource, Permission>,
    num_workers: usize,
) -> anyhow::Result<()>
where
    Auth: RelatedAuth<AccessLevel, Resource, Permission>,
    Store: RelatedStorage,
    AccessLevel: RelatedAccessLevel,
    Resource: RelatedResource,
    Permission: RelatedPermission,
{
    workers
        .start(
            ctx,
            AsyncWorker::new(PREFIX)
                .with_num_workers(num_workers)
                .run(CleanupWorker),
        )
        .await?;
    Ok(())
}