// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    block_executor::config::BlockExecutorConfigFromOnchain, on_chain_config::OnChainConfig,
};
use anyhow::{format_err, Result};
use serde::{Deserialize, Serialize};

/// The on-chain execution config, in order to be able to add fields, we use enum to wrap the actual struct.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum OnChainExecutionConfig {
    V1(ExecutionConfigV1),
    V2(ExecutionConfigV2),
    V3(ExecutionConfigV3),
    /// To maintain backwards compatibility on replay, we must ensure that any new features resolve
    /// to previous behavior (before OnChainExecutionConfig was registered) in case of Missing.
    Missing,
    // Reminder: Add V4 and future versions here, after Missing (order matters for enums).
    V4(ExecutionConfigV4),
}

/// The public interface that exposes all values with safe fallback.
impl OnChainExecutionConfig {
    /// The type of the transaction shuffler being used.
    pub fn transaction_shuffler_type(&self) -> TransactionShufflerType {
        match &self {
            OnChainExecutionConfig::Missing => TransactionShufflerType::NoShuffling,
            OnChainExecutionConfig::V1(config) => config.transaction_shuffler_type.clone(),
            OnChainExecutionConfig::V2(config) => config.transaction_shuffler_type.clone(),
            OnChainExecutionConfig::V3(config) => config.transaction_shuffler_type.clone(),
            OnChainExecutionConfig::V4(config) => config.transaction_shuffler_type.clone(),
        }
    }

    /// The per-block gas limit being used.
    pub fn block_gas_limit_type(&self) -> BlockGasLimitType {
        match &self {
            OnChainExecutionConfig::Missing => BlockGasLimitType::NoLimit,
            OnChainExecutionConfig::V1(_config) => BlockGasLimitType::NoLimit,
            OnChainExecutionConfig::V2(config) => config
                .block_gas_limit
                .map_or(BlockGasLimitType::NoLimit, BlockGasLimitType::Limit),
            OnChainExecutionConfig::V3(config) => config
                .block_gas_limit
                .map_or(BlockGasLimitType::NoLimit, BlockGasLimitType::Limit),
            OnChainExecutionConfig::V4(config) => config.block_gas_limit_type.clone(),
        }
    }

    pub fn block_executor_onchain_config(&self) -> BlockExecutorConfigFromOnchain {
        BlockExecutorConfigFromOnchain {
            block_gas_limit_type: self.block_gas_limit_type(),
        }
    }

    /// The type of the transaction deduper being used.
    pub fn transaction_deduper_type(&self) -> TransactionDeduperType {
        match &self {
            // Note, this behavior was enabled before OnChainExecutionConfig was registered.
            OnChainExecutionConfig::Missing => TransactionDeduperType::TxnHashAndAuthenticatorV1,
            OnChainExecutionConfig::V1(_config) => TransactionDeduperType::NoDedup,
            OnChainExecutionConfig::V2(_config) => TransactionDeduperType::NoDedup,
            OnChainExecutionConfig::V3(config) => config.transaction_deduper_type.clone(),
            OnChainExecutionConfig::V4(config) => config.transaction_deduper_type.clone(),
        }
    }

    /// The default values to use for new networks, e.g., devnet, forge.
    /// Features that are ready for deployment can be enabled here.
    pub fn default_for_genesis() -> Self {
        OnChainExecutionConfig::V4(ExecutionConfigV4 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
            block_gas_limit_type: BlockGasLimitType::ComplexLimitV1 {
                effective_block_gas_limit: 35000,
                execution_gas_effective_multiplier: 1,
                io_gas_effective_multiplier: 1,
                block_output_limit: None,
                conflict_penalty_window: 1,
                add_block_limit_outcome_onchain: false,
                use_granular_resource_group_conflicts: false,
            },
            transaction_deduper_type: TransactionDeduperType::TxnHashAndAuthenticatorV1,
        })
    }

    /// The default values to use when on-chain config is not initialized.
    /// This value should not be changed, for replay purposes.
    pub fn default_if_missing() -> Self {
        OnChainExecutionConfig::Missing
    }
}

impl OnChainConfig for OnChainExecutionConfig {
    const MODULE_IDENTIFIER: &'static str = "execution_config";
    const TYPE_IDENTIFIER: &'static str = "ExecutionConfig";

    /// The Move resource is
    /// ```ignore
    /// struct AptosExecutionConfig has copy, drop, store {
    ///    config: vector<u8>,
    /// }
    /// ```
    /// so we need two rounds of bcs deserilization to turn it back to OnChainExecutionConfig
    fn deserialize_into_config(bytes: &[u8]) -> Result<Self> {
        let raw_bytes: Vec<u8> = bcs::from_bytes(bytes)?;
        bcs::from_bytes(&raw_bytes)
            .map_err(|e| format_err!("[on-chain config] Failed to deserialize into config: {}", e))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExecutionConfigV1 {
    pub transaction_shuffler_type: TransactionShufflerType,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExecutionConfigV2 {
    pub transaction_shuffler_type: TransactionShufflerType,
    pub block_gas_limit: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExecutionConfigV3 {
    pub transaction_shuffler_type: TransactionShufflerType,
    pub block_gas_limit: Option<u64>,
    pub transaction_deduper_type: TransactionDeduperType,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExecutionConfigV4 {
    pub transaction_shuffler_type: TransactionShufflerType,
    pub block_gas_limit_type: BlockGasLimitType,
    pub transaction_deduper_type: TransactionDeduperType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")] // cannot use tag = "type" as nested enums cannot work, and bcs doesn't support it
pub enum TransactionShufflerType {
    NoShuffling,
    DeprecatedSenderAwareV1(u32),
    SenderAwareV2(u32),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")] // cannot use tag = "type" as nested enums cannot work, and bcs doesn't support it
pub enum TransactionDeduperType {
    NoDedup,
    TxnHashAndAuthenticatorV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BlockGasLimitType {
    NoLimit,
    Limit(u64),
    /// Provides two separate block limits:
    /// 1. effective_block_gas_limit
    /// 2. block_output_limit
    ComplexLimitV1 {
        /// Formula for effective block gas limit:
        /// effective_block_gas_limit <
        /// (execution_gas_effective_multiplier * execution_gas_used +
        ///  io_gas_effective_multiplier * io_gas_used
        /// ) * (1 + num conflicts in conflict_penalty_window)
        effective_block_gas_limit: u64,
        execution_gas_effective_multiplier: u64,
        io_gas_effective_multiplier: u64,
        conflict_penalty_window: u32,
        /// Block limit on the total (approximate) txn output size in bytes.
        block_output_limit: Option<u64>,

        add_block_limit_outcome_onchain: bool,

        // If true we look at granular resource group conflicts (i.e. if same Tag
        // within a resource group has a conflict)
        // If false, we treat any conclicts inside of resource groups (even across
        // non-overlapping tags) as conflicts).
        use_granular_resource_group_conflicts: bool,
    },
}

impl BlockGasLimitType {
    pub fn block_gas_limit(&self) -> Option<u64> {
        match self {
            BlockGasLimitType::NoLimit => None,
            BlockGasLimitType::Limit(limit) => Some(*limit),
            BlockGasLimitType::ComplexLimitV1 {
                effective_block_gas_limit,
                ..
            } => Some(*effective_block_gas_limit),
        }
    }

    pub fn execution_gas_effective_multiplier(&self) -> u64 {
        match self {
            BlockGasLimitType::NoLimit => 1,
            BlockGasLimitType::Limit(_) => 1,
            BlockGasLimitType::ComplexLimitV1 {
                execution_gas_effective_multiplier,
                ..
            } => *execution_gas_effective_multiplier,
        }
    }

    pub fn io_gas_effective_multiplier(&self) -> u64 {
        match self {
            BlockGasLimitType::NoLimit => 1,
            BlockGasLimitType::Limit(_) => 1,
            BlockGasLimitType::ComplexLimitV1 {
                io_gas_effective_multiplier,
                ..
            } => *io_gas_effective_multiplier,
        }
    }

    pub fn block_output_limit(&self) -> Option<u64> {
        match self {
            BlockGasLimitType::NoLimit => None,
            BlockGasLimitType::Limit(_) => None,
            BlockGasLimitType::ComplexLimitV1 {
                block_output_limit, ..
            } => *block_output_limit,
        }
    }

    pub fn conflict_penalty_window(&self) -> Option<u32> {
        match self {
            BlockGasLimitType::NoLimit => None,
            BlockGasLimitType::Limit(_) => None,
            BlockGasLimitType::ComplexLimitV1 {
                conflict_penalty_window,
                ..
            } => {
                if *conflict_penalty_window > 1 {
                    Some(*conflict_penalty_window)
                } else {
                    None
                }
            },
        }
    }

    pub fn add_block_limit_outcome_onchain(&self) -> bool {
        match self {
            BlockGasLimitType::NoLimit => false,
            BlockGasLimitType::Limit(_) => false,
            BlockGasLimitType::ComplexLimitV1 {
                add_block_limit_outcome_onchain,
                ..
            } => *add_block_limit_outcome_onchain,
        }
    }

    pub fn use_granular_resource_group_conflicts(&self) -> bool {
        match self {
            BlockGasLimitType::NoLimit => false,
            BlockGasLimitType::Limit(_) => false,
            BlockGasLimitType::ComplexLimitV1 {
                use_granular_resource_group_conflicts,
                ..
            } => *use_granular_resource_group_conflicts,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::on_chain_config::{InMemoryOnChainConfig, OnChainConfigPayload};
    use rand::Rng;
    use std::collections::HashMap;

    #[test]
    fn test_config_yaml_serialization() {
        let config = OnChainExecutionConfig::default_for_genesis();
        let s = serde_yaml::to_string(&config).unwrap();

        serde_yaml::from_str::<OnChainExecutionConfig>(&s).unwrap();
    }

    #[test]
    fn test_config_bcs_serialization() {
        let config = OnChainExecutionConfig::default_for_genesis();
        let s = bcs::to_bytes(&config).unwrap();

        bcs::from_bytes::<OnChainExecutionConfig>(&s).unwrap();
    }

    #[test]
    fn test_config_serialization() {
        let config = OnChainExecutionConfig::V1(ExecutionConfigV1 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
        });

        let s = serde_yaml::to_string(&config).unwrap();
        let result = serde_yaml::from_str::<OnChainExecutionConfig>(&s).unwrap();
        assert!(matches!(
            result.transaction_shuffler_type(),
            TransactionShufflerType::SenderAwareV2(32)
        ));

        // V2 test with random per-block gas limit
        let rand_gas_limit = rand::thread_rng().gen_range(0, 1000000) as u64;
        let config = OnChainExecutionConfig::V2(ExecutionConfigV2 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
            block_gas_limit: Some(rand_gas_limit),
        });

        let s = serde_yaml::to_string(&config).unwrap();
        let result = serde_yaml::from_str::<OnChainExecutionConfig>(&s).unwrap();
        assert!(matches!(
            result.transaction_shuffler_type(),
            TransactionShufflerType::SenderAwareV2(32)
        ));
        assert_eq!(
            result.block_gas_limit_type(),
            BlockGasLimitType::Limit(rand_gas_limit)
        );

        // V2 test with no per-block gas limit
        let config = OnChainExecutionConfig::V2(ExecutionConfigV2 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
            block_gas_limit: None,
        });

        let s = serde_yaml::to_string(&config).unwrap();
        let result = serde_yaml::from_str::<OnChainExecutionConfig>(&s).unwrap();
        assert!(matches!(
            result.transaction_shuffler_type(),
            TransactionShufflerType::SenderAwareV2(32)
        ));
        assert_eq!(result.block_gas_limit_type(), BlockGasLimitType::NoLimit);
    }

    #[test]
    fn test_config_onchain_payload() {
        let execution_config = OnChainExecutionConfig::V1(ExecutionConfigV1 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
        });

        let mut configs = HashMap::new();
        configs.insert(
            OnChainExecutionConfig::CONFIG_ID,
            // Requires double serialization, check deserialize_into_config for more details
            bcs::to_bytes(&bcs::to_bytes(&execution_config).unwrap()).unwrap(),
        );

        let payload = OnChainConfigPayload::new(1, InMemoryOnChainConfig::new(configs));

        let result: OnChainExecutionConfig = payload.get().unwrap();
        assert!(matches!(
            result.transaction_shuffler_type(),
            TransactionShufflerType::SenderAwareV2(32)
        ));

        // V2 test with random per-block gas limit
        let rand_gas_limit = rand::thread_rng().gen_range(0, 1000000) as u64;
        let execution_config = OnChainExecutionConfig::V2(ExecutionConfigV2 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
            block_gas_limit: Some(rand_gas_limit),
        });

        let mut configs = HashMap::new();
        configs.insert(
            OnChainExecutionConfig::CONFIG_ID,
            // Requires double serialization, check deserialize_into_config for more details
            bcs::to_bytes(&bcs::to_bytes(&execution_config).unwrap()).unwrap(),
        );

        let payload = OnChainConfigPayload::new(1, InMemoryOnChainConfig::new(configs));

        let result: OnChainExecutionConfig = payload.get().unwrap();
        assert!(matches!(
            result.transaction_shuffler_type(),
            TransactionShufflerType::SenderAwareV2(32)
        ));
        assert_eq!(
            result.block_gas_limit_type(),
            BlockGasLimitType::Limit(rand_gas_limit)
        );

        // V2 test with no per-block gas limit
        let execution_config = OnChainExecutionConfig::V2(ExecutionConfigV2 {
            transaction_shuffler_type: TransactionShufflerType::SenderAwareV2(32),
            block_gas_limit: None,
        });

        let mut configs = HashMap::new();
        configs.insert(
            OnChainExecutionConfig::CONFIG_ID,
            // Requires double serialization, check deserialize_into_config for more details
            bcs::to_bytes(&bcs::to_bytes(&execution_config).unwrap()).unwrap(),
        );

        let payload = OnChainConfigPayload::new(1, InMemoryOnChainConfig::new(configs));

        let result: OnChainExecutionConfig = payload.get().unwrap();
        assert!(matches!(
            result.transaction_shuffler_type(),
            TransactionShufflerType::SenderAwareV2(32)
        ));
        assert_eq!(result.block_gas_limit_type(), BlockGasLimitType::NoLimit);
    }
}
