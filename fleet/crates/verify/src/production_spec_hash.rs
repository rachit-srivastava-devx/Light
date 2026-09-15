use super::super::tree::file_hash::bounded_file_digest;
use crate::impl_;
use crate::VerifyError;

pub(super) fn append_spec(
    hasher: &mut blake3::Hasher,
    gates: &impl_::GatesRoot,
    spec: &impl_::GateSpec,
) -> Result<(), VerifyError> {
    append_unresolved_spec(hasher, spec);
    if let impl_::GateCommand::Script { relative, .. } = spec.command {
        let path = gates.require(relative).map_err(|error| {
            VerifyError::InvalidCandidate(format!("gate '{}' cannot be resolved: {error}", spec.id))
        })?;
        field(hasher, b"resolved-script");
        field(hasher, relative.as_bytes());
        field(
            hasher,
            &bounded_file_digest(&path).map_err(|error| {
                VerifyError::InvalidCandidate(format!(
                    "gate '{}' asset unreadable: {error}",
                    spec.id
                ))
            })?,
        );
    }
    Ok(())
}

pub(crate) fn append_unresolved_spec(hasher: &mut blake3::Hasher, spec: &impl_::GateSpec) {
    field(hasher, spec.id.as_bytes());
    field(
        hasher,
        match spec.requirement {
            impl_::Requirement::Required => b"required",
            impl_::Requirement::Advisory => b"advisory",
        },
    );
    field(hasher, probe_name(spec.probe).as_bytes());
    match spec.command {
        impl_::GateCommand::OnPath(args) => {
            field(hasher, b"path");
            field_args(hasher, args);
        }
        impl_::GateCommand::Script { relative, args } => {
            field(hasher, b"script");
            field(hasher, relative.as_bytes());
            field_args(hasher, args);
        }
    }
}

pub(crate) fn field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn field_args(hasher: &mut blake3::Hasher, args: &[&str]) {
    field(hasher, &(args.len() as u64).to_le_bytes());
    for arg in args {
        field(hasher, arg.as_bytes());
    }
}

fn probe_name(probe: impl_::ProbeTool) -> String {
    match probe {
        impl_::ProbeTool::Cargo => "cargo".into(),
        impl_::ProbeTool::CargoFmt => "cargo-fmt".into(),
        impl_::ProbeTool::CargoClippy => "cargo-clippy".into(),
        impl_::ProbeTool::CargoDeny => "cargo-deny".into(),
        impl_::ProbeTool::CargoAudit => "cargo-audit".into(),
        impl_::ProbeTool::CargoLlvmCov => "cargo-llvm-cov".into(),
        impl_::ProbeTool::CargoMutants => "cargo-mutants".into(),
        impl_::ProbeTool::Named(name) => format!("named:{name}"),
    }
}
