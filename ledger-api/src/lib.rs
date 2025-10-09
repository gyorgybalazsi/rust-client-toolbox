pub use com::daml::ledger::api::v2 as v2;

pub mod com {
    pub mod daml {
        pub mod ledger {
            pub mod api {
                pub mod v2 {
                    include!("pb/com.daml.ledger.api.v2.rs");
                    pub mod testing {
                        include!("pb/com.daml.ledger.api.v2.testing.rs");
                    }
                    pub mod admin {
                        include!("pb/com.daml.ledger.api.v2.admin.rs");
                    }
                    pub mod interactive {
                        include!("pb/com.daml.ledger.api.v2.interactive.rs");
                        pub mod transaction {
                            pub mod v1 {
                                include!("pb/com.daml.ledger.api.v2.interactive.transaction.v1.rs");
                            }
                        }
                    }
                }
            }
        }
    }
}
pub mod google {
    // pub mod protobuf {
    //     include!(concat!(env!("OUT_DIR"), "/google.protobuf.rs"));
    // }

    pub mod rpc {
        include!("pb/google.rpc.rs");
    }
}
