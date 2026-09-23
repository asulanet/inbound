use crate::Error;
use commonware_runtime::buffer::paged::CacheRef;
use commonware_storage::journal::contiguous::{
    Contiguous,
    variable::{Config, Journal},
};
use commonware_utils::{NZU16, NZU64, NZUsize};
use futures::channel::mpsc;
use inbound_types::{ChannelSpec, Offer, PublicKey};
use std::collections::HashMap;
use std::num::NonZeroU16;

const PAGE_SIZE: NonZeroU16 = NZU16!(4096);

/// A delivery an operator makes to one subscriber.
pub type Delivery = (u64, Offer);

/// What a subscriber holds. In a deployment this is the far end of a webhook or an open MCP
/// stream; here it is an in-process queue, but the direction is the same: the operator writes to
/// it, the agent never asks.
pub struct Subscription {
    channel: String,
    incoming: mpsc::UnboundedReceiver<Delivery>,
}

impl Subscription {
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Everything pushed since the agent last looked, without a round trip to the operator.
    pub fn drain(&mut self) -> Vec<Delivery> {
        let mut delivered = Vec::new();
        while let Ok(item) = self.incoming.try_recv() {
            delivered.push(item);
        }
        delivered
    }

    /// Wait for the next push. Returns `None` once the operator is gone.
    pub async fn next(&mut self) -> Option<Delivery> {
        use futures::StreamExt;
        self.incoming.next().await
    }
}

/// One channel: an append-only log of offers, the rules the operator enforces on what may enter
/// it, and the set of agents it pushes to.
pub struct ChannelLog<E: commonware_storage::Context> {
    spec: ChannelSpec,
    journal: Option<Journal<E, Offer>>,
    subscribers: Vec<mpsc::UnboundedSender<Delivery>>,
    /// Bonds the operator is actually holding, by seller. A seller cannot talk its way past this
    /// by writing a number in its own offer.
    bonds: HashMap<PublicKey, u64>,
}

impl<E: commonware_storage::Context> ChannelLog<E> {
    pub async fn init(context: E, spec: ChannelSpec) -> Result<Self, Error> {
        let config = Config {
            partition: format!(
                "inbound_channel_{}",
                spec.channel.as_str().replace('/', "_")
            ),
            items_per_section: NZU64!(1024),
            compression: None,
            codec_config: (),
            page_cache: CacheRef::from_pooler(&context, PAGE_SIZE, NZUsize!(64)),
            write_buffer: NZUsize!(64 * 1024),
            replay_buffer: NZUsize!(256 * 1024),
        };
        let journal = Journal::init(context, config).await?;
        Ok(Self {
            spec,
            journal: Some(journal),
            subscribers: Vec::new(),
            bonds: HashMap::new(),
        })
    }

    pub fn spec(&self) -> &ChannelSpec {
        &self.spec
    }

    /// A seller puts money up before it may post. Nothing here slashes it yet; what this buys
    /// today is that posting is not free to a stranger.
    pub fn deposit_bond(&mut self, seller: PublicKey, amount: u64) -> u64 {
        let entry = self.bonds.entry(seller).or_insert(0);
        *entry += amount;
        *entry
    }

    pub fn bond_held(&self, seller: &PublicKey) -> u64 {
        self.bonds.get(seller).copied().unwrap_or(0)
    }

    pub fn subscribers(&self) -> usize {
        self.subscribers.len()
    }

    /// An agent registers to be pushed to. Nothing about the agent or its user is recorded: the
    /// operator learns only that one more party is listening to this topic.
    pub fn subscribe(&mut self) -> Subscription {
        let (sender, incoming) = mpsc::unbounded();
        self.subscribers.push(sender);
        Subscription {
            channel: self.spec.channel.to_string(),
            incoming,
        }
    }

    /// A seller posts. The operator checks the offer, appends it, then pushes it to everyone
    /// subscribed. Subscribers that have gone away are dropped.
    pub async fn post(&mut self, offer: &Offer) -> Result<u64, Error> {
        offer
            .verify()
            .map_err(|error| Error::Rejected(error.to_string()))?;
        if offer.body.channel != self.spec.channel {
            return Err(Error::Rejected(format!(
                "addressed to {} not {}",
                offer.body.channel, self.spec.channel
            )));
        }
        if offer.body.format != self.spec.format {
            return Err(Error::Rejected(format!(
                "format {} but channel expects {}",
                offer.body.format, self.spec.format
            )));
        }
        let held = self.bond_held(&offer.body.seller);
        if held < self.spec.min_bond {
            return Err(Error::Rejected(format!(
                "seller has {held} on deposit, channel requires {}",
                self.spec.min_bond
            )));
        }
        if offer.body.budget < offer.body.max_reward() {
            return Err(Error::Rejected(
                "budget cannot cover a single acceptance".into(),
            ));
        }

        let journal = self
            .journal
            .take()
            .ok_or_else(|| Error::Rejected("channel log lost after a failed write".into()))?;
        let (journal, position) = journal.append(offer).await?;
        self.journal = Some(journal.commit().await?);

        self.subscribers
            .retain(|sender| sender.unbounded_send((position, offer.clone())).is_ok());
        Ok(position)
    }

    pub fn tip(&self) -> u64 {
        self.journal
            .as_ref()
            .map(|journal| journal.size())
            .unwrap_or(0)
    }

    /// Catch-up for an agent that was not reachable when something was pushed. Push is how offers
    /// normally arrive; this is how an agent recovers after being off.
    pub async fn read_from(&self, cursor: u64) -> Result<Vec<Delivery>, Error> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(Vec::new());
        };
        let bounds = journal.bounds();
        let start = cursor.max(bounds.start);
        let mut offers = Vec::with_capacity(bounds.end.saturating_sub(start) as usize);
        for position in start..bounds.end {
            offers.push((position, journal.read(position).await?));
        }
        Ok(offers)
    }
}
