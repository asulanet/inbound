//! Inbound: a channel businesses post offers into, and agents subscribe to.
//!
//! Three moving parts. A [Channel] holds an append-only log of signed [Offer]s and pushes each
//! one to whoever is listening. A [Buyer] describes one person's situation. [rank] decides which
//! offers are worth their attention, on their own machine, against facts the channel never sees.

mod channel;
mod offer;
mod rank;
mod sellers;

pub use channel::{Channel, Delivery, Subscription};
pub use offer::{Body, NAMESPACE, Offer, Predicate, Reward, Terms};
pub use rank::{Buyer, Ranked, rank};
pub use sellers::{Seller, name_of, sellers, write_offer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("offer rejected: {0}")]
    Rejected(&'static str),
    #[error("channel closed after a failed write")]
    Closed,
    #[error("storage: {0}")]
    Storage(#[from] commonware_storage::journal::Error),
}
