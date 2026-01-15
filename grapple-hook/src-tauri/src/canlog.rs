use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, AtomicUsize},
    time::Instant,
};

use crate::rpc::RpcBase;
use grapple_frc_msgs::{
    bridge::BridgedCANMessage,
    grapple::{GrappleDeviceMessage, GrappleMessageId},
    MessageId,
};
use grapple_hook_macros::rpc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};

use bounded_static::{ToBoundedStatic, ToStatic};

#[derive(Debug, Clone, Serialize, JsonSchema, ToStatic)]
pub struct MailboxItem<'a> {
    pub seq: usize,
    #[serde(borrow)]
    pub raw: BridgedCANMessage<'a>,
    #[serde(borrow)]
    pub grpl_defrag: Option<GrappleDeviceMessage<'a>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum Filter {
    GrappleOnly,
    IdMask { id: MessageId, mask: MessageId },
    IdMaskRaw { id: u32, mask: u32 },
    BodySize { min: u8, max: u8 },
}

impl Filter {
    pub fn accept<'a>(
        &self,
        msg: &BridgedCANMessage<'a>,
        defrag: &Option<&GrappleDeviceMessage<'a>>,
    ) -> bool {
        match self {
            Filter::GrappleOnly => defrag.is_some(),
            Filter::IdMask { id, mask } => {
                let id_raw: u32 = id.clone().into();
                let mask_raw: u32 = mask.clone().into();

                let msg_id_raw: u32 = msg.id.clone().into();

                (msg_id_raw & mask_raw) == (id_raw & mask_raw)
            }
            Filter::IdMaskRaw { id, mask } => {
                let msg_id_raw: u32 = msg.id.clone().into();

                (msg_id_raw & (*mask)) == ((*id) & (*mask))
            }
            Filter::BodySize { min, max } => {
                msg.data.len() >= (*min as usize) && msg.data.len() <= (*max as usize)
            }
        }
    }
}

pub struct CanLog {
    logging_enabled: AtomicBool,
    max_mailbox_size: usize,
    mailbox: RwLock<VecDeque<MailboxItem<'static>>>,
    seq: AtomicUsize,
    filters: RwLock<Vec<Filter>>,
    can_send_raw_tx: mpsc::Sender<(MessageId, Vec<u8>)>,
    rel_epoch: Instant,
}

impl CanLog {
    pub fn new(max_size: usize, can_send_raw_tx: mpsc::Sender<(MessageId, Vec<u8>)>) -> Self {
        Self {
            logging_enabled: AtomicBool::new(false),
            max_mailbox_size: max_size,
            mailbox: RwLock::new(VecDeque::with_capacity(max_size)),
            seq: AtomicUsize::new(0),
            filters: RwLock::new(Vec::new()),
            can_send_raw_tx,
            rel_epoch: Instant::now(),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.logging_enabled
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    pub async fn on_message<'a>(
        &self,
        msg: &BridgedCANMessage<'a>,
        defrag: Option<&GrappleDeviceMessage<'a>>,
    ) {
        // Need to retime since incoming messages will have different timestamps depending on whether GrappleHook sent them
        // or sniffed them.
        let elapsed = self.rel_epoch.elapsed().as_millis() as u32;

        for filter in self.filters.read().await.iter() {
            if !filter.accept(msg, &defrag) {
                return;
            }
        }

        let seq = self.seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if self
            .logging_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let item = MailboxItem {
                seq,
                raw: BridgedCANMessage {
                    id: msg.id,
                    timestamp: elapsed,
                    data: msg.data.to_static(),
                },
                grpl_defrag: defrag.map(ToBoundedStatic::to_static),
            };

            let mut q = self.mailbox.write().await;
            while q.len() >= (self.max_mailbox_size - 1) {
                q.pop_front();
            }

            q.push_back(item);
        }
    }
}

#[rpc]
impl CanLog {
    async fn set_log_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        self.set_enabled(enabled);
        Ok(())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        self.mailbox.write().await.clear();
        Ok(())
    }

    async fn read_after(&self, seq: usize) -> anyhow::Result<Vec<MailboxItem<'static>>> {
        let q = self.mailbox.read().await;
        Ok(q.iter().filter(|x| x.seq > seq).cloned().collect())
    }

    async fn set_filters(&self, filters: Vec<Filter>) -> anyhow::Result<()> {
        (*self.filters.write().await) = filters;
        Ok(())
    }

    async fn send_raw(&self, id: MessageId, data: Vec<u8>) -> anyhow::Result<()> {
        self.can_send_raw_tx.send((id, data)).await?;
        Ok(())
    }
}
