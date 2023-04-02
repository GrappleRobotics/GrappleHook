use bytes::{BytesMut, Buf, BufMut};
use grapple_frc_msgs::{grapple::usb::GrappleUSBMessage, binmarshal::{rw::{VecBitWriter, BitView, BitWriter}, BinMarshal}};
use tokio_util::codec::{Decoder, Encoder};

pub struct GrappleUsbCodec {}

impl Decoder for GrappleUsbCodec {
  type Item = GrappleUSBMessage;
  type Error = anyhow::Error;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
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

    GrappleUSBMessage::read(&mut BitView::new(&data[..]), ()).map(|v| Some(v)).ok_or(anyhow::anyhow!("Decode Error"))
  }
}

impl Encoder<GrappleUSBMessage> for GrappleUsbCodec {
  type Error = anyhow::Error;

  fn encode(&mut self, item: GrappleUSBMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
    let mut writer = VecBitWriter::new();
    item.write(&mut writer, ());
    let bytes = writer.slice();
    
    dst.reserve(2 + bytes.len());
    dst.put(&(bytes.len() as u16).to_le_bytes()[..]);
    dst.put(&bytes[..]);
    Ok(())
  }
}
