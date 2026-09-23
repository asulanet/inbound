mod attestor;
mod channel;
mod ledger;

pub use attestor::{Attestor, BuyerFacts};
pub use channel::{ChannelLog, Delivery, Subscription};
pub use ledger::{Ledger, Sale, Settlement};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("journal: {0}")]
    Journal(#[from] commonware_storage::journal::Error),
    #[error("offer rejected: {0}")]
    Rejected(String),
    #[error("unknown offer")]
    UnknownOffer,
    #[error("budget exhausted")]
    BudgetExhausted,
    #[error("nullifier already used for this offer")]
    NullifierUsed,
}
