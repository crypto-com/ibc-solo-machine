//! Cosmos bit array implementation
#![allow(missing_docs)]
use std::convert::TryFrom;

use cosmos_sdk_proto::cosmos::crypto::multisig::v1beta1::CompactBitArray;

const MASK: u8 = 0b1000_0000;

pub trait BitArray {
    fn is_empty(&self) -> bool;

    fn len(&self) -> usize;

    fn get(&self, index: usize) -> bool;

    fn num_true_bits_before(&self, index: usize) -> usize;
}

impl BitArray for CompactBitArray {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        if self.extra_bits_stored == 0 {
            return self.elems.len() * 8;
        }

        ((self.elems.len() - 1) * 8) + usize::try_from(self.extra_bits_stored).unwrap()
    }

    fn get(&self, index: usize) -> bool {
        if index >= self.len() {
            return false;
        }

        (self.elems[index >> 3] & (MASK >> (index & 7))) > 0 // equivalent to `(self.elems[index / 8] & (MASK >> (index % 8)))`
    }

    fn num_true_bits_before(&self, index: usize) -> usize {
        let mut num_true_values = 0;

        for i in 0..index {
            if self.get(i) {
                num_true_values += 1;
            }
        }

        num_true_values
    }
}
