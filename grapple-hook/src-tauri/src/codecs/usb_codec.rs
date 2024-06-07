use bounded_static::IntoBoundedStatic;
use bytes::{BufMut, Buf};
use grapple_frc_msgs::{binmarshal::{BitView, BitWriter, Demarshal, Marshal, VecBitWriter}, bridge::BridgedCANMessage, MessageId};
use tokio_util::codec::{Decoder, Encoder};

pub struct GrappleUsbCodec;

impl Decoder for GrappleUsbCodec {
  type Item = BridgedCANMessage<'static>;
  type Error = anyhow::Error;

  fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    let data = src.to_vec();

    let mut bv = BitView::new(&data[..]);

    let result = BridgedCANMessage::read(&mut bv, ())
      .map(|x| x.into_static())
      .map_err(|e| anyhow::anyhow!(format!("{:?}", e)));

    match result {
      Ok(v) => {
        src.advance(bv.offset_for_drain());
        Ok(Some(v))
      },
      Err(_) => Ok(None),
    }
  }
}

impl Encoder<BridgedCANMessage<'_>> for GrappleUsbCodec {
  type Error = anyhow::Error;

  fn encode(&mut self, item: BridgedCANMessage<'_>, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
    let mut writer = VecBitWriter::new();
    item.write(&mut writer, ()).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))?;
    let bytes = writer.slice();
    
    dst.reserve(bytes.len());
    dst.put(&bytes[..]);
    Ok(())
  }
}