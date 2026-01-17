use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::claude::{ExecutionUsage, ModelAlias};

/// Pricing information for a model
#[derive(Debug, Clone, Deserialize)]
pub struct ModelPricing {
    pub name: String,
    pub family: String,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    #[serde(default)]
    pub cache_write_5m_per_mtok: Option<f64>,
    #[serde(default)]
    pub cache_write_1h_per_mtok: Option<f64>,
    #[serde(default)]
    pub cache_read_per_mtok: Option<f64>,
    #[serde(default)]
    pub max_context_tokens: Option<u64>,
    #[serde(default)]
    pub long_context_input_per_mtok: Option<f64>,
    #[serde(default)]
    pub long_context_output_per_mtok: Option<f64>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub deprecation_date: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
}

/// Metadata about the price file
#[derive(Debug, Clone, Deserialize)]
pub struct PricingMeta {
    pub version: String,
    pub updated_at: String,
    #[serde(default)]
    pub source: Option<String>,
}

/// Batch API configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BatchConfig {
    pub discount_percent: u32,
    #[serde(default)]
    pub description: Option<String>,
}

/// Extra costs configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ExtrasCosts {
    #[serde(default)]
    pub web_search_per_1000: Option<f64>,
    #[serde(default)]
    pub web_fetch_per_request: Option<f64>,
    #[serde(default)]
    pub tool_overhead_tokens: Option<HashMap<String, u64>>,
}

/// Complete pricing configuration
#[derive(Debug, Clone, Deserialize)]
pub struct PricingConfig {
    pub meta: PricingMeta,
    pub aliases: HashMap<String, String>,
    pub models: HashMap<String, ModelPricing>,
    #[serde(default)]
    pub batch: Option<BatchConfig>,
    #[serde(default)]
    pub extras: Option<ExtrasCosts>,
}

impl PricingConfig {
    /// Load pricing from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read price file: {}", path.display()))?;
        Self::from_str(&content)
    }

    /// Parse pricing from a TOML string
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse price.toml")
    }

    /// Load default pricing (embedded in binary)
    pub fn default_pricing() -> Self {
        let content = include_str!("../../price.toml");
        Self::from_str(content).expect("Default price.toml should be valid")
    }

    /// Get the model ID for an alias
    pub fn resolve_alias(&self, alias: &str) -> Option<&str> {
        self.aliases.get(alias).map(|s| s.as_str())
    }

    /// Get pricing for a model by ID
    pub fn get_model(&self, model_id: &str) -> Option<&ModelPricing> {
        self.models.get(model_id)
    }

    /// Get pricing for a model alias
    pub fn get_model_by_alias(&self, alias: &ModelAlias) -> Option<&ModelPricing> {
        let model_id = alias.to_model_id();
        self.get_model(model_id)
    }

    /// List all available models
    pub fn list_models(&self) -> Vec<(&str, &ModelPricing)> {
        self.models.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    /// List non-deprecated models
    pub fn list_active_models(&self) -> Vec<(&str, &ModelPricing)> {
        self.models
            .iter()
            .filter(|(_, v)| !v.deprecated)
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }
}

/// Cost calculator for Claude API usage
#[derive(Debug)]
pub struct CostCalculator {
    config: PricingConfig,
}

impl CostCalculator {
    pub fn new(config: PricingConfig) -> Self {
        Self { config }
    }

    pub fn with_default_pricing() -> Self {
        Self::new(PricingConfig::default_pricing())
    }

    /// Calculate the cost for a given usage
    pub fn calculate_cost(&self, model: &ModelAlias, usage: &ExecutionUsage) -> CostBreakdown {
        let pricing = self.config.get_model_by_alias(model);

        let (input_rate, output_rate, cache_read_rate) = pricing
            .map(|p| {
                (
                    p.input_per_mtok,
                    p.output_per_mtok,
                    p.cache_read_per_mtok.unwrap_or(0.0),
                )
            })
            .unwrap_or((0.0, 0.0, 0.0));

        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * output_rate;
        let cache_read_cost = (usage.cache_read_tokens as f64 / 1_000_000.0) * cache_read_rate;

        CostBreakdown {
            input_cost,
            output_cost,
            cache_read_cost,
            cache_write_cost: 0.0, // Not tracked in ExecutionUsage
            total_cost: input_cost + output_cost + cache_read_cost,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_creation_tokens,
        }
    }

    /// Estimate cost for a given number of tokens
    pub fn estimate_cost(
        &self,
        model: &ModelAlias,
        input_tokens: u64,
        output_tokens: u64,
    ) -> CostEstimate {
        let pricing = self.config.get_model_by_alias(model);

        let (input_rate, output_rate) = pricing
            .map(|p| (p.input_per_mtok, p.output_per_mtok))
            .unwrap_or((0.0, 0.0));

        let input_cost = (input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * output_rate;

        CostEstimate {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            input_tokens,
            output_tokens,
        }
    }

    /// Get price per million tokens for a model
    pub fn get_rates(&self, model: &ModelAlias) -> Option<(f64, f64)> {
        self.config
            .get_model_by_alias(model)
            .map(|p| (p.input_per_mtok, p.output_per_mtok))
    }

    /// Get the pricing config
    pub fn config(&self) -> &PricingConfig {
        &self.config
    }
}

/// Detailed cost breakdown
#[derive(Debug, Clone, Default)]
pub struct CostBreakdown {
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_read_cost: f64,
    pub cache_write_cost: f64,
    pub total_cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

impl CostBreakdown {
    pub fn add(&mut self, other: &CostBreakdown) {
        self.input_cost += other.input_cost;
        self.output_cost += other.output_cost;
        self.cache_read_cost += other.cache_read_cost;
        self.cache_write_cost += other.cache_write_cost;
        self.total_cost += other.total_cost;
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }
}

/// Cost estimate for planning
#[derive(Debug, Clone)]
pub struct CostEstimate {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Format cost for display
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else if cost < 1.0 {
        format!("${:.3}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// Format tokens for display
pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.2}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_pricing() {
        let config = PricingConfig::default_pricing();
        assert!(!config.models.is_empty());
        assert!(config.aliases.contains_key("haiku"));
        assert!(config.aliases.contains_key("sonnet"));
        assert!(config.aliases.contains_key("opus"));
    }

    #[test]
    fn test_resolve_alias() {
        let config = PricingConfig::default_pricing();
        let model_id = config.resolve_alias("sonnet");
        assert!(model_id.is_some());
        assert!(model_id.unwrap().contains("sonnet"));
    }

    #[test]
    fn test_get_model_by_alias() {
        let config = PricingConfig::default_pricing();
        let pricing = config.get_model_by_alias(&ModelAlias::Sonnet);
        assert!(pricing.is_some());
        let p = pricing.unwrap();
        assert_eq!(p.family, "sonnet");
    }

    #[test]
    fn test_calculate_cost() {
        let calc = CostCalculator::with_default_pricing();
        let usage = ExecutionUsage {
            input_tokens: 1_000_000, // 1M tokens
            output_tokens: 100_000,  // 100K tokens
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_cost_usd: 0.0,
            duration_ms: 0,
        };

        let breakdown = calc.calculate_cost(&ModelAlias::Sonnet, &usage);

        // Sonnet 4.5: $3/MTok input, $15/MTok output
        assert!((breakdown.input_cost - 3.0).abs() < 0.01);
        assert!((breakdown.output_cost - 1.5).abs() < 0.01);
        assert!((breakdown.total_cost - 4.5).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost() {
        let calc = CostCalculator::with_default_pricing();
        let estimate = calc.estimate_cost(&ModelAlias::Haiku, 1_000_000, 500_000);

        // Haiku 4.5: $1/MTok input, $5/MTok output
        assert!((estimate.input_cost - 1.0).abs() < 0.01);
        assert!((estimate.output_cost - 2.5).abs() < 0.01);
        assert!((estimate.total_cost - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_get_rates() {
        let calc = CostCalculator::with_default_pricing();

        let (input, output) = calc.get_rates(&ModelAlias::Opus).unwrap();
        assert!((input - 5.0).abs() < 0.01); // Opus: $5/MTok input
        assert!((output - 25.0).abs() < 0.01); // Opus: $25/MTok output
    }

    #[test]
    fn test_cost_breakdown_add() {
        let mut total = CostBreakdown::default();
        let item = CostBreakdown {
            input_cost: 1.0,
            output_cost: 2.0,
            total_cost: 3.0,
            input_tokens: 100,
            output_tokens: 200,
            ..Default::default()
        };

        total.add(&item);
        assert!((total.total_cost - 3.0).abs() < 0.001);

        total.add(&item);
        assert!((total.total_cost - 6.0).abs() < 0.001);
        assert_eq!(total.input_tokens, 200);
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.001), "$0.0010");
        assert_eq!(format_cost(0.05), "$0.050");
        assert_eq!(format_cost(1.5), "$1.50");
        assert_eq!(format_cost(10.0), "$10.00");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.50M");
    }

    #[test]
    fn test_list_active_models() {
        let config = PricingConfig::default_pricing();
        let active = config.list_active_models();
        assert!(!active.is_empty());

        // All active models should have deprecated = false
        for (_, pricing) in &active {
            assert!(!pricing.deprecated);
        }
    }

    #[test]
    fn test_model_pricing_fields() {
        let config = PricingConfig::default_pricing();
        let sonnet = config.get_model_by_alias(&ModelAlias::Sonnet).unwrap();

        assert!(sonnet.max_context_tokens.is_some());
        assert!(sonnet.cache_read_per_mtok.is_some());
    }
}
