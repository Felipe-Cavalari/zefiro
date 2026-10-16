//! Decodificação de imagens do WinRT (capa do SMTC, ícone de app) para `Image` BGRA pré-multiplicada.

use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapBounds, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode};
use windows::Storage::Streams::{IRandomAccessStream, IRandomAccessStreamReference};
use windows::core::{Interface, Result};
use zefiro_platform::image::Image;

/// Decodifica e reduz para `side` px; com `square`, recorta o centro (capas de vídeo são 16:9).
/// Chamada bloqueante: use fora da thread de UI.
pub fn decode(reference: &IRandomAccessStreamReference, side: u32, square: bool) -> Result<Option<Image>> {
    let stream = reference.OpenReadAsync()?.get()?;
    decode_stream(&stream.cast()?, side, square)
}

pub fn decode_stream(stream: &IRandomAccessStream, side: u32, square: bool) -> Result<Option<Image>> {
    let decoder = BitmapDecoder::CreateAsync(stream)?.get()?;
    let (w, h) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
    if w == 0 || h == 0 {
        return Ok(None);
    }
    let fit = if square { w.min(h) } else { w.max(h) };
    let scale = side as f64 / fit as f64;
    let (sw, sh) = (((w as f64) * scale).round().max(1.0) as u32, ((h as f64) * scale).round().max(1.0) as u32);
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(sw)?;
    transform.SetScaledHeight(sh)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    let (ow, oh) = if square { (side.min(sw), side.min(sh)) } else { (sw, sh) };
    transform.SetBounds(BitmapBounds { X: (sw - ow) / 2, Y: (sh - oh) / 2, Width: ow, Height: oh })?;
    let data = decoder
        .GetPixelDataTransformedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Premultiplied, &transform, ExifOrientationMode::RespectExifOrientation, ColorManagementMode::DoNotColorManage)?
        .get()?;
    Ok(Image::new(ow, oh, data.DetachPixelData()?.to_vec()))
}
