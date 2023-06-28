#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
use wasmtime::{
    component::{Component, Linker},
    Config, Engine, Store,
};
use wasmtime_wasi::preview2::{
    self, wasi::clocks::wall_clock, wasi::filesystem::filesystem, Table, WasiCtx, WasiCtxBuilder,
    WasiView,
};

wasmtime::component::bindgen!({
    path: "../wit",
    world: "test-reactor",
    async: true,
    with: {
       "wasi:io/streams": preview2::wasi::io::streams,
       "wasi:filesystem/filesystem": preview2::wasi::filesystem::filesystem,
       "wasi:cli-base/environment": preview2::wasi::cli_base::environment,
       "wasi:cli-base/preopens": preview2::wasi::cli_base::preopens,
       "wasi:cli-base/exit": preview2::wasi::cli_base::exit,
       "wasi:cli-base/stdin": preview2::wasi::cli_base::stdin,
       "wasi:cli-base/stdout": preview2::wasi::cli_base::stdout,
       "wasi:cli-base/stderr": preview2::wasi::cli_base::stderr,
    },
    ownership: Borrowing {
        duplicate_if_necessary: false
    }
});

struct ReactorCtx {
    table: Table,
    wasi: WasiCtx,
}

impl WasiView for ReactorCtx {
    fn table(&self) -> &Table {
        &self.table
    }
    fn table_mut(&mut self) -> &mut Table {
        &mut self.table
    }
    fn ctx(&self) -> &WasiCtx {
        &self.wasi
    }
    fn ctx_mut(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[arg()]
    file: String,
}

async fn setup(args: Args) -> Result<(Store<ReactorCtx>, TestReactor)> {
    let mut config = Config::new();
    config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
    config.wasm_component_model(true);
    config.async_support(true);

    let engine = Engine::new(&config)?;

    let mut linker = Linker::new(&engine);

    // All of the imports available to the world are provided by the wasmtime-wasi preview2
    // implementation:
    preview2::wasi::poll::poll::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::io::streams::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::clocks::monotonic_clock::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::clocks::wall_clock::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::filesystem::filesystem::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::cli_base::environment::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::cli_base::preopens::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::cli_base::exit::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::cli_base::stdin::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::cli_base::stdout::add_to_linker(&mut linker, |x| x)?;
    preview2::wasi::cli_base::stderr::add_to_linker(&mut linker, |x| x)?;

    let mut table = Table::new();
    let wasi = WasiCtxBuilder::new()
        .push_env("GOOD_DOG", "gussie")
        .push_env("POUTY_DOG", "willa")
        .build(&mut table)?;

    let mut store = Store::new(&engine, ReactorCtx { table, wasi });

    let component = Component::from_file(&engine, args.file)?;

    let (reactor, _instance) =
        TestReactor::instantiate_async(&mut store, &component, &linker).await?;

    Ok((store, reactor))
}

//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let (mut store, mut reactor) = setup(args).await?;

    // Show that integration with the WASI context is working - the guest will
    // interpolate $GOOD_DOG to gussie here using the environment:
    let r = reactor
        .call_add_strings(&mut store, &["hello", "$GOOD_DOG"])
        .await?;
    assert_eq!(r, 2);

    let contents = reactor.call_get_strings(&mut store).await?;
    println!("call_get_strings: {contents:?}");
    assert_eq!(contents, &["hello", "gussie"]);

    demo_async_output_stream(&mut store, &mut reactor).await?;

    Ok(())
}

//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//

async fn demo_memory_output_stream(
    store: &mut Store<ReactorCtx>,
    reactor: &mut TestReactor,
) -> Result<()> {
    // Show that we can pass in a resource type whose impls are defined in the
    // `host` and `wasi-common` crate.
    // Note, this works because of the add_to_linker invocations using the
    // `host` crate for `streams`, not because of `with` in the bindgen macro.
    let writepipe = preview2::pipe::MemoryOutputPipe::new();
    let table_ix = preview2::TableStreamExt::push_output_stream(
        store.data_mut().table_mut(),
        Box::new(writepipe.clone()),
    )?;
    let r = reactor.call_write_strings_to(store, table_ix).await?;
    assert_eq!(r, Ok(()));
    assert_eq!(writepipe.contents(), b"hellogussie");
    Ok(())
}

//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//

async fn demo_async_output_stream(
    store: &mut Store<ReactorCtx>,
    reactor: &mut TestReactor,
) -> Result<()> {
    let (mut client, server) = tokio::io::duplex(64);

    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut buffer = Vec::new();
        loop {
            let mut bs = [0];
            match client.read_exact(&mut bs).await {
                Ok(_) => {
                    if bs[0] == b'\n' {
                        println!("stream read: {:?}", String::from_utf8_lossy(&buffer));
                        buffer = Vec::new();
                    } else {
                        buffer.push(bs[0]);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    println!("stream read: {:?}", String::from_utf8_lossy(&buffer));
                    println!("end of stream");
                    break;
                }
                Err(e) => panic!("unexpected error: {:?}", e),
            };
        }
    });

    // Show that we can pass in a resource type whose impls are defined in the
    // `host` and `wasi-common` crate.
    // Note, this works because of the add_to_linker invocations using the
    // `host` crate for `streams`, not because of `with` in the bindgen macro.
    let writepipe = preview2::AsyncWriteStream::new(server);
    let table_ix = preview2::TableStreamExt::push_output_stream(
        store.data_mut().table_mut(),
        Box::new(writepipe),
    )?;
    let r = reactor.call_write_strings_to(store, table_ix).await?;
    assert_eq!(r, Ok(()));

    Ok(())
}

//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//
//

async fn demo_wasi_structures(
    store: &mut Store<ReactorCtx>,
    reactor: &mut TestReactor,
) -> Result<()> {
    // Show that the `with` invocation in the macro means we get to re-use the
    // type definitions from inside the `host` crate for these structures:
    let ds = filesystem::DescriptorStat {
        data_access_timestamp: wall_clock::Datetime {
            nanoseconds: 123,
            seconds: 45,
        },
        data_modification_timestamp: wall_clock::Datetime {
            nanoseconds: 789,
            seconds: 10,
        },
        device: 0,
        inode: 0,
        link_count: 0,
        size: 0,
        status_change_timestamp: wall_clock::Datetime {
            nanoseconds: 0,
            seconds: 1,
        },
        type_: filesystem::DescriptorType::Unknown,
    };
    let expected = format!("{ds:?}");
    let got = reactor.call_pass_an_imported_record(store, ds).await?;
    assert_eq!(expected, got);
    Ok(())
}
