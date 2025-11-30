use crate::trident_svm::TridentSVM;
use crate::trident_svm_log::setup_cli_logging;
use crate::trident_svm_log::setup_file_logging;
use crate::trident_svm_log::turn_off_solana_logging;
use crate::types::trident_account::TridentAccountSharedData;
#[cfg(feature = "syscall-v2")]
use crate::types::trident_entrypoint::TridentEntrypoint;

#[derive(Default)]
pub struct TridentSVMConfig {
    syscalls_v1: bool,
    syscalls_v2: bool,
    cli_logs: bool, // TODO, add better debbug levels
    debug_file_logs: Option<String>,
    #[cfg(feature = "syscall-v2")]
    program_entrypoints: Vec<TridentEntrypoint>,
    permanent_accounts: Vec<TridentAccountSharedData>,
}

#[derive(Default)]
pub struct TridentSVMBuilder {
    config: TridentSVMConfig,
}

impl TridentSVMBuilder {
    pub fn new() -> Self {
        Self {
            config: TridentSVMConfig::default(),
        }
    }

    pub fn with_syscalls_v1(&mut self) -> &Self {
        self.config.syscalls_v1 = true;
        self
    }

    pub fn with_syscalls_v2(&mut self) -> &Self {
        self.config.syscalls_v2 = true;
        self
    }

    #[cfg(feature = "syscall-v2")]
    pub fn with_program_entries(&mut self, entries: Vec<TridentEntrypoint>) -> &Self {
        self.config.program_entrypoints = entries;
        self
    }

    pub fn with_permanent_accounts(&mut self, accounts: Vec<TridentAccountSharedData>) -> &Self {
        self.config.permanent_accounts = accounts;
        self
    }

    pub fn with_cli_logs(&mut self) -> &Self {
        self.config.cli_logs = true;
        self
    }

    pub fn with_debug_file_logs(&mut self, path: &str) -> &Self {
        self.config.debug_file_logs = Some(path.to_string());
        self
    }

    pub fn build(&self) -> TridentSVM {
        let mut svm = TridentSVM::default();

        #[cfg(feature = "syscall-v2")]
        if self.config.syscalls_v2 {
            svm.initialize_syscalls_v2();
        }

        if self.config.cli_logs {
            setup_cli_logging();
        } else if let Some(path) = &self.config.debug_file_logs {
            setup_file_logging(path);
        } else {
            turn_off_solana_logging();
        }

        #[cfg(feature = "syscall-v2")]
        for entry in &self.config.program_entrypoints {
            svm.deploy_entrypoint_program(entry);
        }

        for account in &self.config.permanent_accounts {
            svm.accounts
                .set_permanent_account(&account.address, &account.account);
        }

        svm
    }
}
