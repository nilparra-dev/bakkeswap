use anyhow::{bail, Result};

use crate::domain::models::SwapPlanRecord;

#[derive(Debug, Default)]
pub struct PlannerService;

impl PlannerService {
    pub fn create_plan(&self, _target_product_id: i64, _source_product_id: i64) -> Result<SwapPlanRecord> {
        bail!("not implemented: planner service")
    }
}
