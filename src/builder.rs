use crate::trident_account::TridentAccountSharedData;
use crate::trident_entrypoint::TridentEntrypoint;
use crate::trident_program::TridentProgram;
use crate::trident_svm::TridentSVM;

#[derive(Default)]
pub struct TridentSVMConfig {
    syscalls_v1: bool,
    syscalls_v2: bool,
    program_entrypoints: Vec<TridentEntrypoint>,
    program_binaries: Vec<TridentProgram>,
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

    pub fn with_syscalls_v1(mut self) -> Self {
        self.config.syscalls_v1 = true;
        self
    }

    pub fn with_syscalls_v2(mut self) -> Self {
        self.config.syscalls_v2 = true;
        self
    }

    pub fn with_program_entries(mut self, entries: Vec<TridentEntrypoint>) -> Self {
        self.config.program_entrypoints = entries;
        self
    }

    pub fn with_sbf_programs(mut self, programs: Vec<TridentProgram>) -> Self {
        self.config.program_binaries = programs;
        self
    }

    pub fn with_permanent_accounts(mut self, accounts: Vec<TridentAccountSharedData>) -> Self {
        self.config.permanent_accounts = accounts;
        self
    }

    pub fn build(self) -> TridentSVM {
        let mut svm = TridentSVM::default();

        if self.config.syscalls_v1 {
            svm.initialize_syscalls_v1();
        }
        if self.config.syscalls_v2 {
            svm.initialize_syscalls_v2();
        }

        for entry in self.config.program_entrypoints {
            svm.deploy_entrypoint_program(&entry);
        }

        for program in self.config.program_binaries {
            svm.deploy_binary_program(&program);
        }

        for account in self.config.permanent_accounts {
            svm.accounts
                .set_permanent_account(&account.address, &account.account);
        }

        svm
    }
}
