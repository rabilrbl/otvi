pub use sqlx_core::acquire::Acquire;
pub use sqlx_core::arguments::{Arguments, IntoArguments};
pub use sqlx_core::column::{Column, ColumnIndex};
pub use sqlx_core::connection::{ConnectOptions, Connection};
pub use sqlx_core::database::{self, Database};
pub use sqlx_core::describe::Describe;
pub use sqlx_core::error::{self, Error, Result};
pub use sqlx_core::executor::{Execute, Executor};
pub use sqlx_core::from_row::FromRow;
pub use sqlx_core::pool::{self, Pool};
pub use sqlx_core::query::{query, query_with};
pub use sqlx_core::query_as::{query_as, query_as_with};
pub use sqlx_core::query_builder::{self, QueryBuilder};
pub use sqlx_core::query_scalar::{query_scalar, query_scalar_with};
pub use sqlx_core::raw_sql::{raw_sql, RawSql};
pub use sqlx_core::row::Row;
pub use sqlx_core::statement::Statement;
pub use sqlx_core::transaction::{Transaction, TransactionManager};
pub use sqlx_core::type_info::TypeInfo;
pub use sqlx_core::types::Type;
pub use sqlx_core::value::{Value, ValueRef};
pub use sqlx_core::Either;

#[cfg(feature = "migrate")]
pub use sqlx_core::migrate;

#[cfg(feature = "postgres")]
pub use sqlx_postgres::{
    self as postgres, PgConnection, PgExecutor, PgPool, PgTransaction, Postgres,
};

#[cfg(feature = "sqlite")]
pub use sqlx_sqlite::{
    self as sqlite, Sqlite, SqliteConnection, SqliteExecutor, SqlitePool, SqliteTransaction,
};

#[cfg(feature = "any")]
pub use any::{reexports::*, Any, AnyExecutor};

#[cfg(feature = "any")]
pub mod any {
    use std::sync::Once;

    pub use sqlx_core::any::driver::install_drivers;
    pub use sqlx_core::any::{
        Any, AnyArguments, AnyConnectOptions, AnyConnectionBackend, AnyExecutor, AnyPool,
        AnyPoolOptions, AnyQueryResult, AnyRow, AnyStatement, AnyTransactionManager, AnyTypeInfo,
        AnyTypeInfoKind, AnyValue, AnyValueKind, AnyValueRef,
    };

    pub mod reexports {
        pub use sqlx_core::any::{AnyConnection, AnyPool};
    }

    pub fn install_default_drivers() {
        static ONCE: Once = Once::new();

        ONCE.call_once(|| {
            install_drivers(&[
                sqlx_postgres::any::DRIVER,
                sqlx_sqlite::any::DRIVER,
            ])
            .expect("non-default drivers already installed")
        });
    }
}
