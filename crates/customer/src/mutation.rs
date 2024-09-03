use crate::model::*;
use qm_entity::ids::{InfraId, InstitutionIds};
use sqlx::types::Uuid;
use sqlx::PgPool;
use std::sync::Arc;

pub const DEFAULT_TYPE: &str = "none";

const NAME_MAX_LEN: usize = 1024;
const TY_MAX_LEN: usize = 16;
const INPUT_SLICE_MAX_SIZE: usize = 1024 * 1024 * 1024;

fn check_max_size(name: &str, v: Option<&str>, max_len: usize) -> anyhow::Result<()> {
    if let Some(v) = v {
        if v.len() > max_len {
            anyhow::bail!("The value of '{name}' name is bigger than {max_len} characters");
        }
    }
    Ok(())
}

fn check_max_size_input_slice<T>(name: &str, v: &[T]) -> anyhow::Result<()> {
    let mem_size = std::mem::size_of_val(v);
    if mem_size > INPUT_SLICE_MAX_SIZE {
        anyhow::bail!(
            "The input length of '{name}' is bigger than {} bytes",
            INPUT_SLICE_MAX_SIZE
        );
    }
    Ok(())
}

pub async fn create_customer(
    pool: &PgPool,
    id: Option<i64>,
    name: &str,
    ty: Option<&str>,
    created_by: &Uuid,
) -> anyhow::Result<Customer> {
    check_max_size("Customer name", Some(name), NAME_MAX_LEN)?;
    check_max_size("Customer ty", ty, TY_MAX_LEN)?;
    if let Some(id) = id {
        let rec = sqlx::query!(
            r#"
INSERT INTO customers ( id, name, ty, created_by )
VALUES ( $1, $2, $3, $4 )
RETURNING
    id,
    name,
    ty,
    created_by,
    created_at,
    updated_by,
    updated_at
"#,
            id,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            created_by
        )
        .fetch_one(pool)
        .await?;
        Ok(Customer {
            id: rec.id.into(),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
        })
    } else {
        let rec = sqlx::query!(
            r#"
INSERT INTO customers ( name, ty, created_by )
VALUES ( $1, $2, $3 )
RETURNING
    id,
    name,
    ty,
    created_by,
    created_at,
    updated_by,
    updated_at
"#,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            created_by
        )
        .fetch_one(pool)
        .await?;

        Ok(Customer {
            id: rec.id.into(),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
        })
    }
}

pub async fn update_customer(
    pool: &PgPool,
    id: InfraId,
    name: &str,
    updated_by: &Uuid,
) -> anyhow::Result<Customer> {
    check_max_size("Customer name", Some(name), NAME_MAX_LEN)?;
    let rec = sqlx::query!(
        r#"
UPDATE customers AS v
SET name = $2, updated_by = $3, updated_at = NOW()
WHERE v.id = $1
RETURNING
    v.id as id,
    v.name as name,
    v.ty as ty,
    v.created_by as created_by,
    v.created_at as created_at,
    v.updated_by as updated_by,
    v.updated_at as updated_at
"#,
        id.as_ref(),
        name,
        updated_by
    )
    .fetch_one(pool)
    .await?;

    Ok(Customer {
        id: rec.id.into(),
        name: Arc::from(rec.name),
        ty: Arc::from(rec.ty),
        created_by: rec.created_by,
        created_at: rec.created_at,
        updated_by: rec.updated_by,
        updated_at: rec.updated_at,
    })
}

pub async fn remove_customer(pool: &PgPool, id: InfraId) -> anyhow::Result<u64> {
    Ok(
        sqlx::query!("DELETE FROM customers WHERE id = $1", id.as_ref())
            .execute(pool)
            .await?
            .rows_affected() as u64,
    )
}

pub async fn remove_customers(pool: &PgPool, ids: &[i64]) -> anyhow::Result<u64> {
    check_max_size_input_slice("Customer ids", ids)?;
    let result = sqlx::query!(
        "DELETE FROM customers WHERE id IN (SELECT UNNEST($1::int8[]))",
        &ids[..] as &[i64]
    )
    .execute(pool)
    .await?
    .rows_affected() as u64;
    Ok(result)
}

pub async fn create_organization(
    pool: &PgPool,
    id: Option<i64>,
    name: &str,
    ty: Option<&str>,
    customer_id: InfraId,
    created_by: &Uuid,
) -> anyhow::Result<Organization> {
    check_max_size("Organization name", Some(name), NAME_MAX_LEN)?;
    check_max_size("Organization ty", ty, TY_MAX_LEN)?;
    if let Some(id) = id {
        let rec = sqlx::query!(
            r#"
    INSERT INTO organizations ( id, name, ty, customer_id, created_by )
    VALUES ( $1, $2, $3, $4, $5 )
    RETURNING
        id,
        customer_id,
        name,
        ty,
        created_by,
        created_at,
        updated_by,
        updated_at
    "#,
            id,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            customer_id.as_ref(),
            created_by
        )
        .fetch_one(pool)
        .await?;

        Ok(Organization {
            id: rec.id.into(),
            customer_id: rec.customer_id.into(),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
        })
    } else {
        let rec = sqlx::query!(
            r#"
    INSERT INTO organizations ( name, ty, customer_id, created_by )
    VALUES ( $1, $2, $3, $4 )
    RETURNING
        id,
        customer_id,
        name,
        ty,
        created_by,
        created_at,
        updated_by,
        updated_at
    "#,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            customer_id.as_ref(),
            created_by
        )
        .fetch_one(pool)
        .await?;

        Ok(Organization {
            id: rec.id.into(),
            customer_id: rec.customer_id.into(),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
        })
    }
}

pub async fn update_organization(
    pool: &PgPool,
    id: InfraId,
    name: &str,
    updated_by: &Uuid,
) -> anyhow::Result<Organization> {
    let rec = sqlx::query!(
        r#"
UPDATE organizations AS v
SET name = $2, updated_by = $3, updated_at = NOW()
WHERE v.id = $1
RETURNING
    v.id as id,
    v.customer_id as customer_id,
    v.name as name,
    v.ty as ty,
    v.created_by as created_by,
    v.created_at as created_at,
    v.updated_by as updated_by,
    v.updated_at as updated_at
"#,
        id.as_ref(),
        name,
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(Organization {
        id: rec.id.into(),
        customer_id: rec.customer_id.into(),
        name: Arc::from(rec.name),
        ty: Arc::from(rec.ty),
        created_by: rec.created_by,
        created_at: rec.created_at,
        updated_by: rec.updated_by,
        updated_at: rec.updated_at,
    })
}

pub async fn remove_organization(pool: &PgPool, id: InfraId) -> anyhow::Result<u64> {
    Ok(
        sqlx::query!("DELETE FROM organizations WHERE id = $1", id.as_ref())
            .execute(pool)
            .await?
            .rows_affected() as u64,
    )
}

pub async fn remove_organizations(pool: &PgPool, ids: &[i64]) -> anyhow::Result<u64> {
    check_max_size_input_slice("Organization ids", ids)?;
    let result = sqlx::query!(
        "DELETE FROM organizations WHERE id IN (SELECT UNNEST($1::int8[]))",
        &ids[..] as &[i64]
    )
    .execute(pool)
    .await?
    .rows_affected() as u64;
    Ok(result)
}

pub async fn create_institution(
    pool: &PgPool,
    id: Option<i64>,
    name: &str,
    ty: Option<&str>,
    customer_id: InfraId,
    organization_id: InfraId,
    created_by: &Uuid,
) -> anyhow::Result<Institution> {
    check_max_size("Institution name", Some(name), NAME_MAX_LEN)?;
    check_max_size("Institution ty", ty, TY_MAX_LEN)?;
    if let Some(id) = id {
        let rec = sqlx::query!(
            r#"
INSERT INTO institutions ( id, name, ty, customer_id, organization_id, created_by )
VALUES ( $1, $2, $3, $4, $5, $6 )
RETURNING
    id,
    customer_id,
    organization_id,
    name,
    ty,
    created_by,
    created_at,
    updated_by,
    updated_at
"#,
            id,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            customer_id.as_ref(),
            organization_id.as_ref(),
            created_by
        )
        .fetch_one(pool)
        .await?;

        Ok(Institution {
            id: rec.id.into(),
            customer_id: rec.customer_id.into(),
            organization_id: rec.organization_id.into(),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
        })
    } else {
        let rec = sqlx::query!(
            r#"
INSERT INTO institutions ( name, ty, customer_id, organization_id, created_by )
VALUES ( $1, $2, $3, $4, $5 )
RETURNING
    id,
    customer_id,
    organization_id,
    name,
    ty,
    created_by,
    created_at,
    updated_by,
    updated_at
"#,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            customer_id.as_ref(),
            organization_id.as_ref(),
            created_by
        )
        .fetch_one(pool)
        .await?;

        Ok(Institution {
            id: rec.id.into(),
            customer_id: rec.customer_id.into(),
            organization_id: rec.organization_id.into(),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
        })
    }
}

pub async fn update_institution(
    pool: &PgPool,
    id: InfraId,
    name: &str,
    updated_by: &Uuid,
) -> anyhow::Result<Institution> {
    check_max_size("Institution name", Some(name), NAME_MAX_LEN)?;
    let rec = sqlx::query!(
        r#"
UPDATE institutions AS v
SET name = $2, updated_by = $3, updated_at = NOW()
WHERE v.id = $1
RETURNING
    v.id as id,
    v.customer_id as customer_id,
    v.organization_id as organization_id,
    v.name as name,
    v.ty as ty,
    v.created_by as created_by,
    v.created_at as created_at,
    v.updated_by as updated_by,
    v.updated_at as updated_at
"#,
        id.as_ref(),
        name,
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(Institution {
        id: rec.id.into(),
        customer_id: rec.customer_id.into(),
        organization_id: rec.organization_id.into(),
        name: Arc::from(rec.name),
        ty: Arc::from(rec.ty),
        created_by: rec.created_by,
        created_at: rec.created_at,
        updated_by: rec.updated_by,
        updated_at: rec.updated_at,
    })
}

pub async fn remove_institution(pool: &PgPool, id: InfraId) -> anyhow::Result<u64> {
    Ok(
        sqlx::query!("DELETE FROM institutions WHERE id = $1", id.as_ref())
            .execute(pool)
            .await?
            .rows_affected() as u64,
    )
}

pub async fn remove_institutions(pool: &PgPool, ids: &[i64]) -> anyhow::Result<u64> {
    check_max_size_input_slice("Institution ids", ids)?;
    let result = sqlx::query!(
        "DELETE FROM institutions WHERE id IN (SELECT UNNEST($1::int8[]))",
        &ids[..] as &[i64]
    )
    .execute(pool)
    .await?
    .rows_affected() as u64;
    Ok(result)
}

pub async fn create_organization_unit(
    pool: &PgPool,
    id: Option<i64>,
    name: &str,
    ty: Option<&str>,
    customer_id: InfraId,
    organization_id: Option<InfraId>,
    created_by: &Uuid,
    members: InstitutionIds,
) -> anyhow::Result<OrganizationUnit> {
    check_max_size("Organization Unit name", Some(name), NAME_MAX_LEN)?;
    check_max_size("Organization Unit ty", ty, TY_MAX_LEN)?;
    if let Some(id) = id {
        let rec = sqlx::query!(
            r#"
INSERT INTO organization_units ( id, name, ty, customer_id, organization_id, created_by )
VALUES ( $1, $2, $3, $4, $5, $6 )
RETURNING
    id,
    customer_id,
    organization_id,
    name,
    ty,
    created_by,
    created_at,
    updated_by,
    updated_at
"#,
            id,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            customer_id.as_ref(),
            organization_id.as_deref(),
            created_by
        )
        .fetch_one(pool)
        .await?;

        let organization_unit_ids: Vec<i64> = (0..members.len()).map(|_| rec.id).collect();
        let customer_ids: Vec<i64> = members.iter().map(|m| m.cid).collect();
        let organization_ids: Vec<i64> = members.iter().map(|m| m.oid).collect();
        let institution_ids: Vec<i64> = members.iter().map(|m| m.iid).collect();
        sqlx::query!(
        "
            INSERT INTO organization_unit_members(organization_unit_id, customer_id, organization_id, institution_id)
            SELECT * FROM UNNEST($1::int8[], $2::int8[], $3::int8[], $4::int8[])
        ",
        &organization_unit_ids[..] as &[i64],
        &customer_ids[..] as &[i64],
        &organization_ids[..] as &[i64],
        &institution_ids[..] as &[i64],
    )
        .execute(pool)
        .await?;

        Ok(OrganizationUnit {
            id: rec.id.into(),
            customer_id: rec.customer_id.into(),
            organization_id: rec.organization_id.map(Into::into),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
            members,
        })
    } else {
        let rec = sqlx::query!(
            r#"
INSERT INTO organization_units ( name, ty, customer_id, organization_id, created_by )
VALUES ( $1, $2, $3, $4, $5 )
RETURNING
    id,
    customer_id,
    organization_id,
    name,
    ty,
    created_by,
    created_at,
    updated_by,
    updated_at
"#,
            name,
            ty.unwrap_or(DEFAULT_TYPE),
            customer_id.as_ref(),
            organization_id.as_deref(),
            created_by
        )
        .fetch_one(pool)
        .await?;

        let organization_unit_ids: Vec<i64> = (0..members.len()).map(|_| rec.id).collect();
        let customer_ids: Vec<i64> = members.iter().map(|m| m.cid).collect();
        let organization_ids: Vec<i64> = members.iter().map(|m| m.oid).collect();
        let institution_ids: Vec<i64> = members.iter().map(|m| m.iid).collect();
        sqlx::query!(
            "
                INSERT INTO organization_unit_members(organization_unit_id, customer_id, organization_id, institution_id)
                SELECT * FROM UNNEST($1::int8[], $2::int8[], $3::int8[], $4::int8[])
            ",
            &organization_unit_ids[..] as &[i64],
            &customer_ids[..] as &[i64],
            &organization_ids[..] as &[i64],
            &institution_ids[..] as &[i64],
        )
            .execute(pool)
            .await?;

        Ok(OrganizationUnit {
            id: rec.id.into(),
            customer_id: rec.customer_id.into(),
            organization_id: rec.organization_id.map(Into::into),
            name: Arc::from(rec.name),
            ty: Arc::from(rec.ty),
            created_by: rec.created_by,
            created_at: rec.created_at,
            updated_by: rec.updated_by,
            updated_at: rec.updated_at,
            members,
        })
    }
}

pub async fn update_organization_unit(
    pool: &PgPool,
    id: InfraId,
    name: &str,
    updated_by: &Uuid,
) -> anyhow::Result<OrganizationUnit> {
    check_max_size("Organization Unit name", Some(name), NAME_MAX_LEN)?;
    let rec = sqlx::query!(
        r#"
UPDATE organization_units AS v
SET name = $2, updated_by = $3, updated_at = NOW()
WHERE v.id = $1
RETURNING
    v.id as id,
    v.customer_id as customer_id,
    v.organization_id as organization_id,
    v.name as name,
    v.ty as ty,
    v.created_by as created_by,
    v.created_at as created_at,
    v.updated_by as updated_by,
    v.updated_at as updated_at
"#,
        id.as_ref(),
        name,
        updated_by
    )
    .fetch_one(pool)
    .await?;

    let members_rec = sqlx::query!(
        r#"SELECT institution_id, organization_id, customer_id FROM organization_unit_members WHERE organization_unit_id = $1;"#,
        id.as_ref(),
    )
    .fetch_all(pool)
    .await?;

    Ok(OrganizationUnit {
        id: rec.id.into(),
        customer_id: rec.customer_id.into(),
        organization_id: rec.organization_id.map(Into::into),
        name: Arc::from(rec.name),
        ty: Arc::from(rec.ty),
        created_by: rec.created_by,
        created_at: rec.created_at,
        updated_by: rec.updated_by,
        updated_at: rec.updated_at,
        members: members_rec
            .into_iter()
            .map(|m| (m.customer_id, m.organization_id, m.institution_id).into())
            .collect(),
    })
}

pub async fn remove_organization_unit(pool: &PgPool, id: InfraId) -> anyhow::Result<u64> {
    Ok(
        sqlx::query!("DELETE FROM organization_units WHERE id = $1", id.as_ref())
            .execute(pool)
            .await?
            .rows_affected() as u64,
    )
}

pub async fn remove_organization_units(pool: &PgPool, ids: &[i64]) -> anyhow::Result<u64> {
    check_max_size_input_slice("Organization Unit ids", ids)?;
    let result = sqlx::query!(
        "DELETE FROM organization_units WHERE id IN (SELECT UNNEST($1::int8[]))",
        &ids[..] as &[i64]
    )
    .execute(pool)
    .await?
    .rows_affected() as u64;
    Ok(result)
}
