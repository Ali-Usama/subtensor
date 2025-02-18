//! A custom RPC method to listen to new extrinsic and broadcast them to subscribers.
//! It listens for a new extrinsic in the transaction pool
//! Notifies RPC subscribers when an extrinsic is received
//! Emits the extrinsic hash to the subscribed clients

use futures::StreamExt;
use jsonrpsee::core::{async_trait, SubscriptionResult};
use jsonrpsee::{proc_macros::rpc};
use jsonrpsee::{PendingSubscriptionSink, SubscriptionMessage};
use sp_core::H256;

/// RPC Subscription API for listening to new extrinsics.
#[rpc(client, server)]
pub trait ExtrinsicSubscriptionApi {
    /// Subscribe to new extrinsic received by the node.
    #[subscription(name = "subscribe_new_extrinsic", item = String)]
    async fn extrinsic_subscription(&self) -> SubscriptionResult;
}

pub struct ExtrinsicSubscription<P> {
    pool: std::sync::Arc<P>,
    sender: tokio::sync::broadcast::Sender<H256>,
}

impl<P> ExtrinsicSubscription<P>
where
    P: sc_transaction_pool_api::TransactionPool<Block = node_subtensor_runtime::opaque::Block, Hash = H256> + Send + Sync + 'static,
{
    pub fn new(pool: std::sync::Arc<P>) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(1000);
        Self { pool, sender }
    }

    pub fn start_listener(&self) {
        let pool = self.pool.clone();
        let sender = self.sender.clone();

        tokio::spawn(async move {
            let mut notification_stream = pool.import_notification_stream();

            while let Some(notification) = notification_stream.next().await {
                if sender.send(notification).is_err() {
                    log::warn!("No active subscribers for new extrinsic");
                }
            }
        });
    }
}

#[async_trait]
impl<P> ExtrinsicSubscriptionApiServer for ExtrinsicSubscription<P>
where
    P: sc_transaction_pool_api::TransactionPool<Block = node_subtensor_runtime::opaque::Block, Hash = H256> + Send + Sync + 'static,
{
      async fn extrinsic_subscription(&self, pending_subscription_sink: PendingSubscriptionSink) -> SubscriptionResult {
        let mut receiver = self.sender.subscribe();

          let Ok(subscription_sink) = pending_subscription_sink.accept().await else {
              return Ok(())
          };

        tokio::spawn(async move {
            while let Ok(hash) = receiver.recv().await {
                let message = SubscriptionMessage::try_from(hash.to_string()).unwrap();
                if subscription_sink.send(message).await.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }
}