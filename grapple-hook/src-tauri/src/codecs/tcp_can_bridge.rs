use bounded_static::IntoBoundedStatic;
use bytes::{BufMut, Buf};
use grapple_frc_msgs::{binmarshal::{BitView, VecBitWriter, BitWriter, Marshal, Demarshal}, bridge::BridgedCANMessage};
use tokio_util::codec::{Decoder, Encoder};

pub struct GrappleTcpCanBridgeCodec;

impl Decoder for GrappleTcpCanBridgeCodec {
  type Item = BridgedCANMessage<'static>;
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

    BridgedCANMessage::read(&mut BitView::new(&data[..]), ())
      .map(|x| Some(x.into_static()))
      .map_err(|e| anyhow::anyhow!(format!("{:?}", e)))
  }
}

impl Encoder<BridgedCANMessage<'_>> for GrappleTcpCanBridgeCodec {
  type Error = anyhow::Error;

  fn encode(&mut self, item: BridgedCANMessage<'_>, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
    let mut writer = VecBitWriter::new();
    item.write(&mut writer, ()).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))?;
    let bytes = writer.slice();
    
    dst.reserve(2 + bytes.len());
    dst.put(&(bytes.len() as u16).to_le_bytes()[..]);
    dst.put(&bytes[..]);
    Ok(())
  }
}