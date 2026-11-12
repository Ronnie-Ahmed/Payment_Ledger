use sqlx::{PgPool, Postgres, Transaction, pool};
use uuid::Uuid;

#[derive(thiserror::Error,Debug)]
pub enum LedgerError{
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("invalid request: {0}")]
    Invalid(&'static str),
    #[error("idempotency key reused with a different request")]
    KeyConflict,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub async fn create_account(
    pool :&PgPool,
    name: &str,
    currency:&str,
    allow_negative:bool
)->Result<Uuid,LedgerError>{
   Ok(sqlx::query_scalar(
    "INSERT INTO accounts (name,currency,allow_negative)
          VALUES ($1,$2,$3) RETURNING id",
   )
   .bind(name).bind(currency).bind(allow_negative).fetch_one(pool).await?
)
}
pub async fn balance(pool: &PgPool, account: Uuid) -> Result<i64, LedgerError> {
    Ok(sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM postings WHERE account_id = $1",
    )
    .bind(account).fetch_one(pool).await?)
}

pub async fn post_transfer_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    from: Uuid, to: Uuid, amount: i64, description: &str,
) -> Result<Uuid, LedgerError> {
    if amount <= 0 { return Err(LedgerError::Invalid("amount must be positive")); }
    if from == to  { return Err(LedgerError::Invalid("same account")); }

    // lock both rows in a fixed order -> no deadlocks between A->B and B->A
    let (lo, hi) = if from < to { (from, to) } else { (to, from) };
    let accts: Vec<(Uuid, bool, String)> = sqlx::query_as(
        "SELECT id, allow_negative, currency FROM accounts
         WHERE id = ANY($1::uuid[]) ORDER BY id FOR UPDATE",
    )
    .bind(&[lo, hi][..])
    .fetch_all(&mut **tx)
    .await?;

    if accts.len() != 2 { return Err(LedgerError::AccountNotFound); }
    if accts[0].2 != accts[1].2 { return Err(LedgerError::CurrencyMismatch); }

    let from_allows_negative = accts.iter().find(|a| a.0 == from).unwrap().1;
    if !from_allows_negative {
        let bal: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM postings WHERE account_id = $1",
        ).bind(from).fetch_one(&mut **tx).await?;
        if bal - amount < 0 { return Err(LedgerError::InsufficientFunds); }
    }

pub async fn post_transfer(
    pool: &PgPool, from: Uuid, to: Uuid, amount: i64, description: &str,
) -> Result<Uuid, LedgerError> {
    let mut tx = pool.begin().await?;
    let id = post_transfer_in_tx(&mut tx, from, to, amount, description).await?;
    tx.commit().await?;
    Ok(id)
}
