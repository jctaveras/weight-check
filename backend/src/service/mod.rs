//! Business layer: model lifecycle, benchmark orchestration and system information.

pub mod benchmark_service;
pub mod benchmarks;
pub mod model_service;
pub mod system_service;

#[cfg(test)]
pub(crate) mod testing;
