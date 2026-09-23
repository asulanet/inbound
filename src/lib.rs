//! Inbound: channels businesses post offers into, and agents subscribe to.
//!
//! Four moving parts. A [Channel] holds an append-only log of signed [Offer]s and pushes each one
//! to whoever is listening. A [Buyer] is what an agent knows about its person. A [Directive] is
//! what that person asked it to watch for. [Agent] puts the last two together and decides what is
//! worth their attention — on their own machine, against facts no channel ever sees.

mod channel;
mod directive;
mod offer;
mod rank;
mod sellers;

pub use channel::{Channel, Delivery, Subscription};
pub use directive::{Agent, Decision, Directive, directive_for};
pub use offer::{Body, NAMESPACE, Offer, Predicate, Reward, Terms};
pub use rank::{Buyer, Ranked, rank};
pub use sellers::{CHANNELS, Seller, name_of, next_seller, sellers, write_offer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("offer rejected: {0}")]
    Rejected(&'static str),
    #[error("channel closed after a failed write")]
    Closed,
    #[error("storage: {0}")]
    Storage(#[from] commonware_storage::journal::Error),
}
