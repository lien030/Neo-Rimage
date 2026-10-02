use std::{any::TypeId, collections::BTreeMap, collections::HashSet, fs, path::Path};

use ts_rs::{Config, TypeVisitor, TS};

use super::*;

struct ContractTypes {
    config: Config,
    visited: HashSet<TypeId>,
    declarations: BTreeMap<String, String>,
}

impl TypeVisitor for ContractTypes {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if !self.visited.insert(TypeId::of::<T>()) {
            return;
        }
        if T::output_path().is_some() {
            self.declarations.insert(
                T::ident(&self.config),
                format!("export {}", T::decl(&self.config)),
            );
        }
        T::visit_dependencies(self);
        T::visit_generics(self);
    }
}

fn generate_contracts() -> String {
    let mut types = ContractTypes {
        config: Config::new().with_large_int("number"),
        visited: HashSet::new(),
        declarations: BTreeMap::new(),
    };
    types.visit::<BackendCapabilities>();
    types.visit::<BackendSnapshot>();
    types.visit::<JobDetailSnapshot>();
    types.visit::<CreateJobCommand>();
    types.visit::<CreateJobResponse>();
    types.visit::<ScanInputsCommand>();
    types.visit::<ScanInputsResponse>();
    types.visit::<JobCommand>();
    types.visit::<RetryItemsCommand>();
    types.visit::<SetSchedulerPausedCommand>();
    types.visit::<SetWorkerCountCommand>();
    types.visit::<CommandAccepted<JobSnapshot>>();
    types.visit::<StateEventEnvelope>();
    types.visit::<NoticeEventEnvelope>();
    types.visit::<CommandErrorEnvelope>();

    let mut output = format!(
        "export const IPC_SCHEMA_VERSION = {IPC_SCHEMA_VERSION} as const;\nexport const JOB_CONFIG_VERSION = {JOB_CONFIG_VERSION} as const;\n\n"
    );
    for declaration in types.declarations.into_values() {
        output.push_str(&declaration);
        output.push_str("\n\n");
    }
    output.trim_end().to_owned() + "\n"
}

#[test]
fn typescript_contract_matches_rust() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/ipc/contracts.ts");
    let generated = generate_contracts();
    if std::env::var("NEO_RIMAGE_GENERATE_CONTRACTS").as_deref() == Ok("1") {
        fs::write(&path, &generated).expect("write TypeScript contract");
    }
    let existing = fs::read_to_string(&path).expect("read TypeScript contract");
    assert_eq!(
        existing.replace("\r\n", "\n"),
        generated,
        "Rust IPC types changed; run pnpm contracts:generate and commit the generated contract"
    );
    assert!(!generated.contains("bigint"));
}

#[test]
fn transparent_ids_and_counters_match_json_primitives() {
    let config = Config::new().with_large_int("number");
    assert_eq!(JobId::decl(&config), "type JobId = string;");
    assert_eq!(Revision::decl(&config), "type Revision = number;");
    assert_eq!(TimestampMs::decl(&config), "type TimestampMs = number;");
    assert_eq!(
        serde_json::to_value(JobId::from("job-1")).unwrap(),
        serde_json::json!("job-1")
    );
    assert_eq!(
        serde_json::to_value(Revision(42)).unwrap(),
        serde_json::json!(42)
    );
    assert_eq!(
        serde_json::to_value(TimestampMs(1_000)).unwrap(),
        serde_json::json!(1_000)
    );
}
