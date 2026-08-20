pub mod analysis;
pub mod scoring;
pub mod shortlet;
pub mod simulation;

pub use analysis::{InvestmentAnalysis, InvestmentInputs};
pub use scoring::PropertyScoreInputs;
pub use shortlet::ShortletInvestmentInputs;
pub use simulation::ScenarioSimulationRequest;
