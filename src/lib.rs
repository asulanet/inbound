//! Inbound: channels businesses post offers into, and agents subscribe to.
//!
//! Four moving parts. A [Feed] holds an append-only log of signed [Offer]s across every channel
//! the operator runs, and pushes each one to whoever is listening. A [Buyer] is what an agent
//! knows about its person. A [Directive] is what that person asked it to watch for. [Agent] puts
//! the last two together and decides what is worth their attention — on their own machine,
//! against facts no operator ever sees.

mod directive;
mod feed;
mod offer;
mod rank;
mod sellers;

pub use directive::{Agent, Decision, Directive, directive_for};
pub use feed::{Delivery, Feed, Record, Subscription};
pub use offer::{Body, NAMESPACE, Offer, Predicate, Reward, Terms};
pub use rank::{Buyer, Ranked, rank};
pub use sellers::{CHANNELS, Seller, name_of, next_seller, sellers, write_offer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("offer rejected: {0}")]
    Rejected(&'static str),
    #[error("feed closed after a failed write")]
    Closed,
    #[error("storage: {0}")]
    Storage(#[from] commonware_storage::journal::Error),
}
