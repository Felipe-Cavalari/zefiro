//! Decodificação de imagens do WinRT (capa do SMTC, ícone de app) para `Image` BGRA pré-multiplicada.

use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapBounds, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode};
use windows::Storage::Streams::{DataReader, DataWriter, IRandomAccessStream, IRandomAccessStreamReference, InMemoryRandomAccessStream};
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

/// Bytes de um arquivo de imagem num stream em memória.
fn memory_stream(bytes: &[u8]) -> Result<InMemoryRandomAccessStream> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    writer.WriteBytes(bytes)?;
    writer.StoreAsync()?.get()?;
    writer.DetachStream()?;
    stream.Seek(0)?;
    Ok(stream)
}

/// Miniatura (lado maior `side`) de um arquivo de imagem em memória.
/// Chamada bloqueante: use fora da thread de UI.
pub fn decode_bytes(bytes: &[u8], side: u32) -> Option<Image> {
    let stream = memory_stream(bytes).ok()?;
    decode_stream(&stream.cast().ok()?, side, false).ok().flatten()
}

/// Qualquer imagem que o Windows abre (PNG, JPG, GIF, BMP, WebP) vira PNG, reduzida para o lado
/// maior caber em `max_side` (sem aumentar). Chamada bloqueante: use fora da thread de UI.
pub fn to_png(bytes: &[u8], max_side: u32) -> Result<Vec<u8>> {
    let input = memory_stream(bytes)?;
    let decoder = BitmapDecoder::CreateAsync(&input.cast::<IRandomAccessStream>()?)?.get()?;
    let (w, h) = (decoder.OrientedPixelWidth()?, decoder.OrientedPixelHeight()?);
    let scale = (max_side as f64 / w.max(h).max(1) as f64).min(1.0);
    let (sw, sh) = (((w as f64) * scale).round().max(1.0) as u32, ((h as f64) * scale).round().max(1.0) as u32);
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(sw)?;
    transform.SetScaledHeight(sh)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    let data = decoder
        .GetPixelDataTransformedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Straight, &transform, ExifOrientationMode::RespectExifOrientation, ColorManagementMode::DoNotColorManage)?
        .get()?;
    let pixels = data.DetachPixelData()?;
    let output = InMemoryRandomAccessStream::new()?;
    let encoder = BitmapEncoder::CreateAsync(BitmapEncoder::PngEncoderId()?, &output.cast::<IRandomAccessStream>()?)?.get()?;
    encoder.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Straight, sw, sh, 96.0, 96.0, &pixels)?;
    encoder.FlushAsync()?.get()?;
    let size = output.Size()? as u32;
    output.Seek(0)?;
    let reader = DataReader::CreateDataReader(&output.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.get()?;
    let mut png = vec![0u8; size as usize];
    reader.ReadBytes(&mut png)?;
    Ok(png)
}

/// Imagem de um arquivo (`.png`, `.ico`, `.jpg`, `.bmp`) em `side` px, ou `None` se não abrir.
/// Chamada bloqueante: use fora da thread de UI.
pub fn decode_file(path: &std::path::Path, side: u32) -> Option<zefiro_platform::image::Image> {
    let file = windows::Storage::StorageFile::GetFileFromPathAsync(&windows::core::HSTRING::from(path.as_os_str())).ok()?.get().ok()?;
    let stream = file.OpenReadAsync().ok()?.get().ok()?;
    decode_stream(&stream.cast().ok()?, side, false).ok().flatten()
}
