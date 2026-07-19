.PHONY: build test demo serve mcp clean
PYTHON ?= python3

build:
	cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
	cmake --build build --config Release -j

test: build
	ctest --test-dir build --output-on-failure
	PYTHONPATH=python $(PYTHON) -m unittest discover -s tests -v

demo: build
	PYTHONPATH=python SMART_HEDGE_CORE=build/smart_dynamic_hedge $(PYTHON) -m smart_hedge.cli once --symbol SPY

serve: build
	PYTHONPATH=python SMART_HEDGE_CORE=build/smart_dynamic_hedge $(PYTHON) -m smart_hedge.cli serve

mcp: build
	PYTHONPATH=python SMART_HEDGE_CORE=build/smart_dynamic_hedge $(PYTHON) -m smart_hedge.cli mcp

clean:
	rm -rf build .smart_hedge __pycache__ python/smart_hedge/__pycache__ tests/__pycache__
