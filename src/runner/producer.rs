use crate::runner::cache::Cache;
use crate::runner::report::QueryStatusStore;
use crate::utils::Argument;
use governor::clock::{Clock, DefaultClock, QuantaClock, QuantaInstant, Reference};
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::ops::Add;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct QueryProducer {
    pub store: QueryStatusStore,
    max_counter: u64,
    counter: u64,
    stop_at: u64,
    start_send: std::time::SystemTime,
    rate_limiter: Option<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    cache: Cache,
}

pub enum PacketGeneratorStatus<'a> {
    Success(&'a [u8], u16),
    Wait(u64),
    Stop,
}

impl QueryProducer {
    pub fn new(argument: Argument) -> QueryProducer {
        let mut stop_at = 0;
        if argument.until_stop > 0 {
            stop_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + argument.until_stop as u64;
        };

        let mut limiter = {
            if argument.client >= 1 {
                argument.qps / argument.client
            } else {
                1
            }
        } as u32;
        let mut max = {
            if argument.client >= 1 {
                argument.max / argument.client
            } else {
                1
            }
        } as u32;
        QueryProducer {
            store: QueryStatusStore::new(),
            counter: 0,
            max_counter: max as u64,
            stop_at,
            start_send: std::time::SystemTime::now(),
            rate_limiter: {
                if argument.qps == 0 {
                    None
                } else {
                    Some(RateLimiter::direct(
                        Quota::per_second(NonZeroU32::new(limiter).expect("qps setting error"))
                            .allow_burst(NonZeroU32::new(1).unwrap()),
                    ))
                }
            },
            cache: Cache::new(&argument.clone()),
        }
    }
    pub fn retrieve(&mut self) -> PacketGeneratorStatus {
        if let Some(limiter) = self.rate_limiter.as_ref() {
            match limiter.check() {
                Err(err) => {
                    let sleep = err
                        .earliest_possible()
                        .duration_since(QuantaClock::default().now());
                    return PacketGeneratorStatus::Wait(sleep.into());
                }
                _ => {}
            };
        }
        // max counter limit && max duration limit
        if (self.max_counter != 0 && self.counter >= self.max_counter)
            || (self.stop_at != 0
                && SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    >= self.stop_at)
        {
            self.store.set_query_total(self.counter as usize);
            self.store
                .set_send_duration(self.start_send.elapsed().unwrap());
            return PacketGeneratorStatus::Stop;
        }
        let message = self.cache.build_message();
        self.counter = self.counter + 1;

        PacketGeneratorStatus::Success(message.0, message.1)
    }
}
