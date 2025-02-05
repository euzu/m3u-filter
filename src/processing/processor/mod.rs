pub mod playlist;
mod xtream;
mod affix;
mod xtream_vod;
mod xtream_series;

#[macro_export]
macro_rules! handle_error {
    ($stmt:expr, $map_err:expr) => {
        if let Err(err) = $stmt {
            $map_err(err);
        }
    };
}
use handle_error;

#[macro_export]
macro_rules! handle_error_and_return {
    ($stmt:expr, $map_err:expr) => {
        if let Err(err) = $stmt {
            $map_err(err);
            return Default::default();
        }
    };
}
use handle_error_and_return;


#[macro_export]
macro_rules! create_resolve_options_function_for_xtream_target {
    ($cluster:ident) => {
        paste::paste! {
            fn [<get_resolve_ $cluster _options>](target: &ConfigTarget, fpl: &FetchedPlaylist) -> (bool, u16) {
                let (resolve, resolve_delay) =
                    target.options.as_ref().map_or((false, 0), |opt| {
                        (opt.[<xtream_resolve_ $cluster>] && fpl.input.input_type == InputType::Xtream,
                         opt.[<xtream_resolve_ $cluster _delay>])
                    });
                (resolve, resolve_delay)
            }
        }
    };
}
use create_resolve_options_function_for_xtream_target;
