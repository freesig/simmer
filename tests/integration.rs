use simmer::*;
use std::convert::TryInto;
use tracing::*;

mod observability;

#[tokio::test(threaded_scheduler)]
async fn hello_world() {
    observability::test_run().ok();
    let r = tokio::task::spawn(registry());
    let h = tokio::task::spawn(hello());
    let w = tokio::task::spawn(world());
    let (r, h, w) = tokio::join!(r, h, w);
    h.unwrap();
    w.unwrap();
    r.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn hello_world2() {
    observability::test_run().ok();
    let r = tokio::task::spawn(registry());
    let w = tokio::task::spawn(world());
    let h = tokio::task::spawn(hello());
    let (r, h, w) = tokio::join!(r, h, w);
    h.unwrap();
    w.unwrap();
    r.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn hello_world3() {
    observability::test_run().ok();
    let r = tokio::task::spawn(registry());
    let w = tokio::task::spawn(world());
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    let h = tokio::task::spawn(hello());
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    let (r, h, w) = tokio::join!(r, h, w);
    h.unwrap();
    w.unwrap();
    r.unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn hello_world4() {
    observability::test_run().ok();
    let r = tokio::task::spawn(registry());
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    let h = tokio::task::spawn(hello());
    tokio::time::delay_for(std::time::Duration::from_millis(100)).await;
    let w = tokio::task::spawn(world());
    let (r, h, w) = tokio::join!(r, h, w);
    h.unwrap();
    w.unwrap();
    r.unwrap();
}

#[instrument]
async fn hello() {
    debug!(l = line!());
    let world_in_s: StartType<_> =
        get_channel::<String>("world", "talk", Direction::In, Side::Start)
            .await
            .unwrap()
            .try_into()
            .unwrap();
    debug!(l = line!());
    let mut msg = "hello".to_string();
    while let Err(m) = world_in_s.send(msg) {
        wait_till_online("world").await.unwrap();
        msg = m.0;
    }
    debug!(l = line!());
}

#[instrument]
async fn world() {
    debug!(l = line!());
    let mut world_in_e: EndType<_> =
        get_channel::<String>("world", "talk", Direction::In, Side::End)
            .await
            .unwrap()
            .try_into()
            .unwrap();
    debug!(l = line!());
    // Try again if lagged
    let hello = loop {
        match tokio::time::timeout(std::time::Duration::from_millis(20), world_in_e.recv()).await {
            Ok(Err(tokio::sync::broadcast::RecvError::Lagged(_))) => {
                debug!("lagged");
                tokio::spawn(hello());
            }
            Err(_) => {
                debug!("timedout");
                tokio::spawn(hello());
            }
            r => break r.unwrap().unwrap(),
        }
    };
    debug!(l = line!());
    let hello_world = format!("{} world", hello);
    debug!(l = line!());
    assert_eq!(hello_world, "hello world");
    tokio::time::delay_for(std::time::Duration::from_secs(2)).await;
    shutdown_registry().await.ok();
    debug!(l = line!());
}
