// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use chrono::Utc;
use starcoin_types::block::BlockNumber;
use tracing::{debug, info};

/// Configuration for index monitoring logic
#[derive(Debug, Clone)]
pub struct IndexMonitorConfig {
    pub max_block_difference: u64,
    pub max_notify_time_interval: u64,
}

impl Default for IndexMonitorConfig {
    fn default() -> Self {
        Self {
            max_block_difference: 1000,
            max_notify_time_interval: 600,
        }
    }
}

/// State for tracking notification timing
#[derive(Debug, Clone)]
pub struct NotificationState {
    pub latest_notify_time: u64,
}

impl Default for NotificationState {
    fn default() -> Self {
        Self {
            latest_notify_time: 0,
        }
    }
}

/// Result of index monitoring check
#[derive(Debug, Clone, PartialEq)]
pub enum IndexMonitorResult {
    /// No action needed
    NoAction,
    /// Should wait (current block is behind cached block)
    ShouldWait,
    /// Should notify about index exception
    ShouldNotify {
        current_block: BlockNumber,
        cached_block: BlockNumber,
        difference: u64,
    },
}

/// Check if current block number is behind cached block number
pub fn is_current_block_behind(
    current_block: BlockNumber,
    cached_block: BlockNumber,
) -> bool {
    current_block < cached_block
}

/// Check if there's a significant block difference that requires notification
pub fn has_significant_block_difference(
    current_block: BlockNumber,
    cached_block: BlockNumber,
    max_difference: u64,
) -> bool {
    if current_block <= cached_block {
        return false; // No difference or current is behind
    }
    let difference = current_block - cached_block;
    difference > max_difference
}

/// Check if enough time has passed since the last notification
pub fn should_notify_based_on_time(
    latest_notify_time: u64,
    max_interval: u64,
) -> bool {
    if latest_notify_time == 0 {
        return true; // First notification
    }
    
    let current_time = Utc::now().timestamp() as u64;
    current_time - latest_notify_time > max_interval
}

/// Main function to determine what action should be taken based on current state
pub fn check_index_monitor_state(
    current_block: BlockNumber,
    cached_block: BlockNumber,
    notification_state: &NotificationState,
    config: &IndexMonitorConfig,
) -> IndexMonitorResult {
    debug!(
        "Checking index monitor state: current={}, cached={}, last_notify={}",
        current_block, cached_block, notification_state.latest_notify_time
    );

    // Check if current block is behind cached block
    if is_current_block_behind(current_block, cached_block) {
        info!(
            "Current block {} is behind cached block {}, should wait",
            current_block, cached_block
        );
        return IndexMonitorResult::ShouldWait;
    }

    // Check if there's a significant difference
    if has_significant_block_difference(current_block, cached_block, config.max_block_difference) {
        // Check if enough time has passed for notification
        if should_notify_based_on_time(
            notification_state.latest_notify_time,
            config.max_notify_time_interval,
        ) {
            let difference = current_block - cached_block;
            info!(
                "Index exception detected: current={}, cached={}, difference={}",
                current_block, cached_block, difference
            );
            return IndexMonitorResult::ShouldNotify {
                current_block,
                cached_block,
                difference,
            };
        } else {
            debug!("Significant difference detected but not enough time has passed for notification");
        }
    }

    IndexMonitorResult::NoAction
}

/// Update notification state after sending a notification
pub fn update_notification_state(state: &mut NotificationState) {
    state.latest_notify_time = Utc::now().timestamp() as u64;
    debug!("Updated notification time to: {}", state.latest_notify_time);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock function to simulate Elasticsearch connection
    /// This allows us to test different scenarios without requiring a real ES instance
    async fn mock_get_cached_index_block_number(scenario: &str) -> u64 {
        match scenario {
            "normal" => 1000,           // Normal case
            "behind" => 1200,           // Cached ahead of current
            "large_gap" => 500,         // Large gap between current and cached
            "small_gap" => 1050,        // Small gap
            "error" => 0,               // Simulate connection error
            "zero" => 0,                // Edge case: zero
            _ => 1000,                  // Default case
        }
    }

    #[tokio::test]
    async fn test_mock_es_scenarios() {
        // Test normal scenario
        let block_number = mock_get_cached_index_block_number("normal").await;
        assert_eq!(block_number, 1000);
        
        // Test behind scenario
        let block_number = mock_get_cached_index_block_number("behind").await;
        assert_eq!(block_number, 1200);
        
        // Test large gap scenario
        let block_number = mock_get_cached_index_block_number("large_gap").await;
        assert_eq!(block_number, 500);
        
        // Test error scenario
        let block_number = mock_get_cached_index_block_number("error").await;
        assert_eq!(block_number, 0);
    }

    #[test]
    fn test_is_current_block_behind() {
        assert!(is_current_block_behind(100, 200));
        assert!(!is_current_block_behind(200, 100));
        assert!(!is_current_block_behind(100, 100));
    }

    #[test]
    fn test_has_significant_block_difference() {
        let config = IndexMonitorConfig::default();
        
        // Difference is greater than max
        assert!(has_significant_block_difference(1200, 100, config.max_block_difference));
        
        // Difference is equal to max (should not trigger)
        assert!(!has_significant_block_difference(1100, 100, config.max_block_difference));
        
        // Difference is less than max
        assert!(!has_significant_block_difference(1050, 100, config.max_block_difference));
    }

    #[test]
    fn test_should_notify_based_on_time() {
        let max_interval = 600;
        
        // First notification (latest_notify_time == 0)
        assert!(should_notify_based_on_time(0, max_interval));
        
        // Not enough time has passed
        let recent_time = Utc::now().timestamp() as u64 - 100;
        assert!(!should_notify_based_on_time(recent_time, max_interval));
        
        // Enough time has passed
        let old_time = Utc::now().timestamp() as u64 - 700;
        assert!(should_notify_based_on_time(old_time, max_interval));
    }

    #[test]
    fn test_check_index_monitor_state_should_wait() {
        let config = IndexMonitorConfig::default();
        let state = NotificationState::default();
        
        let result = check_index_monitor_state(100, 200, &state, &config);
        assert!(matches!(result, IndexMonitorResult::ShouldWait));
    }

    #[test]
    fn test_check_index_monitor_state_should_notify() {
        let config = IndexMonitorConfig::default();
        let state = NotificationState::default(); // latest_notify_time = 0
        
        let result = check_index_monitor_state(1200, 100, &state, &config);
        match result {
            IndexMonitorResult::ShouldNotify { current_block, cached_block, difference } => {
                assert_eq!(current_block, 1200);
                assert_eq!(cached_block, 100);
                assert_eq!(difference, 1100);
            }
            _ => panic!("Expected ShouldNotify result"),
        }
    }

    #[test]
    fn test_check_index_monitor_state_no_action() {
        let config = IndexMonitorConfig::default();
        let state = NotificationState::default();
        
        // Small difference, should not notify
        let result = check_index_monitor_state(1050, 100, &state, &config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
    }

    #[test]
    fn test_check_index_monitor_state_no_action_due_to_time() {
        let config = IndexMonitorConfig::default();
        let mut state = NotificationState::default();
        state.latest_notify_time = Utc::now().timestamp() as u64 - 100; // Recent notification
        
        // Large difference but recent notification
        let result = check_index_monitor_state(1200, 100, &state, &config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
    }

    #[test]
    fn test_update_notification_state() {
        let mut state = NotificationState::default();
        let original_time = state.latest_notify_time;
        
        update_notification_state(&mut state);
        
        assert!(state.latest_notify_time > original_time);
    }

    #[test]
    fn test_edge_cases_block_difference() {
        let config = IndexMonitorConfig::default();
        
        // Edge case: exactly at the threshold
        assert!(!has_significant_block_difference(1100, 100, config.max_block_difference));
        
        // Edge case: just above threshold
        assert!(has_significant_block_difference(1101, 100, config.max_block_difference));
        
        // Edge case: zero difference
        assert!(!has_significant_block_difference(100, 100, config.max_block_difference));
        
        // Edge case: negative difference (shouldn't happen in real world but test for robustness)
        assert!(!has_significant_block_difference(50, 100, config.max_block_difference));
    }

    #[test]
    fn test_edge_cases_time_notification() {
        let max_interval = 600;
        
        // Edge case: exactly at the interval
        let exact_time = Utc::now().timestamp() as u64 - 600;
        assert!(!should_notify_based_on_time(exact_time, max_interval));
        
        // Edge case: just over the interval
        let just_over_time = Utc::now().timestamp() as u64 - 601;
        assert!(should_notify_based_on_time(just_over_time, max_interval));
    }

    #[test]
    fn test_comprehensive_monitor_scenarios() {
        let config = IndexMonitorConfig::default();
        let mut state = NotificationState::default();
        
        // Scenario 1: Normal operation - no action needed
        let result = check_index_monitor_state(1050, 1000, &state, &config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
        
        // Scenario 2: First notification - should notify
        let result = check_index_monitor_state(2100, 1000, &state, &config);
        assert!(matches!(result, IndexMonitorResult::ShouldNotify { .. }));
        
        // Update state after notification
        if let IndexMonitorResult::ShouldNotify { .. } = result {
            update_notification_state(&mut state);
        }
        
        // Scenario 3: Large difference but recent notification - no action
        let result = check_index_monitor_state(2200, 1000, &state, &config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
        
        // Scenario 4: Wait scenario - current behind cached
        let result = check_index_monitor_state(900, 1000, &state, &config);
        assert!(matches!(result, IndexMonitorResult::ShouldWait));
    }

    #[test]
    fn test_custom_config_values() {
        let custom_config = IndexMonitorConfig {
            max_block_difference: 500,
            max_notify_time_interval: 300,
        };
        let state = NotificationState::default();
        
        // With custom config, smaller difference should trigger notification
        let result = check_index_monitor_state(700, 100, &state, &custom_config);
        assert!(matches!(result, IndexMonitorResult::ShouldNotify { .. }));
        
        // But this difference wouldn't trigger with default config
        let default_config = IndexMonitorConfig::default();
        let result = check_index_monitor_state(700, 100, &state, &default_config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
    }

    #[test]
    fn test_notification_state_persistence() {
        let mut state = NotificationState::default();
        
        // Initial state
        assert_eq!(state.latest_notify_time, 0);
        
        // Simulate a notification
        update_notification_state(&mut state);
        let first_notification_time = state.latest_notify_time;
        assert!(first_notification_time > 0);
        
        // Simulate another notification after some time
        std::thread::sleep(std::time::Duration::from_millis(100)); // Longer delay to ensure time difference
        update_notification_state(&mut state);
        let second_notification_time = state.latest_notify_time;
        assert!(second_notification_time >= first_notification_time); // Use >= to handle edge cases
    }

    #[tokio::test]
    async fn test_complete_monitoring_workflow() {
        let config = IndexMonitorConfig::default();
        let mut state = NotificationState::default();
        
        // Simulate a complete monitoring cycle with different scenarios
        
        // Step 1: Normal operation - current block slightly ahead
        let current_block = 1100;
        let cached_block = mock_get_cached_index_block_number("normal").await;
        let result = check_index_monitor_state(current_block, cached_block, &state, &config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
        
        // Step 2: Large gap detected - should notify
        let current_block = 2100;
        let cached_block = mock_get_cached_index_block_number("normal").await;
        let result = check_index_monitor_state(current_block, cached_block, &state, &config);
        assert!(matches!(result, IndexMonitorResult::ShouldNotify { .. }));
        
        // Step 3: Update notification state
        if let IndexMonitorResult::ShouldNotify { .. } = result {
            update_notification_state(&mut state);
        }
        
        // Step 4: Large gap again but recent notification - no action
        let current_block = 2200;
        let cached_block = mock_get_cached_index_block_number("normal").await;
        let result = check_index_monitor_state(current_block, cached_block, &state, &config);
        assert!(matches!(result, IndexMonitorResult::NoAction));
        
        // Step 5: Current block behind cached - should wait
        let current_block = 900;
        let cached_block = mock_get_cached_index_block_number("behind").await;
        let result = check_index_monitor_state(current_block, cached_block, &state, &config);
        assert!(matches!(result, IndexMonitorResult::ShouldWait));
        
        // Step 6: ES connection error - cached block is 0
        let current_block = 1000;
        let cached_block = mock_get_cached_index_block_number("error").await;
        let result = check_index_monitor_state(current_block, cached_block, &state, &config);
        // With cached_block = 0, current_block > cached_block, but difference is 1000
        // which is not greater than max_difference (1000), so should be NoAction
        assert!(matches!(result, IndexMonitorResult::NoAction));
        
        // Step 7: Large enough difference to trigger notification
        let current_block = 2000;
        let cached_block = mock_get_cached_index_block_number("error").await;
        let result = check_index_monitor_state(current_block, cached_block, &state, &config);
        // With cached_block = 0, current_block = 2000, difference is 2000 > 1000
        // But we need to check if enough time has passed since last notification
        match result {
            IndexMonitorResult::ShouldNotify { .. } => {
                // Expected
            }
            IndexMonitorResult::NoAction => {
                // This is also valid if not enough time has passed
                println!("NoAction returned - this is valid if recent notification");
            }
            IndexMonitorResult::ShouldWait => {
                panic!("Unexpected ShouldWait result");
            }
        }
    }
} 