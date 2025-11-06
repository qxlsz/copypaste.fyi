use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use image::codecs::png::PngEncoder;
use image::load_from_memory;
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum StegoError {
    #[error("failed to parse data uri")]
    InvalidDataUri,
    #[error("unsupported carrier image format")]
    UnsupportedFormat,
    #[error("failed to decode carrier image: {0}")]
    DecodeCarrier(String),
    #[error(
        "payload too large for carrier (capacity {capacity} bytes, required {required} bytes)"
    )]
    PayloadTooLarge { required: usize, capacity: usize },
    #[error("failed to encode stego image: {0}")]
    EncodeFailure(String),
}

pub enum StegoCarrierSource {
    BuiltIn(String),
    Uploaded { mime: String, data: Vec<u8> },
}

pub struct StegoEmbedResult {
    pub mime: String,
    pub image_data: Vec<u8>,
}

pub fn embed_payload(
    source: StegoCarrierSource,
    payload: &[u8],
) -> Result<StegoEmbedResult, StegoError> {
    let (mut image, mime) = match source {
        StegoCarrierSource::BuiltIn(identifier) => generate_builtin(identifier.as_str()),
        StegoCarrierSource::Uploaded { mime, data } => {
            let dynamic = load_from_memory(&data)
                .map_err(|error| StegoError::DecodeCarrier(error.to_string()))?;
            (dynamic.to_rgba8(), mime)
        }
    };

    embed_message(payload, &mut image)?;
    let mut buffer = Vec::new();
    {
        let encoder = PngEncoder::new(Cursor::new(&mut buffer));
        if let Err(error) = encoder.write_image(
            &image,
            image.width(),
            image.height(),
            image::ColorType::Rgba8,
        ) {
            return Err(StegoError::EncodeFailure(error.to_string()));
        }
    }

    Ok(StegoEmbedResult {
        mime,
        image_data: buffer,
    })
}

pub fn parse_data_uri(input: &str) -> Result<(String, Vec<u8>), StegoError> {
    let Some(rest) = input.strip_prefix("data:") else {
        return Err(StegoError::InvalidDataUri);
    };
    let (meta, data_part) = rest.split_once(",").ok_or(StegoError::InvalidDataUri)?;
    if !meta.ends_with(";base64") {
        return Err(StegoError::InvalidDataUri);
    }
    let mime = meta
        .strip_suffix(";base64")
        .ok_or(StegoError::InvalidDataUri)?
        .to_string();
    let data = BASE64_STANDARD
        .decode(data_part)
        .map_err(|_| StegoError::InvalidDataUri)?;
    Ok((mime, data))
}

fn embed_message(payload: &[u8], image: &mut RgbaImage) -> Result<(), StegoError> {
    let length_bytes = (payload.len() as u32).to_be_bytes();
    let mut bits = Vec::with_capacity((payload.len() + length_bytes.len()) * 8);
    for byte in length_bytes.iter().chain(payload.iter()) {
        for shift in (0..8).rev() {
            bits.push((byte >> shift) & 1);
        }
    }

    let capacity_bits = (image.width() as usize) * (image.height() as usize) * 3;
    if bits.len() > capacity_bits {
        return Err(StegoError::PayloadTooLarge {
            required: payload.len(),
            capacity: capacity_bits / 8,
        });
    }

    let mut bit_index = 0;
    let total_bits = bits.len();

    for pixel in image.pixels_mut() {
        for channel in pixel.0.iter_mut().take(3) {
            if bit_index >= total_bits {
                return Ok(());
            }
            let bit = bits[bit_index];
            *channel = (*channel & 0xFE) | bit;
            bit_index += 1;
        }
    }

    Ok(())
}

fn clamp_to_byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn generate_builtin(identifier: &str) -> (RgbaImage, String) {
    match identifier {
        "aurora" => generate_gradient(|x, y, width, height| {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            Rgba([
                clamp_to_byte(200.0_f32 * fx),
                clamp_to_byte(120.0_f32 + 100.0_f32 * fy),
                clamp_to_byte(180.0_f32 + 60.0_f32 * (1.0 - fx)),
                255,
            ])
        }),
        "horizon" => generate_gradient(|x, y, width, height| {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            Rgba([
                clamp_to_byte(50.0_f32 + 150.0_f32 * fx),
                clamp_to_byte(80.0_f32 + 100.0_f32 * (1.0 - fy)),
                clamp_to_byte(200.0_f32 + 40.0_f32 * fy),
                255,
            ])
        }),
        _ => generate_gradient(|x, y, width, height| {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            Rgba([
                clamp_to_byte(30.0_f32 + 220.0_f32 * fx.sin().abs()),
                clamp_to_byte(180.0_f32 + 70.0_f32 * fy.cos().abs()),
                clamp_to_byte(90.0_f32 + 130.0_f32 * (fx * fy).sin().abs()),
                255,
            ])
        }),
    }
}

fn generate_gradient<F>(mut f: F) -> (RgbaImage, String)
where
    F: FnMut(u32, u32, u32, u32) -> Rgba<u8>,
{
    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 360;
    let mut buffer: RgbaImage = ImageBuffer::from_fn(WIDTH, HEIGHT, |x, y| f(x, y, WIDTH, HEIGHT));

    // Apply a mild blur effect to break harsh edges and add noise for better diffusion.
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let pixel = buffer.get_pixel_mut(x, y);
            pixel.0[0] = pixel.0[0].saturating_add(((x * y + 13) % 7) as u8);
            pixel.0[1] = pixel.0[1].saturating_sub(((x + y + 11) % 5) as u8);
        }
    }

    (buffer, "image/png".to_string())
}
