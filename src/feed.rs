use crate::Offer;
use bytes::{Buf, BufMut};
use commonware_codec::{EncodeSize, Read, ReadExt, Write};
use commonware_runtime::buffer::paged::CacheRef;
use commonware_storage::journal::contiguous::{
    Contiguous,
    variable::{Config, Journal},
};
use commonware_utils::{NZU16, NZU64, NZUsize};
use futures::channel::mpsc;
use std::collections::HashMap;

/// What the operator keeps: the offer as its seller signed it, and when the operator first saw
/// it. The arrival time is the operator's own word, so an agent judges whether an offer is live
/// from `expires` inside the signature and never from this.
#[derive(Clone)]
pub struct Record {
    pub arrived: u64,
    pub offer: Offer,
}

/// A record with the two positions the operator assigns it. `feed` is the cursor an agent resumes
/// from, and `channel` counts only within this offer's own channel — so an agent reading one
/// channel out of many can tell it missed something there without ever subscribing to it, and so
/// without telling the operator which channel it cares about.
#[derive(Clone)]
pub struct Delivery {
    pub feed: u64,
    pub channel: u64,
    pub arrived: u64,
    pub offer: Offer,
}

/// What a subscriber holds. In a deployment this is the far end of an SSE stream; the direction is
/// what matters — the operator writes to it, the agent never asks.
pub struct Subscription(mpsc::UnboundedReceiver<Delivery>);

impl Subscription {
    pub async fn next(&mut self) -> Option<Delivery> {
        use futures::StreamExt;
        self.0.next().await
    }
}

/// Every channel the operator runs, as one append-only log with one cursor. Channels are a field
/// on the offers rather than separate logs, so an agent subscribes once and sorts them out itself.
pub struct Feed<E: commonware_storage::Context> {
    journal: Option<Journal<E, Record>>,
    counts: HashMap<String, u64>,
    subscribers: Vec<mpsc::UnboundedSender<Delivery>>,
}

impl<E: commonware_storage::Context> Feed<E> {
    pub async fn open(context: E) -> Result<Self, crate::Error> {
        let page_cache = CacheRef::from_pooler(&context, NZU16!(4096), NZUsize!(64));
        let journal = Journal::init(
            context,
            Config {
                partition: "inbound_feed".to_string(),
                items_per_section: NZU64!(1024),
                compression: None,
                codec_config: (),
                page_cache,
                write_buffer: NZUsize!(65536),
                replay_buffer: NZUsize!(262144),
            },
        )
        .await?;

        let mut counts: HashMap<String, u64> = HashMap::new();
        let bounds = journal.bounds();
        for position in bounds.start..bounds.end {
            let record: Record = journal.read(position).await?;
            *counts.entry(record.offer.body.channel).or_insert(0) += 1;
        }

        Ok(Self {
            journal: Some(journal),
            counts,
            subscribers: Vec::new(),
        })
    }

    pub fn subscribers(&self) -> usize {
        self.subscribers.len()
    }

    pub fn tip(&self) -> u64 {
        self.journal.as_ref().map(Journal::size).unwrap_or(0)
    }

    /// Every channel the operator carries and how much has been posted to each. An agent reads
    /// this to know what exists; it says nothing about who reads what.
    pub fn channels(&self) -> Vec<(&'static str, u64)> {
        crate::CHANNELS
            .iter()
            .map(|name| (*name, self.counts.get(*name).copied().unwrap_or(0)))
            .collect()
    }

    /// An agent asks to be told. The operator records a queue and nothing else: not who the agent
    /// is, not who it acts for, not which channels it came for.
    pub fn subscribe(&mut self) -> Subscription {
        let (sender, receiver) = mpsc::unbounded();
        self.subscribers.push(sender);
        Subscription(receiver)
    }

    /// A seller posts. Bad signatures and offers for channels this operator does not run are
    /// refused; the rest are appended and pushed to everyone listening. Subscribers that went
    /// away are dropped. The caller supplies `arrived` because the library reads no clock.
    pub async fn post(&mut self, offer: &Offer, arrived: u64) -> Result<Delivery, crate::Error> {
        if !offer.verify() {
            return Err(crate::Error::Rejected("signature"));
        }
        if !crate::CHANNELS.contains(&offer.body.channel.as_str()) {
            return Err(crate::Error::Rejected("unknown channel"));
        }

        let record = Record {
            arrived,
            offer: offer.clone(),
        };
        let journal = self.journal.take().ok_or(crate::Error::Closed)?;
        let (journal, feed) = journal.append(&record).await?;
        self.journal = Some(journal.commit().await?);

        let count = self
            .counts
            .entry(offer.body.channel.clone())
            .or_insert_with(|| 0);
        *count += 1;

        let delivery = Delivery {
            feed,
            channel: *count - 1,
            arrived,
            offer: offer.clone(),
        };
        self.subscribers
            .retain(|sender| sender.unbounded_send(delivery.clone()).is_ok());
        Ok(delivery)
    }

    /// Catch-up for an agent that was not connected when something was pushed. Channel positions
    /// are counted from the oldest record still held, so they are consecutive for anything an
    /// agent receives but do not survive the log being pruned.
    pub async fn since(&self, cursor: u64) -> Result<Vec<Delivery>, crate::Error> {
        let Some(journal) = self.journal.as_ref() else {
            return Ok(Vec::new());
        };
        let bounds = journal.bounds();
        let mut counted: HashMap<String, u64> = HashMap::new();
        let mut out = Vec::new();
        for feed in bounds.start..bounds.end {
            let record = journal.read(feed).await?;
            let seen = counted
                .entry(record.offer.body.channel.clone())
                .or_insert_with(|| 0);
            let channel = *seen;
            *seen += 1;
            if feed < cursor {
                continue;
            }
            out.push(Delivery {
                feed,
                channel,
                arrived: record.arrived,
                offer: record.offer,
            });
        }
        Ok(out)
    }
}

impl Write for Record {
    fn write(&self, buf: &mut impl BufMut) {
        self.arrived.write(buf);
        self.offer.write(buf);
    }
}

impl EncodeSize for Record {
    fn encode_size(&self) -> usize {
        self.arrived.encode_size() + self.offer.encode_size()
    }
}

impl Read for Record {
    type Cfg = ();

    fn read_cfg(buf: &mut impl Buf, _: &()) -> Result<Self, commonware_codec::Error> {
        Ok(Self {
            arrived: u64::read(buf)?,
            offer: Offer::read(buf)?,
        })
    }
}
