pub(crate) fn setup_solana_logging() {
    #[rustfmt::skip]
    solana_logger::setup_with_default(
        "solana_rbpf::vm=debug,\
            solana_runtime::message_processor=debug,\
            solana_runtime::system_instruction_processor=trace",
    );
}

pub(crate) fn turn_off_solana_logging() {
    solana_logger::setup_with_default("off");
}
