#[macro_export]
macro_rules! include_modules {
    () => {
        pub mod api;
        pub mod auth;
        pub mod m3u_filter_error;
        pub mod messaging;
        pub mod model;
        pub mod processing;
        pub mod repository;
        pub mod utils;
        pub mod tools;
        pub mod foundation;
    }
}

