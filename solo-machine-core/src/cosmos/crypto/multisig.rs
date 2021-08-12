use std::convert::TryFrom;

use anyhow::{anyhow, ensure, Error, Result};
use cosmos_sdk_proto::cosmos::tx::signing::v1beta1::signature_descriptor::data::Multi as MultiSignatureData;
use serde::{Deserialize, Serialize};

use crate::{
    cosmos::bit_array::BitArray,
    proto::{cosmos::crypto::multisig::LegacyAminoPubKey, AnyConvert},
};

use super::PublicKey;

pub const MULTISIG_PUB_KEY_TYPE_URL: &str = "/cosmos.crypto.multisig.LegacyAminoPubKey";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigPublicKey {
    pub threshold: u32,
    pub public_keys: Vec<PublicKey>,
}

impl MultisigPublicKey {
    pub fn verify_multi_signature(
        &self,
        message: &[u8],
        signature_data: &MultiSignatureData,
    ) -> Result<()> {
        let threshold = usize::try_from(self.threshold).unwrap();

        let bit_array = signature_data
            .bitarray
            .as_ref()
            .ok_or_else(|| anyhow!("missing bit array from signature data"))?;
        let signatures = &signature_data.signatures;

        let size = bit_array.len();

        // ensure bit array is the correct size
        ensure!(
            self.public_keys.len() == size,
            "bit array size is incorrect {}",
            size
        );

        // ensure size of signature list
        ensure!(
            signatures.len() >= threshold && signatures.len() <= size,
            "signature size is incorrect {}",
            signatures.len()
        );

        // ensure at least k signatures are set
        ensure!(
            bit_array.num_true_bits_before(size) >= threshold,
            "minimum number of signatures not set, have {}, expected {}",
            bit_array.num_true_bits_before(size),
            threshold
        );

        let mut signature_index = 0;

        for i in 0..size {
            if bit_array.get(i) {
                let signature = &signatures[signature_index];

                let signature_data = signature
                    .sum
                    .as_ref()
                    .ok_or_else(|| anyhow!("missing signature data"))?;

                self.public_keys[i].verify_signature(message, signature_data)?;

                signature_index += 1;
            }
        }

        Ok(())
    }
}

impl TryFrom<&MultisigPublicKey> for LegacyAminoPubKey {
    type Error = Error;

    fn try_from(value: &MultisigPublicKey) -> Result<Self, Self::Error> {
        let threshold = value.threshold;
        let public_keys = value
            .public_keys
            .iter()
            .map(AnyConvert::to_any)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            threshold,
            public_keys,
        })
    }
}

impl TryFrom<&LegacyAminoPubKey> for MultisigPublicKey {
    type Error = Error;

    fn try_from(value: &LegacyAminoPubKey) -> Result<Self, Self::Error> {
        let threshold = value.threshold;
        let public_keys = value
            .public_keys
            .iter()
            .map(AnyConvert::from_any)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            threshold,
            public_keys,
        })
    }
}

impl_any_conversion!(LegacyAminoPubKey, MULTISIG_PUB_KEY_TYPE_URL);
