use ethabi;
use serde_json;
use std::io;
use tokio_timer::{TimeoutError, TimerError};
use web3;

error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        Io(io::Error);
        Timer(TimerError);
        Web3(web3::Error);
        Ethabi(ethabi::Error);
        Json(serde_json::Error);
    }

    errors {
        TimedOut {
            description("Request timed out"),
            display("Request timed out"),
        }

        /// Unable to schedule wake up.
        FaultyTimer(e: ::tokio::timer::Error) {
            description("Timer error"),
            display("Timer error: {}", e),
        }

        /// Unable to find file.
        UnknownFile(file: String) {
            description("File not found"),
            display("File {} not found", file),
        }
    }
}

impl<F> From<TimeoutError<F>> for Error {
    fn from(err: TimeoutError<F>) -> Self {
        match err {
            TimeoutError::Timer(_, timer_error) => timer_error.into(),
            TimeoutError::TimedOut(_) => ErrorKind::TimedOut.into(),
        }
    }
}
