use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use image::codecs::png::PngEncoder;
use image::load_from_memory;
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use std::{f32::consts::PI, io::Cursor};

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

#[derive(Debug)]
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
        "nebula" => generate_gradient(|x, y, width, height| {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let swirl = ((fx * 7.0).sin() + (fy * 5.5).cos()) * 0.5 + 0.5;
            let band = ((fx - fy) * 8.0).sin() * 0.5 + 0.5;
            Rgba([
                clamp_to_byte(140.0 + 80.0 * swirl),
                clamp_to_byte(60.0 + 45.0 * band),
                clamp_to_byte(200.0 + 55.0 * swirl),
                255,
            ])
        }),
        "solstice" => generate_gradient(|x, y, width, height| {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let sunset_intensity = ((1.0 - fy).powf(1.4) * 255.0).min(255.0) / 255.0;
            let sky_wave = (fx * PI).cos().abs();
            Rgba([
                clamp_to_byte(180.0 + 65.0 * sunset_intensity + 25.0 * sky_wave),
                clamp_to_byte(120.0 + 55.0 * sunset_intensity + 30.0 * sky_wave),
                clamp_to_byte(110.0 + 100.0 * fy.powf(1.2)),
                255,
            ])
        }),
        "midnight" => generate_gradient(|x, y, width, height| {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let mut r = clamp_to_byte(18.0 + 60.0 * (1.0 - fy).powf(1.4));
            let mut g = clamp_to_byte(24.0 + 80.0 * (1.0 - fy).powf(1.6));
            let mut b = clamp_to_byte(80.0 + 140.0 * (1.0 - fy));

            if fy > 0.62 {
                let depth = ((fy - 0.62) / 0.38).min(1.0);
                let shade = 1.0 - depth * 0.7;
                r = clamp_to_byte(f32::from(r) * shade);
                g = clamp_to_byte(f32::from(g) * shade);
                b = clamp_to_byte(f32::from(b) * shade);

                let skyline_tint = (fx * PI * 4.0).sin().abs();
                r = clamp_to_byte(f32::from(r) + 40.0 * skyline_tint * (1.0 - depth));
                g = clamp_to_byte(f32::from(g) + 25.0 * skyline_tint * (1.0 - depth));

                if pseudo_random(x, y) > 0.96 {
                    r = clamp_to_byte(210.0 + 40.0 * pseudo_random(y, x));
                    g = clamp_to_byte(140.0 + 80.0 * pseudo_random(x + 11, y + 7));
                    b = clamp_to_byte(70.0 + 50.0 * pseudo_random(x + 19, y + 3));
                }
            } else if pseudo_random(x, y) > 0.995 {
                let sparkle = 200.0 + 55.0 * pseudo_random(y + 13, x + 17);
                r = clamp_to_byte(sparkle);
                g = clamp_to_byte(sparkle);
                b = clamp_to_byte(240.0);
            }

            Rgba([r, g, b, 255])
        }),
        "cinder" => generate_gradient(|x, y, _width, height| {
            let fy = y as f32 / height as f32;
            let mut r = clamp_to_byte(40.0 + 70.0 * (1.0 - fy).powf(1.5));
            let mut g = clamp_to_byte(28.0 + 35.0 * (1.0 - fy));
            let mut b = clamp_to_byte(22.0 + 40.0 * (1.0 - fy));

            if pseudo_random(x, y) > 0.985 {
                let ember = 210.0 + 40.0 * pseudo_random(x + 23, y + 9);
                r = clamp_to_byte(ember);
                g = clamp_to_byte(90.0 + 60.0 * pseudo_random(x + 5, y + 29));
                b = clamp_to_byte(60.0 + 40.0 * pseudo_random(y + 17, x + 3));
            }

            Rgba([r, g, b, 255])
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

fn pseudo_random(x: u32, y: u32) -> f32 {
    let mut value = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    value = (value ^ (value >> 13)).wrapping_mul(1_274_126_177);
    let masked = value ^ (value >> 16);
    ((masked & 0x00FF_FFFF) as f32) / 0x00FF_FFFF as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD_TEST;
    use base64::Engine;
    use image::{ImageBuffer, ImageEncoder, Rgba};

    #[test]
    fn parse_data_uri_decodes_payload() {
        let original = b"hello";
        let uri = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD_TEST.encode(original)
        );
        let (mime, data) = parse_data_uri(&uri).expect("data uri should decode");
        assert_eq!(mime, "image/png");
        assert_eq!(data, original);
    }

    #[test]
    fn parse_data_uri_rejects_invalid_inputs() {
        assert!(matches!(
            parse_data_uri("not-a-data-uri"),
            Err(StegoError::InvalidDataUri)
        ));
        assert!(matches!(
            parse_data_uri("data:text/plain,hello"),
            Err(StegoError::InvalidDataUri)
        ));
    }

    #[test]
    fn embed_payload_builtin_carrier_produces_png() {
        let result = embed_payload(
            StegoCarrierSource::BuiltIn("aurora".to_string()),
            b"secret payload",
        )
        .expect("embedding into builtin carrier should succeed");

        assert_eq!(result.mime, "image/png");
        assert!(!result.image_data.is_empty());
    }

    #[test]
    fn embed_payload_rejects_large_payload_for_small_carrier() {
        let mut buffer = Vec::new();
        {
            let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
            encoder
                .write_image(&[255, 0, 0, 255], 1, 1, image::ColorType::Rgba8)
                .expect("encode 1x1 image");
        }

        let source = StegoCarrierSource::Uploaded {
            mime: "image/png".to_string(),
            data: buffer,
        };

        let err = embed_payload(source, &[0u8; 16]).expect_err("payload should be too large");
        assert!(matches!(err, StegoError::PayloadTooLarge { .. }));
    }

    #[test]
    fn embed_message_writes_bits_until_payload_complete() {
        let baseline = ImageBuffer::from_pixel(16, 16, Rgba([0, 0, 0, 255]));
        let mut image = baseline.clone();
        embed_message(b"a", &mut image).expect("embedding small payload succeeds");

        assert_ne!(image, baseline, "embedding should modify carrier pixels");
    }
}
