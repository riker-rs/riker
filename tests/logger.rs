use futures::executor::block_on;

use riker::actors::*;
use slog::{Logger, Fuse, o};

mod common {
    use std::{fmt, result};

    use slog::*;

    pub struct PrintlnSerializer;

    impl Serializer for PrintlnSerializer {
        fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> Result {
            print!(", {}={}", key, val);
            Ok(())
        }
    }

    pub struct PrintlnDrain;

    impl Drain for PrintlnDrain {
        type Ok = ();
        type Err = ();

        fn log(
            &self,
            record: &Record,
            values: &OwnedKVList,
        ) -> result::Result<Self::Ok, Self::Err> {

            print!("{}", record.msg());

            record
                .kv()
                .serialize(record, &mut PrintlnSerializer)
                .unwrap();
            values.serialize(record, &mut PrintlnSerializer).unwrap();

            println!();
            Ok(())
        }
    }
}

#[test]
fn system_create_with_slog() {
    let log = Logger::root(Fuse(common::PrintlnDrain), o!("version" => "v1", "run_env" => "test"));
    let sys = SystemBuilder::new()
        .log(log)
        .create().unwrap();
    block_on(sys.shutdown()).unwrap();
}