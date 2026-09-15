use super::output::convert_result;
use crate::impl_;
use crate::types::{GateResult, GateSpec, VerifyError};
use crate::GateRunner;

pub struct ProductionGateRunner<'a, P, R> {
    probe: &'a P,
    runner: &'a R,
    gates: &'a impl_::GatesRoot,
    specs: &'a [impl_::GateSpec],
}

impl<'a, P, R> ProductionGateRunner<'a, P, R> {
    pub fn new(
        probe: &'a P,
        runner: &'a R,
        gates: &'a impl_::GatesRoot,
        specs: &'a [impl_::GateSpec],
    ) -> Self {
        Self {
            probe,
            runner,
            gates,
            specs,
        }
    }
}

impl<P, R> GateRunner for ProductionGateRunner<'_, P, R>
where
    P: impl_::ToolProbe + Send + Sync,
    R: impl_::ProcessRunner + Send + Sync,
{
    fn run(&self, gate: &GateSpec, _: &str) -> Result<GateResult, VerifyError> {
        let spec = self
            .specs
            .iter()
            .find(|spec| spec.id == gate.id)
            .ok_or_else(|| {
                VerifyError::InvalidGate(format!("unknown registered gate {}", gate.id))
            })?;
        let (result, output) =
            impl_::run_gate_with_output(spec, self.probe, self.runner, self.gates);
        Ok(convert_result(result, output.as_ref()))
    }
}
