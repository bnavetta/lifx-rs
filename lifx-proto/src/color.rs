use std::{convert::TryFrom, f32, u16};

use bytes::{Buf, BufMut};
use thiserror::Error;

use crate::ProtocolError;

/// Color and color temperature, represented in HSB and Kelvin
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hsbk {
    pub hue: u16,
    pub saturation: u16,
    pub brightness: u16,
    pub temperature: Kelvin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kelvin(u16);

impl Hsbk {
    /// Size of a Hsbk value on the wire, in bytes
    pub const SIZE: usize = 8;

    pub fn encode<B: BufMut>(self, buf: &mut B) {
        buf.put_u16_le(self.hue);
        buf.put_u16_le(self.saturation);
        buf.put_u16_le(self.brightness);
        buf.put_u16_le(self.temperature.0);
    }

    pub fn decode<B: Buf>(buf: &mut B) -> Result<Hsbk, ProtocolError> {
        let hue = buf.get_u16_le();
        let saturation = buf.get_u16_le();
        let brightness = buf.get_u16_le();
        let temperature = Kelvin::try_from(buf.get_u16_le()).map_err(|err| ProtocolError::InvalidPayload(err.to_string()))?;
        Ok(Hsbk { 
            hue,
            saturation,
            brightness,
            temperature
        })
    }
}

impl Kelvin {
    pub fn new(value: u16) -> Kelvin {
        Kelvin::try_from(value).expect("Temperature out of bounds")
    }
}

impl TryFrom<u16> for Kelvin {
    type Error = KelvinError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            2500..=9000 => Ok(Kelvin(value)),
            _ => Err(KelvinError(value))
        }
    }
}

#[cfg(feature = "palette")]
impl Hsbk {
    pub fn new(color: palette::Hsv, temperature: Kelvin) -> Hsbk {
        use palette::Component;

        // Palette's Hsv type requires that components be floats, so we have to decompose it and convert to u16 ourselves
        let (hue, saturation, value) = color.into_components();
        // Scale the 0-360 value to an 0-2^16 value
        let hue_degrees = (u16::MAX as f32) * (hue.to_positive_degrees() / 360f32);
        let hue_int = hue_degrees.round() as u16;
        let saturation_int = saturation.convert::<u16>();
        let value_int = value.convert::<u16>();
        Hsbk {
            hue: hue_int,
            saturation: saturation_int,
            brightness: value_int,
            temperature
        }
    }

    pub fn color(&self) ->  palette::Hsv {
        use palette::{Component, Hsv, RgbHue};
        // Scale the 0-2^16 value to an 0-360 value
        let hue = RgbHue::from_degrees((self.hue as f32) / (std::u16::MAX as f32) * 360f32);
        let saturation = self.saturation.convert::<f32>();
        let value = self.brightness.convert::<f32>();
        Hsv::new(hue, saturation, value)
    }
}

#[derive(Error, Debug)]
#[error("invalid Kelvin value: {0}")]
pub struct KelvinError(u16);

impl Into<u16> for Kelvin {
    fn into(self) -> u16 {
        self.0
    }
}