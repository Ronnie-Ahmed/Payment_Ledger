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
    if from == to { return Err(LedgerError::Invalid("same account")); }

    let entry_id: Uuid = sqlx::query_scalar(
        "INSERT INTO journal_entries (description) VALUES ($1) RETURNING id",
    )
    .bind(description).fetch_one(&mut **tx).await?;

    for (account, signed) in [(from, -amount), (to, amount)] {
        sqlx::query(
            "INSERT INTO postings (journal_entry_id, account_id, amount)
             VALUES ($1, $2, $3)",
        )
        .bind(entry_id).bind(account).bind(signed)
        .execute(&mut **tx).await?;
    }
    Ok(entry_id)
}

pub async fn post_transfer(
    pool: &PgPool, from: Uuid, to: Uuid, amount: i64, description: &str,
) -> Result<Uuid, LedgerError> {
    let mut tx = pool.begin().await?;
    let id = post_transfer_in_tx(&mut tx, from, to, amount, description).await?;
    tx.commit().await?;
    Ok(id)
}