use bitcoin::hashes::hex::ToHex;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};

/// Newtype for the bag id `[u8; 32]`.
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

impl Display for BagId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO: better representation?
        let hex = self.0.to_hex();
        f.write_str(&hex)
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
