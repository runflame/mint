use std::convert::TryFrom;

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub struct BagId(pub [u8; 32]);

impl From<[u8; 32]> for BagId {
    fn from(val: [u8; 32]) -> Self {
        Self(val)
    }
}

impl PartialEq<[u8; 32]> for BagId {
    fn eq(&self, other: &[u8; 32]) -> bool {
        self.0.eq(other)
    }
}

impl<'a> TryFrom<&'a [u8]> for BagId {
    type Error = <[u8; 32] as TryFrom<&'a [u8]>>::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(BagId(<_>::try_from(value)?))
    }
}

#[cfg(feature = "rusqlite")]
mod sql_impls {
    use crate::bag_id::BagId;
    use rusqlite::types::ToSqlOutput;
    use rusqlite::ToSql;

    impl ToSql for BagId {
        fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
            self.0.to_sql()
        }
    }
}
