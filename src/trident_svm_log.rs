pub fn log_message(message: &str, level: log::Level) {
    log::log!(level, "{message}",);
}

pub(crate) fn setup_cli_logging() {
    solana_logger::setup_with_default(
        "solana_rbpf::vm=debug,\
            solana_runtime::message_processor=debug,\
            solana_runtime::system_instruction_processor=trace",
    );
}

pub(crate) fn setup_file_logging(path: &str) {
    solana_logger::setup_file_with_default(
        path,
        "solana_rbpf::vm=debug,\
            solana_runtime::message_processor=debug,\
            solana_runtime::system_instruction_processor=trace,\
            trident_svm=debug",
    );
}

pub(crate) fn turn_off_solana_logging() {
    solana_logger::setup_with_default("off");
}
