package main

import (
	"context"
	"fmt"
	"os"
	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

func main() {
	ctx := context.Background()
	r := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig())
	defer r.Close(ctx)
	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	data, _ := os.ReadFile("app/src/browser39/browser39_core.wasm")
	compiled, _ := r.CompileModule(ctx, data)
	inst, _ := r.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().WithName(""))

	// Check all exports
	for name := range inst.ExportedFunctionDefinitions() {
		fmt.Printf("  export: %s\n", name)
	}

	// Check cabi_realloc
	fn := inst.ExportedFunction("cabi_realloc")
	fmt.Printf("\ncabi_realloc: %v\n", fn != nil)

	if fn != nil {
		// Test allocation
		res, err := fn.Call(ctx, 0, 0, 1, 4)
		fmt.Printf("realloc(0,0,1,4): res=%v, err=%v\n", res, err)
	}
}
