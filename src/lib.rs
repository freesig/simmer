use error::{SimmerError, SimmerResult};
use futures::future::FutureExt;
use lazy_static::lazy_static;
use std::{
    any::TypeId,
    collections::HashMap,
    convert::{TryFrom, TryInto},
    sync::{Arc, Once},
};
use tokio::stream::StreamExt;
use tokio::sync;
use tracing::*;

pub mod error;

lazy_static! {
    static ref REGISTRY: Registry = {
        let (request, end) = sync::mpsc::channel(1);
        let end = std::sync::Mutex::new(Some(end));
        let (shutdown, shutdown_end) = sync::oneshot::channel();
        let shutdown = std::sync::Mutex::new(Some(shutdown));
        let shutdown_end = std::sync::Mutex::new(Some(shutdown_end));
        Registry {
            request,
            end,
            shutdown,
            shutdown_end,
        }
    };
}

macro_rules! ok {
    ($x:expr, $event:expr) => {
        $x.map_err(|e| {
            $event;
            e
        })
        .ok()
    };
}

struct Registry {
    pub request: sync::mpsc::Sender<ChannelRequest>,
    // Should only ever be one call to shutdown
    // If it's contested it's at the end of the program anyway
    pub shutdown: std::sync::Mutex<Option<sync::oneshot::Sender<()>>>,
    end: std::sync::Mutex<Option<sync::mpsc::Receiver<ChannelRequest>>>,
    shutdown_end: std::sync::Mutex<Option<sync::oneshot::Receiver<()>>>,
}

static START: Once = Once::new();

pub async fn registry() {
    if !START.is_completed() {
        let (tx, rx) = sync::oneshot::channel();
        START.call_once(|| {
            if tx.send(setup_registry()).is_err() {
                panic!("Bug: Failed to send setup registry");
            }
        });
        match rx.await {
            Err(_) => warn!("Couldn't await the setup future, may have already been run"),
            Ok(r) => r.await,
        }
    }
}

async fn setup_registry() {
    let mut channels = ChannelRegistry::new();
    let mut end = {
        REGISTRY
            .end
            .lock()
            .expect("Bug: Multiple registries")
            .take()
            .expect("Bug: Registry was not initialized")
    };
    let shutdown = {
        REGISTRY
            .shutdown_end
            .lock()
            .expect("Bug: Multiple registries")
            .take()
            .expect("Bug: Registry was not initialized")
    };
    let mut shutdown = shutdown.into_stream();
    loop {
        let f1 = end.next();
        let f2 = shutdown.next();
        tokio::pin!(f1);
        tokio::pin!(f2);
        if let futures::future::Either::Left((Some(request), _)) =
            futures::future::select(f1, f2).await
        {
            let channel = request.channel.clone();
            let r = request
                .response
                .send(channels.get_or_create(&request.channel));
            ok!(
                r,
                error!(msg = "Failed to get channel", actor = channel.actor, direction = ?channel.direction, side = ?channel.side)
            );
        } else {
            break;
        }
    }
}

const CAPACITY: usize = 1;

pub type Opaque = Arc<dyn std::any::Any + Send + Sync + 'static>;
pub type StartType<T> = sync::broadcast::Sender<T>;
pub type EndType<T> = sync::broadcast::Receiver<T>;

pub enum Channel<T> {
    Start(StartType<T>),
    End(EndType<T>),
}
enum OpaqueChannel {
    Start(Opaque),
    End(Opaque),
}

struct ChannelRequest {
    pub channel: ActorChannel,
    response: sync::oneshot::Sender<OpaqueChannel>,
}

type Actor = &'static str;
type ChannelName = &'static str;

#[derive(Hash, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Direction {
    In,
    Out,
}

#[derive(Hash, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Side {
    Start,
    End,
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
struct ActorChannel {
    pub actor: Actor,
    pub channel_name: ChannelName,
    pub direction: Direction,
    pub side: Side,
    pub msg_type: TypeId,
    pub create: fn() -> Opaque,
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
struct Key {
    pub actor: Actor,
    pub channel_name: ChannelName,
    pub msg_type: TypeId,
}

struct ChannelRegistry {
    registry: HashMap<Key, Opaque>,
}

impl From<(&Side, Opaque)> for OpaqueChannel {
    fn from((side, opaque_channel): (&Side, Opaque)) -> Self {
        match side {
            Side::Start => Self::Start(opaque_channel),
            Side::End => Self::End(opaque_channel),
        }
    }
}

impl ChannelRegistry {
    fn new() -> Self {
        let registry = HashMap::new();
        ChannelRegistry { registry }
    }

    fn get_or_create(&mut self, request: &ActorChannel) -> OpaqueChannel {
        let key = Key::from(request);
        match self.registry.get(&key) {
            Some(o) => (&request.side, o.clone()).into(),
            None => {
                let new_channel = request.create;
                let opaque_channel = new_channel();
                let channel_side = (&request.side, opaque_channel.clone()).into();
                self.registry.insert(key, opaque_channel);
                channel_side
            }
        }
    }
}

impl From<&ActorChannel> for Key {
    fn from(request: &ActorChannel) -> Self {
        Self {
            actor: request.actor,
            msg_type: request.msg_type,
            channel_name: request.channel_name,
        }
    }
}

fn create<M: Clone + Send + 'static>() -> Opaque {
    let (start, _) = sync::broadcast::channel::<M>(CAPACITY);
    Arc::new(start)
}

fn msg_type<T: 'static>() -> TypeId {
    TypeId::of::<T>()
}

impl<T> TryFrom<OpaqueChannel> for Channel<T>
where
    T: Send + 'static,
{
    type Error = SimmerError;
    fn try_from(cs: OpaqueChannel) -> Result<Self, Self::Error> {
        match cs {
            OpaqueChannel::Start(p) => p
                .downcast::<StartType<T>>()
                .map(|p| Channel::Start((*p).clone()))
                .map_err(|_| SimmerError::DowncastError),
            OpaqueChannel::End(p) => p
                .downcast::<StartType<T>>()
                .map(|p| Channel::End((*p).subscribe()))
                .map_err(|_| SimmerError::DowncastError),
        }
    }
}

impl<T> TryFrom<Channel<T>> for StartType<T> {
    type Error = SimmerError;
    fn try_from(c: Channel<T>) -> Result<Self, Self::Error> {
        match c {
            Channel::Start(s) => Ok(s),
            Channel::End(_) => Err(SimmerError::WrongSide("Start", "End")),
        }
    }
}

impl<T> TryFrom<Channel<T>> for EndType<T> {
    type Error = SimmerError;
    fn try_from(c: Channel<T>) -> Result<Self, Self::Error> {
        match c {
            Channel::End(r) => Ok(r),
            Channel::Start(_) => Err(SimmerError::WrongSide("End", "Start")),
        }
    }
}

pub async fn get_channel_end<T: Clone + Send + 'static>(
    actor: Actor,
    channel_name: ChannelName,
    direction: Direction,
) -> SimmerResult<EndType<T>> {
    EndType::try_from(get_channel(actor, channel_name, direction, Side::End).await?)
}

pub async fn get_channel_start<T: Clone + Send + 'static>(
    actor: Actor,
    channel_name: ChannelName,
    direction: Direction,
) -> SimmerResult<StartType<T>> {
    StartType::try_from(get_channel(actor, channel_name, direction, Side::Start).await?)
}

pub async fn get_channel<T: Clone + Send + 'static>(
    actor: Actor,
    channel_name: ChannelName,
    direction: Direction,
    side: Side,
) -> SimmerResult<Channel<T>> {
    let registry = REGISTRY.request.clone();
    let (response, publisher) = sync::oneshot::channel();
    let request = ChannelRequest {
        channel: ActorChannel {
            direction,
            side,
            actor,
            msg_type: msg_type::<T>(),
            create: create::<T>,
            channel_name,
        },
        response,
    };

    let r = registry.send(request).await;
    ok!(r, error!("Failed to send registry request"));

    publisher
        .await
        .map_err(|_| SimmerError::GetChannel(actor.to_string()))?
        .try_into()
}

pub const ONLINE: &'static str = "SIMMER_CHANNEL_ONLINE";

pub async fn wait_till_online(actor: Actor) -> SimmerResult<()> {
    let online_out_e = get_channel::<()>(actor, ONLINE, Direction::Out, Side::End).await?;
    let mut online_out_e: EndType<_> = online_out_e.try_into()?;

    loop {
        match online_out_e.recv().await {
            Err(sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(sync::broadcast::error::RecvError::Closed) => {
                panic!("Bug: Channel should never be able to close after a get_channel")
            }
            Ok(_) => break,
        }
    }
    Ok(())
}

pub async fn shutdown_registry() -> SimmerResult<()> {
    REGISTRY
        .shutdown
        .lock()
        .ok()
        .and_then(|mut s| s.take().and_then(|s| s.send(()).ok()))
        .ok_or(SimmerError::Shutdown)
}
