use std::io;
use futures::future::join_all;

use log::{warn, info, debug};
use tokio::task::JoinHandle;

use crate::config::{EndpointConfig, EpHalfConfig};

mod net;
mod transport;
pub mod common;

#[cfg(target_os = "linux")]
pub fn meet_zero_copy(listen: &EpHalfConfig, remote: &EpHalfConfig) -> bool {
    use crate::config::TransportConfig;
    matches!(
        (&listen.trans, &remote.trans),
        (TransportConfig::Plain, TransportConfig::Plain)
    ) && listen.net.is_zero_copy()
        && remote.net.is_zero_copy()
}

pub async fn run(eps: Vec<EndpointConfig>) {
    let mut count: usize = 0;
    let mut workers: Vec<JoinHandle<io::Result<()>>> =
        Vec::with_capacity(eps.len());
    for ep in eps.into_iter() {
        debug!("load next endpoint");
        // convert into full config
        let EndpointConfig { listen, remote } = ep;
        let listen: EpHalfConfig = listen.into();
        let remote: EpHalfConfig = remote.into();

        // create zero-copy task
        #[cfg(target_os = "linux")]
        if meet_zero_copy(&listen, &remote) {
            use crate::io::linux_ext::splice;
            let lis = net::new_plain_lis(&listen.addr, &listen.net);
            let conn = net::new_plain_conn(&remote.addr, &remote.net);
            workers.push(tokio::spawn(splice(lis, conn)));
            continue;
        }

        // load transport config and create task
        net::spawn_with_net(&mut workers, &listen, &remote);
        count += 1;
        info!("new endpoint inited[{}]", count);
    }
    warn!("altogether {} endpoints", count);
    warn!("service start");
    join_all(workers).await;
}
