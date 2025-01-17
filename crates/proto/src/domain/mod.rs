pub mod accounts;
pub mod blocks;
pub mod digest;
pub mod merkle;
pub mod notes;
pub mod nullifiers;
pub mod transactions;

// UTILITIES
// ================================================================================================

pub fn convert<T, From, To, R>(from: T) -> R
where
    T: IntoIterator<Item = From>,
    From: Into<To>,
    R: FromIterator<To>,
{
    from.into_iter().map(Into::into).collect()
}

pub fn try_convert<T, E, From, To, R>(from: T) -> Result<R, E>
where
    T: IntoIterator<Item = From>,
    From: TryInto<To, Error = E>,
    R: FromIterator<To>,
{
    from.into_iter().map(TryInto::try_into).collect()
}
