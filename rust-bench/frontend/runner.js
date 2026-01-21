// frontend/runner.js
const { run_wasm_bench } = require('./pkg/wasm_frontend.js');

const SIZE = 2000; // Mesmo tamanho do teste nativo

console.log(`=== BENCHMARK WASM (Node.js) ===`);
console.log(`Matriz: ${SIZE}x${SIZE}`);

const start = performance.now();
const result = run_wasm_bench(SIZE);
const end = performance.now();

console.log(`Resultado: ${result}`);
console.log(`Tempo Total (JS + WASM): ${((end - start) / 1000).toFixed(4)}s`);