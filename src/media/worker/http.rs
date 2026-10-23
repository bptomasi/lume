//! Bounded read polling interrupts obsolete HTTP without extra download threads.
//! Ureq retains proxy/TLS verification, pooling, framing, redirects and limits.
use anyhow::Result;
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use ureq::unversioned::{
    resolver::DefaultResolver,
    transport::{
        Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout,
        Transport,
    },
};

#[derive(Clone, Debug, Default)]
struct Active(Arc<Mutex<Option<Arc<AtomicBool>>>>);

impl Active {
    fn check(&self) -> std::result::Result<(), ureq::Error> {
        let canceled = self
            .0
            .lock()
            .map_err(|_| io::Error::other("Image request stopped"))?
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire));
        if canceled {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Image request canceled",
            )
            .into());
        }
        Ok(())
    }
}

pub(super) struct Client {
    agent: ureq::Agent,
    active: Active,
}
impl Client {
    pub(super) fn new() -> Self {
        let active = Active::default();
        let config = crate::source::agent().config().clone();
        let agent = ureq::Agent::with_parts(
            config,
            CancelConnector(active.clone()),
            DefaultResolver::default(),
        );
        Self { agent, active }
    }
    pub(super) fn download(&self, request: &super::Request) -> Result<Vec<u8>> {
        *self
            .active
            .0
            .lock()
            .map_err(|_| io::Error::other("Image request stopped"))? =
            Some(request.canceled.clone());
        let result = super::download(&self.agent, &request.url);
        *self
            .active
            .0
            .lock()
            .map_err(|_| io::Error::other("Image request stopped"))? = None;
        result
    }
}

#[derive(Debug)]
struct CancelConnector(Active);
impl Connector for CancelConnector {
    type Out = CancelTransport;
    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<()>,
    ) -> std::result::Result<Option<Self::Out>, ureq::Error> {
        self.0.check()?;
        Ok(DefaultConnector::default().connect(details, chained)?.map(
            |inner| CancelTransport {
                inner,
                active: self.0.clone(),
            },
        ))
    }
}

#[derive(Debug)]
struct CancelTransport {
    inner: Box<dyn Transport>,
    active: Active,
}
impl Transport for CancelTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }
    fn transmit_output(
        &mut self,
        amount: usize,
        timeout: NextTimeout,
    ) -> std::result::Result<(), ureq::Error> {
        self.active.check()?;
        self.inner.transmit_output(amount, timeout)
    }
    fn await_input(
        &mut self,
        timeout: NextTimeout,
    ) -> std::result::Result<bool, ureq::Error> {
        let started = Instant::now();
        loop {
            self.active.check()?;
            let remaining = (*timeout.after).saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(ureq::Error::Timeout(timeout.reason));
            }
            let poll = NextTimeout {
                after: remaining.min(Duration::from_millis(50)).into(),
                reason: timeout.reason,
            };
            match self.inner.await_input(poll) {
                Err(ureq::Error::Timeout(_)) => {}
                result => return result,
            }
        }
    }
    fn is_open(&mut self) -> bool {
        self.active.check().is_ok() && self.inner.is_open()
    }
    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}
