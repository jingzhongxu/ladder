use ethabi::RawLog;
use futures::{Async, Future, Poll, Stream};
use web3::types::Log;

pub trait IntoRawLog {
    fn into_raw_log(&self) -> RawLog;
}

impl IntoRawLog for Log {
    fn into_raw_log(&self) -> RawLog {
        RawLog {
            topics: self.topics.iter().map(|t| t.0.into()).collect(),
            data: self.data.0.clone(),
        }
    }
}

/// extends the `Stream` trait by the `last` function
pub trait StreamExt<I> {
    /// if you're interested only in the last item in a stream
    fn last(self) -> Last<Self, I>
    where
        Self: Sized;
}

impl<S, I> StreamExt<I> for S
where
    S: Stream,
{
    fn last(self) -> Last<Self, I>
    where
        Self: Sized,
    {
        Last {
            stream: self,
            last: None,
        }
    }
}

/// `Future` that wraps a `Stream` and completes with the last
/// item in the stream once the stream is over.
pub struct Last<S, I> {
    stream: S,
    last: Option<I>,
}

impl<S, I> Future for Last<S, I>
where
    S: Stream<Item = I>,
{
    type Item = Option<I>;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Self::Item, S::Error> {
        loop {
            match self.stream.poll() {
                Err(err) => return Err(err),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                // stream is finished
                Ok(Async::Ready(None)) => return Ok(Async::Ready(self.last.take())),
                // there is more
                Ok(Async::Ready(item)) => self.last = item,
            }
        }
    }
}
