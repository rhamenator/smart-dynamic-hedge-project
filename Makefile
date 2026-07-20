.PHONY: build test demo serve mcp clean

build:
	cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
	cmake --build build --config Release -j
	cargo build --release --manifest-path rust/Cargo.toml -p smart_hedge_cli

test: build
	ctest --test-dir build --output-on-failure
	cargo test --workspace --manifest-path rust/Cargo.toml

demo: build
	SMART_HEDGE_CORE=build/smart_dynamic_hedge ./rust/target/release/smart-hedge once --symbol SPY

serve: build
	SMART_HEDGE_CORE=build/smart_dynamic_hedge ./rust/target/release/smart-hedge serve

mcp: build
	SMART_HEDGE_CORE=build/smart_dynamic_hedge ./rust/target/release/smart-hedge mcp

clean:
	rm -rf build .smart_hedge rust/target
