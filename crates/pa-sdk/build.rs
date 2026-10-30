// Compila o party_anywhere.h com a implementação: garante que o cabeçalho compila em C a cada build e
// dá aos testes as funções C de verdade para conversarem com o lado Rust.
fn main() {
    println!("cargo:rerun-if-changed=../../sdk/c/party_anywhere.h");
    println!("cargo:rerun-if-changed=shim.c");
    println!("cargo:rerun-if-changed=../../sdk/c/examples/arena_console.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new().file("shim.c").include("../../sdk/c").warnings(true).compile("pa_sdk_shim");
        // o exemplo também precisa compilar (sem o main, que é do executável dele)
        cc::Build::new()
            .file("../../sdk/c/examples/arena_console.c")
            .define("PA_EXAMPLE_NO_MAIN", None)
            .define("arena_step", "pa_example_arena_step")
            .define("PARTY_ANYWHERE_IMPLEMENTED", None)
            .warnings(true)
            .compile("pa_sdk_example");
    }
}
