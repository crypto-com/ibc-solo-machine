use std::{cmp::Ordering, convert::TryFrom};

use anyhow::{anyhow, ensure, Context, Error};
use ibc_proto::ibc::core::client::v1::Height;
use tendermint::block::Height as BlockHeight;

pub trait IHeight: Sized {
    fn new(revision_number: u64, revision_height: u64) -> Self;

    fn zero() -> Self {
        Self::new(0, 0)
    }

    fn is_zero(&self) -> bool;

    fn checked_add(self, rhs: u64) -> Option<Self>;

    fn checked_sub(self, rhs: u64) -> Option<Self>;

    fn cmp(&self, other: &Self) -> Ordering;

    fn to_string(&self) -> String;

    fn to_block_height(&self) -> Result<BlockHeight, Error>;

    fn from_str(height: &str) -> Result<Self, Error>;
}

impl IHeight for Height {
    fn new(revision_number: u64, revision_height: u64) -> Self {
        Self {
            revision_number,
            revision_height,
        }
    }

    fn is_zero(&self) -> bool {
        self.revision_height == 0
    }

    fn checked_add(self, rhs: u64) -> Option<Self> {
        let revision_number = self.revision_number;
        let revision_height = self.revision_height.checked_add(rhs)?;

        Some(Self {
            revision_number,
            revision_height,
        })
    }

    fn checked_sub(self, rhs: u64) -> Option<Self> {
        let revision_number = self.revision_number;
        let revision_height = self.revision_height.checked_sub(rhs)?;

        Some(Self {
            revision_number,
            revision_height,
        })
    }

    fn cmp(&self, other: &Self) -> Ordering {
        match self.revision_number.cmp(&other.revision_number) {
            Ordering::Equal => self.revision_height.cmp(&other.revision_height),
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
        }
    }

    fn to_string(&self) -> String {
        format!("{}-{}", self.revision_number, self.revision_height)
    }

    fn to_block_height(&self) -> Result<BlockHeight, Error> {
        BlockHeight::try_from(self.revision_height)
            .map_err(|e| anyhow!("invalid block height: {}", e))
    }

    fn from_str(height: &str) -> Result<Self, Error> {
        let split: Vec<&str> = height.split('-').collect();

        ensure!(
            split.len() == 2,
            "height should be of format {{revision_number}}-{{revision_height}}"
        );

        Ok(Height {
            revision_number: split[0].parse().context("invalid revision number")?,
            revision_height: split[1].parse().context("invalid revision height")?,
        })
    }
}
