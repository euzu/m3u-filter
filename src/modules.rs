#[macro_export]
macro_rules! include_modules {
    () => {
        extern crate core;
        extern crate env_logger;
        extern crate pest;
        #[macro_use]
        extern crate pest_derive;
        pub mod api;
        pub mod auth;
        pub mod tuliprox_error;
        pub mod messaging;
        pub mod model;
        pub mod processing;
        pub mod repository;
        pub mod utils;
        pub mod tools;
        pub mod foundation;
    }
}

