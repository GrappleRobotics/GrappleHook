use bytes::{BufMut, Buf};
use grapple_frc_msgs::{grapple::tcp::GrappleTCPMessage, binmarshal::{BinMarshal, rw::{BitView, VecBitWriter, BitWriter}}};
use tokio_util::codec::{Decoder, Encoder};

pub struct GrappleTcpCodec {}

impl Decoder for GrappleTcpCodec {
  type Item = GrappleTCPMessage;
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

    GrappleTCPMessage::read(&mut BitView::new(&data[..]), ()).map(|v| Some(v)).ok_or(anyhow::anyhow!("Decode Error"))
  }
}

impl Encoder<GrappleTCPMessage> for GrappleTcpCodec {
  type Error = anyhow::Error;

  fn encode(&mut self, item: GrappleTCPMessage, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
    let mut writer = VecBitWriter::new();
    item.write(&mut writer, ());
    let bytes = writer.slice();
    
    dst.reserve(2 + bytes.len());
    dst.put(&(bytes.len() as u16).to_le_bytes()[..]);
    dst.put(&bytes[..]);
    Ok(())
  }
}