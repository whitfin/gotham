/// Middleware for the Gotham framework to log on requests made to the server.
///
/// This implementation is quite bare at the moment and will log out using the
/// [Common Log Format](https://en.wikipedia.org/wiki/Common_Log_Format) (CLF).
extern crate chrono;
extern crate futures;
extern crate gotham;
#[macro_use]
extern crate gotham_derive;
extern crate hyper;
#[macro_use]
extern crate log;

// all of our imports
use chrono::prelude::*;
use futures::{future, Future};
use gotham::handler::HandlerFuture;
use gotham::middleware::Middleware;
use gotham::state::{client_addr, FromState, State};
use hyper::{HttpVersion, Method, Uri, header::ContentLength};
use log::Level;

/// A struct that can act as a logging middleware for Gotham.
///
/// We implement `NewMiddleware` here for Gotham to allow us to work with the request
/// lifecycle correctly. This trait requires `Clone`, so that is also included.
#[derive(Clone, NewMiddleware)]
pub struct LoggingMiddleware {
    duration: bool,
    level: Level,
}

/// Main implementation for `LoggingMiddleware` to enable various configuration.
impl LoggingMiddleware {
    /// Creates a new `LoggingMiddleware` using the provided log level.
    pub fn with_level(level: Level) -> LoggingMiddleware {
        LoggingMiddleware::with_level_and_duration(level, false)
    }

    /// Creates a new `LoggingMiddleware` using the provided log level, with duration
    /// optionally attached to the end of log messages.
    pub fn with_level_and_duration(level: Level, duration: bool) -> LoggingMiddleware {
        LoggingMiddleware { level, duration }
    }
}

/// Implementing `gotham::middleware::Middleware` allows us to hook into the request chain
/// in order to correctly log out after a request has executed.
impl Middleware for LoggingMiddleware {
    fn call<Chain>(self, state: State, chain: Chain) -> Box<HandlerFuture>
    where
        Chain: FnOnce(State) -> Box<HandlerFuture>,
    {
        // skip everything if logging is disabled
        if !log_enabled!(self.level) {
            return chain(state);
        }

        // extract the current time
        let start_time = Utc::now();

        // hook onto the end of the request to log the access
        let f = chain(state).and_then(move |(state, response)| {
            // format the start time to the CLF formats
            let datetime = start_time.format("%d/%b/%Y:%H:%M:%S %z");

            // grab the ip address from the state
            let ip = client_addr(&state).unwrap().ip();

            // calculate duration
            let duration = {
                // disabled, so skip
                if !self.duration {
                    "".to_owned()
                } else {
                    // calculate microsecond offset from start
                    let micros_offset = Utc::now()
                        .signed_duration_since(start_time)
                        .num_microseconds()
                        .unwrap();

                    // format into a more readable format
                    if micros_offset < 1000 {
                        format!(" - {}µs", micros_offset)
                    } else if micros_offset < 1000000 {
                        format!(" - {:.2}ms", (micros_offset as f32) / 1000.0)
                    } else {
                        format!(" - {:.2}s", (micros_offset as f32) / 1000000.0)
                    }
                }
            };

            {
                // borrows from the state
                let path = Uri::borrow_from(&state);
                let method = Method::borrow_from(&state);
                let version = HttpVersion::borrow_from(&state);

                // take references based on the response
                let status = response.status().as_u16();
                let length = response.headers().get::<ContentLength>().unwrap();

                // log out
                log!(
                    self.level,
                    "{} - - [{}] \"{} {} {}\" {} {} {}",
                    ip,
                    datetime,
                    method,
                    path,
                    version,
                    status,
                    length,
                    duration
                );
            }

            // continue the response chain
            future::ok((state, response))
        });

        // box it up
        Box::new(f)
    }
}
