cargo run --bin bin -- "unexistant.js" -p "./tests"
cargo run --bin bin -- "a.mjs" -p "./tests"
cargo run --bin bin -- "a.mjs" -p "./tests" -x "echo [info]"
cargo run --bin bin -- -r "node ./a.mjs" -p "./tests"