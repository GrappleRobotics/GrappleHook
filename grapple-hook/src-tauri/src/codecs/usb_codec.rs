use bounded_static::IntoBoundedStatic;
use bytes::{BufMut, Buf};
use grapple_frc_msgs::{binmarshal::{BitView, BitWriter, Demarshal, Marshal, VecBitWriter}, bridge::BridgedCANMessage, MessageId};
use tokio_util::codec::{Decoder, Encoder};

pub struct GrappleUsbCodec;

impl Decoder for GrappleUsbCodec {
  type Item = (MessageId, Vec<u8>);
  type Error = anyhow::Error;

  fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if src.len() < 2 {
      return Ok(None);
    }

    let mut len_bytes: [u8; 2] = [0u8; 2];
    len_bytes.copy_from_slice(&src[..2]);
    let length = u16::from_le_bytes(len_bytes) as usize;

    if src.len() < 2 + length {
      src.reserve(2 + length - src.len());
      return Ok(None);
    }

    let data = src[2..2+length].to_vec();
    src.advance(2 + length);

    let msg_id = MessageId::from(u32::from_le_bytes([data[0], data[1], data[2], data[3]]));
    let actual_data = data[4..].to_vec();

    Ok(Some((msg_id, actual_data)))
  }
}

impl Encoder<(MessageId, Vec<u8>)> for GrappleUsbCodec {
  type Error = anyhow::Error;

  fn encode(&mut self, item: (MessageId, Vec<u8>), dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
    // let mut writer = VecBitWriter::new();
    // item.write(&mut writer, ()).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))?;
    // let bytes = writer.slice();
    
    dst.reserve(2 + 4 + item.1.len());
    dst.put(&u16::to_le_bytes(4 + item.1.len() as u16)[..]);
    dst.put(&u32::to_le_bytes(item.0.into())[..]);
    dst.put(&item.1[..]);
    Ok(())
  }
}