// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

/// The conversion from Nano to raw value
const TOKEN_TO_RAW_POWER_OF_10_CONVERSION: u32 = 9;

/// The conversion from Nano to raw value
const TOKEN_TO_RAW_CONVERSION: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
/// An amount in SNT Nanos. 10^9 Nanos = 1 SNT.
pub struct Nano(u64);

impl Nano {
    /// Type safe representation of zero Nano.
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Returns whether it's a representation of zero Nano.
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// New value from a number of nano tokens.
    pub const fn from_nano(value: u64) -> Self {
        Self(value)
    }

    /// Total Nano expressed in number of nano tokens.
    pub const fn as_nano(self) -> u64 {
        self.0
    }

    /// Computes `self + rhs`, returning `None` if overflow occurred.
    pub fn checked_add(self, rhs: Nano) -> Option<Nano> {
        self.0.checked_add(rhs.0).map(Self::from_nano)
    }

    /// Computes `self - rhs`, returning `None` if overflow occurred.
    pub fn checked_sub(self, rhs: Nano) -> Option<Nano> {
        self.0.checked_sub(rhs.0).map(Self::from_nano)
    }

    /// Converts the Nanos into bytes
    pub fn to_bytes(&self) -> [u8; 8] {
        self.0.to_ne_bytes()
    }
}

impl FromStr for Nano {
    type Err = Error;

    fn from_str(value_str: &str) -> Result<Self> {
        let mut itr = value_str.splitn(2, '.');
        let converted_units = {
            let units = itr
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| Error::FailedToParseNano("Can't parse token units".to_string()))?;

            units
                .checked_mul(TOKEN_TO_RAW_CONVERSION)
                .ok_or(Error::ExcessiveNanoValue)?
        };

        let remainder = {
            let remainder_str = itr.next().unwrap_or_default().trim_end_matches('0');

            if remainder_str.is_empty() {
                0
            } else {
                let parsed_remainder = remainder_str.parse::<u64>().map_err(|_| {
                    Error::FailedToParseNano("Can't parse token remainder".to_string())
                })?;

                let remainder_conversion = TOKEN_TO_RAW_POWER_OF_10_CONVERSION
                    .checked_sub(remainder_str.len() as u32)
                    .ok_or(Error::LossOfNanoPrecision)?;
                parsed_remainder * 10_u64.pow(remainder_conversion)
            }
        };

        Ok(Self::from_nano(converted_units + remainder))
    }
}

impl Display for Nano {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        let unit = self.0 / TOKEN_TO_RAW_CONVERSION;
        let remainder = self.0 % TOKEN_TO_RAW_CONVERSION;
        write!(formatter, "{unit}.{remainder:09}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::u64;

    #[test]
    fn from_str() -> Result<()> {
        assert_eq!(Nano(0), Nano::from_str("0")?);
        assert_eq!(Nano(0), Nano::from_str("0.")?);
        assert_eq!(Nano(0), Nano::from_str("0.0")?);
        assert_eq!(Nano(1), Nano::from_str("0.000000001")?);
        assert_eq!(Nano(1_000_000_000), Nano::from_str("1")?);
        assert_eq!(Nano(1_000_000_000), Nano::from_str("1.")?);
        assert_eq!(Nano(1_000_000_000), Nano::from_str("1.0")?);
        assert_eq!(Nano(1_000_000_001), Nano::from_str("1.000000001")?);
        assert_eq!(Nano(1_100_000_000), Nano::from_str("1.1")?);
        assert_eq!(Nano(1_100_000_001), Nano::from_str("1.100000001")?);
        assert_eq!(
            Nano(4_294_967_295_000_000_000),
            Nano::from_str("4294967295")?
        );
        assert_eq!(
            Nano(4_294_967_295_999_999_999),
            Nano::from_str("4294967295.999999999")?,
        );
        assert_eq!(
            Nano(4_294_967_295_999_999_999),
            Nano::from_str("4294967295.9999999990000")?,
        );

        assert_eq!(
            Err(Error::FailedToParseNano(
                "Can't parse token units".to_string()
            )),
            Nano::from_str("a")
        );
        assert_eq!(
            Err(Error::FailedToParseNano(
                "Can't parse token remainder".to_string()
            )),
            Nano::from_str("0.a")
        );
        assert_eq!(
            Err(Error::FailedToParseNano(
                "Can't parse token remainder".to_string()
            )),
            Nano::from_str("0.0.0")
        );
        assert_eq!(
            Err(Error::LossOfNanoPrecision),
            Nano::from_str("0.0000000009")
        );
        assert_eq!(
            Err(Error::ExcessiveNanoValue),
            Nano::from_str("18446744074")
        );
        Ok(())
    }

    #[test]
    fn display() {
        assert_eq!("0.000000000", format!("{}", Nano(0)));
        assert_eq!("0.000000001", format!("{}", Nano(1)));
        assert_eq!("0.000000010", format!("{}", Nano(10)));
        assert_eq!("1.000000000", format!("{}", Nano(1_000_000_000)));
        assert_eq!("1.000000001", format!("{}", Nano(1_000_000_001)));
        assert_eq!(
            "4294967295.000000000",
            format!("{}", Nano(4_294_967_295_000_000_000))
        );
    }

    #[test]
    fn checked_add_sub() {
        assert_eq!(Some(Nano(3)), Nano(1).checked_add(Nano(2)));
        assert_eq!(None, Nano(u64::MAX).checked_add(Nano(1)));
        assert_eq!(None, Nano(u64::MAX).checked_add(Nano(u64::MAX)));

        assert_eq!(Some(Nano(0)), Nano(u64::MAX).checked_sub(Nano(u64::MAX)));
        assert_eq!(None, Nano(0).checked_sub(Nano(u64::MAX)));
        assert_eq!(None, Nano(10).checked_sub(Nano(11)));
    }
}
