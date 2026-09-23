use crate::Offer;
use commonware_runtime::buffer::paged::CacheRef;
use commonware_storage::journal::contiguous::{
    Contiguous,
    variable::{Config, Journal},
};
use commonware_utils::{NZU16, NZU64, NZUsize};
use futures::channel::mpsc;

/// An offer and where it sits in the channel's log.
pub type Delivery = (u64, Offer);

/// What a subscriber holds. In a deployment this is the far end of an SSE stream; the direction
/// is what matters — the operator writes to it, the agent never asks.
pub struct Subscription(mpsc::UnboundedReceiver<Delivery>);

impl Subscription {
    pub async fn next(&mut self) -> Option<Delivery> {
        use futures::StreamExt;
        self.0.next().await
    }
}

/// One channel: an append-only log of offers, and everyone waiting to be told about them.
pub struct Channel<E: commonware_storage::Context> {
    name: String,
    journal: Option<Journal<E, Offer>>,
    subscribers: Vec<mpsc::UnboundedSender<Delivery>>,
}

impl<E: commonware_storage::Context> Channel<E> {
    pub async fn open(context: E, name: &str) -> Result<Self, crate::Error> {
        let page_cache = CacheRef::from_pooler(&context, NZU16!(4096), NZUsize!(64));
        let journal = Journal::init(
            context,
            Config {
                partition: format!("inbound_{}", name.replace('/', "_")),
                items_per_section: NZU64!(1024),
                compression: None,
                codec_config: (),
                page_cache,
                write_buffer: NZUsize!(65536),
                replay_buffer: NZUsize!(262144),
            },
        )
        .await?;
        Ok(Self {
            name: name.to_string(),
            journal: Some(journal),
            subscribers: Vec::new(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn subscribers(&self) -> usize {
        self.subscribers.len()
    }

    /// An agent asks to be told. The operator records a queue and nothing else: not who the agent
    /// is, not who it acts for, not what it is looking for.
    pub fn subscribe(&mut self) -> Subscription {
        let (sender, receiver) = mpsc::unbounded();
        self.subscribers.push(sender);
        Subscription(receiver)
    }

    /// A seller posts. Bad signatures and offers meant for another channel are refused; the rest
    /// are appended and pushed to everyone listening. Subscribers that went away are dropped.
    pub async fn post(&mut self, offer: &Offer) -> Result<u64, crate::Error> {
        if !offer.verify() {
            return Err(crate::Error::Rejected("signature"));
        }
        if offer.body.channel != self.name {
            return Err(crate::Error::Rejected("wrong channel"));
        }
        let journal = self.journal.take().ok_or(crate::Error::Closed)?;
        let (journal, position) = journal.append(offer).await?;
        self.journal = Some(journal.commit().await?);

        self.subscribers
            .retain(|sender| sender.unbounded_send((position, offer.clone())).is_ok());
        Ok(position)
    }

    pub fn tip(&self) -> u64 {
        self.journal.as_ref().map(Journal::size).unwrap_or(0)
    }

    /// Catch-up for an agent that was not connected when something was pushed.
    pub async fn since(&self, cursor: u64) -> Result<Vec<Delivery>, crate::Error> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(Vec::new());
        };
        let bounds = journal.bounds();
        let mut out = Vec::new();
        for position in cursor.max(bounds.start)..bounds.end {
            out.push((position, journal.read(position).await?));
        }
        Ok(out)
    }
}
