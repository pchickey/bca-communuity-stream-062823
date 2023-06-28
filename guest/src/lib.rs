wit_bindgen::generate!("test-reactor" in "../wit");

export_test_reactor!(T);

struct T;

static mut STATE: Vec<String> = Vec::new();

impl TestReactor for T {
    fn add_strings(ss: Vec<String>) -> u32 {
        for s in ss {
            match s.split_once("$") {
                Some((prefix, var)) if prefix.is_empty() => match std::env::var(var) {
                    Ok(val) => unsafe { STATE.push(val) },
                    Err(_) => unsafe { STATE.push("undefined".to_owned()) },
                },
                _ => unsafe { STATE.push(s) },
            }
        }
        unsafe { STATE.len() as u32 }
    }
    fn get_strings() -> Vec<String> {
        unsafe { STATE.clone() }
    }

    fn write_strings_to(o: OutputStream) -> Result<(), ()> {
        for s in Self::get_strings() {
            let output = format!("{s}\n");
            wasi::io::streams::write(o, output.as_bytes()).map_err(|_| ())?;

            //std::thread::sleep(std::time::Duration::from_secs(1));
        }
        Ok(())
    }
    fn pass_an_imported_record(stat: wasi::filesystem::filesystem::DescriptorStat) -> String {
        format!("{stat:?}")
    }
}
