use super::{
    CurrentOperation, Exchange, GattTransport, Packet, TransportError, TransportFuture,
    protocol::frame,
};
use bluer::{Address, gatt::remote::Characteristic};
use futures_util::{Stream, StreamExt};
use std::{pin::Pin, sync::Arc, time::Duration};

const WRITE: uuid::Uuid = uuid::Uuid::from_u128(0x00010203_0405_0607_0809_0a0b0c0d2b11);
const NOTIFY: uuid::Uuid = uuid::Uuid::from_u128(0x00010203_0405_0607_0809_0a0b0c0d2b10);

pub struct Bluetooth {
    address: Address,
}
impl Bluetooth {
    pub fn configure(address: &str) -> Result<Arc<dyn GattTransport>, &'static str> {
        Ok(Arc::new(Self {
            address: address
                .parse()
                .map_err(|_| "Invalid BACKLIGHT_BLUETOOTH_ADDRESS")?,
        }))
    }
    async fn exchange_inner(
        &self,
        writes: &[Packet],
        queries: &[Vec<u8>],
        current: &CurrentOperation,
    ) -> Result<Exchange, TransportError> {
        let session = bluer::Session::new()
            .await
            .map_err(|_| TransportError::Unavailable)?;
        let adapter = session
            .default_adapter()
            .await
            .map_err(|_| TransportError::Unavailable)?;
        // Discovery is needed when BlueZ has no cached device after host startup.
        let discovery = adapter
            .discover_devices()
            .await
            .map_err(|_| TransportError::Unavailable)?;
        tokio::pin!(discovery);
        let device = adapter
            .device(self.address)
            .map_err(|_| TransportError::Unavailable)?;
        if !adapter
            .device_addresses()
            .await
            .map_err(|_| TransportError::Unavailable)?
            .contains(&self.address)
        {
            while let Some(event) = discovery.next().await {
                if matches!(event, bluer::AdapterEvent::DeviceAdded(address) if address == self.address)
                {
                    break;
                }
            }
        }
        let reconnected = !device
            .is_connected()
            .await
            .map_err(|_| TransportError::Unavailable)?;
        if reconnected {
            device
                .connect()
                .await
                .map_err(|_| TransportError::Unavailable)?;
        }
        let operation = async {
            let mut write = None;
            let mut notify = None;
            for service in device
                .services()
                .await
                .map_err(|_| TransportError::Unavailable)?
            {
                for characteristic in service
                    .characteristics()
                    .await
                    .map_err(|_| TransportError::Unavailable)?
                {
                    match characteristic
                        .uuid()
                        .await
                        .map_err(|_| TransportError::Unavailable)?
                    {
                        WRITE => write = Some(characteristic),
                        NOTIFY => notify = Some(characteristic),
                        _ => (),
                    }
                }
            }
            let write = write.ok_or(TransportError::Unavailable)?;
            let notify = notify.ok_or(TransportError::Unavailable)?;
            let notifications = notify
                .notify()
                .await
                .map_err(|_| TransportError::Unavailable)?;
            tokio::pin!(notifications);
            if !current() {
                return Err(TransportError::Superseded);
            }
            for packet in writes {
                write
                    .write(packet)
                    .await
                    .map_err(|_| TransportError::Unavailable)?;
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            let mut replies = Vec::new();
            for query in queries {
                match query_reply(&write, notifications.as_mut(), query).await {
                    Ok(reply) => replies.push(reply),
                    // Completed writes remain acknowledged even when readback times out.
                    Err(_) => break,
                }
            }
            Ok(Exchange {
                reconnected,
                writes_accepted: !writes.is_empty(),
                replies,
            })
        };
        let result = tokio::time::timeout(Duration::from_secs(18), operation)
            .await
            .unwrap_or(Err(TransportError::Unavailable));
        // Keep a healthy link across commands. Disconnecting after Off races the
        // controller's advertising restart when TV Sync is requested immediately.
        if matches!(result, Err(TransportError::Unavailable)) {
            let _ = tokio::time::timeout(Duration::from_secs(2), device.disconnect()).await;
        }
        result
    }
}
impl GattTransport for Bluetooth {
    fn matches_device(&self, identity: &str) -> bool {
        identity
            .parse::<Address>()
            .is_ok_and(|address| address == self.address)
    }
    fn exchange<'a>(
        &'a self,
        writes: &'a [Packet],
        queries: &'a [Vec<u8>],
        current: &'a CurrentOperation,
    ) -> TransportFuture<'a> {
        Box::pin(self.exchange_inner(writes, queries, current))
    }
}
async fn query_reply(
    write: &Characteristic,
    mut notifications: Pin<&mut (impl Stream<Item = Vec<u8>> + Send)>,
    query: &[u8],
) -> Result<Packet, TransportError> {
    write
        .write(&frame(query))
        .await
        .map_err(|_| TransportError::Unavailable)?;
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(reply) = notifications.next().await {
            let Ok(packet) = Packet::try_from(reply.as_slice()) else {
                continue;
            };
            if packet[..2] == query[..2] && packet.iter().fold(0, |a, b| a ^ b) == 0 {
                return Ok(packet);
            }
        }
        Err(TransportError::Unavailable)
    })
    .await
    .unwrap_or(Err(TransportError::Unavailable))
}
