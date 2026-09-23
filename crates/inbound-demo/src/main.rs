mod feed;

use commonware_cryptography::Signer;
use commonware_runtime::{Clock, Runner, Supervisor, tokio};
use feed::{DAY, Posting, month_of_traffic, sellers};
use inbound_agent::{Buyer, Cursor, find_channel, rank};
use inbound_operator::{Attestor, BuyerFacts, ChannelLog, Ledger, Settlement};
use inbound_types::{
    ChannelId, ChannelSpec, FormatId, Offer, PrivateKey, PublicKey, Registry, RegistryBody,
};
use std::collections::HashMap;
use std::time::Duration;

const OCTOBER: u64 = 1_790_812_800;
const DAYS: u64 = 30;
const FLAT_FEE_CENTS: u64 = 100;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

struct Watcher {
    name: &'static str,
    colour: &'static str,
    buyer: Buyer,
    cursor: Cursor,
    best: Option<(String, u64)>,
    ignored: u32,
}

fn main() {
    let millis_per_day: u64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(900);
    let runner = tokio::Runner::default();
    runner.start(|context| async move {
        if let Err(error) = run(context, millis_per_day).await {
            eprintln!("demo failed: {error}");
        }
    });
}

async fn run(
    context: tokio::Context,
    millis_per_day: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let maintainer = PrivateKey::from_seed(1);
    let operator = PrivateKey::from_seed(2);
    let channel = ChannelId::new("cashback/us/sportswear")?;
    let format = FormatId::new("cashback/v0")?;
    let spec = ChannelSpec {
        channel: channel.clone(),
        operator: operator.public_key(),
        format: format.clone(),
        min_bond: 50_000,
    };
    let registry = Registry::sign(
        &maintainer,
        RegistryBody {
            maintainer: maintainer.public_key(),
            channels: vec![spec.clone()],
            published_at: OCTOBER,
        },
    );
    let found = find_channel(&registry, &maintainer.public_key(), &channel)?;

    let mut attestor = Attestor::new(PrivateKey::from_seed(3));
    let mut ledger = Ledger::new(attestor.public_key(), FLAT_FEE_CENTS);
    let mut log = ChannelLog::init(context.child("channel"), spec).await?;

    let sellers = sellers();
    for seller in &sellers {
        log.deposit_bond(seller.public_key(), 50_000);
    }
    let postings = month_of_traffic(&sellers, &channel, &format, OCTOBER, DAYS, 2_026);

    // Two people subscribed to the same channel. Maya is new to all of these shops. Sam has
    // bought from runfast before, and shops smaller.
    let mut maya = Watcher {
        name: "maya",
        colour: CYAN,
        buyer: Buyer {
            root: PrivateKey::from_seed(42),
            credential_secret: [7u8; 32],
            available_balance: 250_000,
            spend_by_seller: HashMap::new(),
            typical_basket: 12_000,
        },
        cursor: Cursor::default(),
        best: None,
        ignored: 0,
    };
    let mut sam = Watcher {
        name: "sam",
        colour: YELLOW,
        buyer: Buyer {
            root: PrivateKey::from_seed(43),
            credential_secret: [9u8; 32],
            available_balance: 40_000,
            spend_by_seller: HashMap::from([(sellers[0].public_key(), 22_000)]),
            typical_basket: 5_000,
        },
        cursor: Cursor::default(),
        best: None,
        ignored: 0,
    };
    for watcher in [&maya, &sam] {
        attestor.record(
            watcher.buyer.root_public_key(),
            BuyerFacts {
                available_balance: watcher.buyer.available_balance,
                settled_spend_by_seller: watcher
                    .buyer
                    .spend_by_seller
                    .iter()
                    .map(|(key, value)| (key.clone(), *value))
                    .collect(),
            },
        );
    }

    header(&found.channel, &maya, &sam);

    let mut day = 0u64;
    let mut posted = 0usize;
    while day < DAYS {
        let now = OCTOBER + day * DAY;
        let mut landed = Vec::new();
        while posted < postings.len() && postings[posted].day == day {
            let Posting {
                seller_index,
                offer,
                ..
            } = &postings[posted];
            ledger.fund(offer);
            log.post(offer).await?;
            landed.push((sellers[*seller_index].name, offer.clone()));
            posted += 1;
        }

        let mut lines = Vec::new();
        for (name, offer) in &landed {
            lines.push(format!(
                "{DIM}│{RESET} {BOLD}{name}{RESET} posts {} back, min ${}, {} days left",
                percent(offer),
                offer.body.min_spend / 100,
                (offer.body.valid_until - now) / DAY
            ));
        }
        for watcher in [&mut maya, &mut sam] {
            if let Some(line) = watcher.poll(&log, now).await? {
                lines.push(line);
            }
        }

        if !lines.is_empty() {
            println!("{DIM}day {:02}{RESET}", day + 1);
            for line in lines {
                println!("  {line}");
            }
        }
        context.sleep(Duration::from_millis(millis_per_day)).await;
        day += 1;
    }

    println!();
    for watcher in [&maya, &sam] {
        match &watcher.best {
            Some((seller, cents)) => println!(
                "{}{}{}  took {BOLD}{seller}{RESET} for {GREEN}${:.2}{RESET} back, ignored {} other offers",
                watcher.colour,
                watcher.name,
                RESET,
                *cents as f64 / 100.0,
                watcher.ignored
            ),
            None => println!(
                "{}{}{}  found nothing worth taking",
                watcher.colour, watcher.name, RESET
            ),
        }
    }

    // One of them buys. The operator issued the card, so it sees the payment and bills the seller.
    if let Some((_, _)) = &maya.best {
        let now = OCTOBER + DAYS * DAY;
        let offers: Vec<(u64, Offer)> = log.read_from(0).await?;
        if let Some(best) = pick(&offers, &maya.buyer, now - DAY) {
            let root = maya.buyer.root_public_key();
            let acceptance =
                maya.buyer
                    .accept(&best, now - DAY, |subject, audience, predicate| {
                        attestor.attest(
                            &root,
                            subject.clone(),
                            audience.clone(),
                            predicate,
                            now - DAY,
                        )
                    })?;
            ledger.reserve(acceptance, root.clone(), now - DAY)?;
            let settlement = Settlement {
                seller: best.body.seller.clone(),
                buyer_root: root.clone(),
                amount: maya.buyer.typical_basket,
                settled_at: now - DAY / 2,
            };
            attestor.record_settlement(&root, settlement.seller.clone(), settlement.amount);
            if let Some(sale) = ledger.settle(&settlement) {
                println!(
                    "\n{DIM}maya pays ${} on her card. the operator sees it, so the seller is billed{RESET}\n  {GREEN}${:.2}{RESET} to maya, {DIM}${:.2} fee{RESET}, after a 30 day returns window",
                    settlement.amount / 100,
                    sale.reward as f64 / 100.0,
                    sale.operator_fee as f64 / 100.0
                );
            }
        }
    }
    Ok(())
}

impl Watcher {
    /// What a subscribed agent does when it wakes: read from its cursor, rank locally, and speak
    /// up only if its answer changed.
    async fn poll<E: commonware_storage::Context>(
        &mut self,
        log: &ChannelLog<E>,
        now: u64,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let delivered = log.read_from(self.cursor.0).await?;
        if delivered.is_empty() {
            return Ok(None);
        }
        self.cursor.advance(&delivered);
        let fresh = delivered.len();

        let all = log.read_from(0).await?;
        let ranked = rank(
            all.into_iter().map(|(_, offer)| offer).collect(),
            &self.buyer,
            now,
            99,
        );
        let Some(top) = ranked.first() else {
            self.ignored += fresh as u32;
            return Ok(None);
        };
        let seller = seller_name(&top.offer.body.seller);
        let improved = match &self.best {
            Some((_, cents)) => top.expected_reward > *cents,
            None => true,
        };
        if !improved {
            self.ignored += fresh as u32;
            return Ok(None);
        }
        let previous = self.best.replace((seller.clone(), top.expected_reward));
        Ok(Some(format!(
            "{}{:<5}{RESET} {DIM}→{RESET} best now {BOLD}{seller}{RESET} at {GREEN}${:.2}{RESET}{}",
            self.colour,
            self.name,
            top.expected_reward as f64 / 100.0,
            match previous {
                Some((_, was)) => format!(" {DIM}(was ${:.2}){RESET}", was as f64 / 100.0),
                None => String::new(),
            }
        )))
    }
}

fn pick(offers: &[(u64, Offer)], buyer: &Buyer, now: u64) -> Option<Offer> {
    rank(
        offers.iter().map(|(_, offer)| offer.clone()).collect(),
        buyer,
        now,
        99,
    )
    .into_iter()
    .next()
    .map(|ranked| ranked.offer)
}

fn percent(offer: &Offer) -> String {
    let best = offer.body.max_reward();
    let base = offer.body.reward.maximum();
    match offer.body.reward {
        inbound_types::Reward::Percent { basis_points, .. } if best > base => {
            format!("{}–{}%", basis_points / 100, best_bps(offer) / 100)
        }
        inbound_types::Reward::Percent { basis_points, .. } => format!("{}%", basis_points / 100),
        inbound_types::Reward::Fixed(amount) => format!("${}", amount / 100),
    }
}

fn best_bps(offer: &Offer) -> u16 {
    offer
        .body
        .better
        .iter()
        .filter_map(|terms| match terms.reward {
            inbound_types::Reward::Percent { basis_points, .. } => Some(basis_points),
            inbound_types::Reward::Fixed(_) => None,
        })
        .max()
        .unwrap_or(0)
}

fn seller_name(key: &PublicKey) -> String {
    sellers()
        .iter()
        .find(|seller| &seller.public_key() == key)
        .map(|seller| seller.name.to_string())
        .unwrap_or_else(|| format!("{key}").chars().take(8).collect())
}

fn header(channel: &ChannelId, maya: &Watcher, sam: &Watcher) {
    println!("\n{BOLD}inbound{RESET} {DIM}·{RESET} {channel}\n");
    println!(
        "  {}{}{RESET}  ${} typical basket, new to every shop here",
        maya.colour,
        maya.name,
        maya.buyer.typical_basket / 100
    );
    println!(
        "  {}{}{RESET}  ${} typical basket, already a runfast customer",
        sam.colour,
        sam.name,
        sam.buyer.typical_basket / 100
    );
    println!(
        "\n{DIM}both subscribed to the same channel. neither is asked anything until the end.{RESET}\n"
    );
}
