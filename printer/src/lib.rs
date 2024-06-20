/// use ansi_term::Colour::{Red, Blue, Green, Yellow, Cyan, Purple};
/// This module contains the `show` module.
pub mod show {
    #[macro_export]
    macro_rules! show {
        ($file:expr, $line:expr, $func:expr, $($args: expr),*) => {
            //std::thread::sleep(std::time::Duration::from_millis(100));
            if false {
                $(
                    let s = $func.to_string();
                    let names: Vec<&str> = s.split("::").collect();
                    let fname = if names.len() >= 3 {
                        names[names.len()-3]
                    }
                    else{
                        "__"
                    };

                    print!("[{}] {}:{}:\n{}():\n{:?}\n\n", chrono::prelude::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(), $file.to_string(), $line, fname, $args);
                )*
            }
        }
    }

    pub mod func {
        #[macro_export]
        macro_rules! func {
            () => {{
                struct S;
                std::any::type_name::<S>()
            }};
        }
    }
}
