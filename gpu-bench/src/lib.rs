use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

const N: u32 = 4096;

const SHADER_CODE: &str = r#"
@group(0) @binding(0) var<storage, read> matrixA : array<f32>;
@group(0) @binding(1) var<storage, read> matrixB : array<f32>;
@group(0) @binding(2) var<storage, read_write> result : array<f32>;

const N: u32 = 1024u;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id : vec3<u32>) {
    let row = global_id.x;
    let col = global_id.y;

    if (row >= N || col >= N) {
        return;
    }

    var sum = 0.0;
    for (var k = 0u; k < N; k = k + 1u) {
        let a_idx = row * N + k;
        let b_idx = k * N + col;
        sum = sum + matrixA[a_idx] * matrixB[b_idx];
    }

    let result_idx = row * N + col;
    result[result_idx] = sum;
}
"#;

fn log(s: &str) {
    web_sys::console::log_1(&JsValue::from_str(s));
}

#[wasm_bindgen]
pub async fn run_gpu_bench() -> String {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    
    log("=== Iniciando GPU Benchmark (Staging Fix) ===");

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("ERRO CRÍTICO: Não foi possível encontrar um adaptador WebGPU!");

    let info = adapter.get_info();
    log(&format!("Hardware Detectado: {} ({:?})", info.name, info.backend));

    let mut required_limits = wgpu::Limits::downlevel_webgl2_defaults();
    required_limits.max_storage_buffer_binding_size = 128 * 1024 * 1024; 

    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits, 
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ).await.expect("ERRO: Falha ao criar dispositivo lógico (Device).");

    let size_bytes = (N * N * 4) as u64;
    let matrix_a_host = vec![1.5f32; (N * N) as usize];
    let matrix_b_host = vec![2.5f32; (N * N) as usize];

    log(&format!("Alocando buffers ({:.2} MB)...", (size_bytes as f64 * 3.0)/1024.0/1024.0));

    let buffer_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Matrix A"), contents: bytemuck::cast_slice(&matrix_a_host), usage: wgpu::BufferUsages::STORAGE,
    });
    let buffer_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Matrix B"), contents: bytemuck::cast_slice(&matrix_b_host), usage: wgpu::BufferUsages::STORAGE,
    });
    
    // --- CORREÇÃO: ARQUITETURA DE STAGING ---
    
    // 1. Buffer de GPU (Rápido, Privado da GPU)
    // O shader escreve aqui. Nós COPIAMOS dele (COPY_SRC).
    let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Storage Buffer (VRAM)"),
        size: size_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // 2. Buffer de Staging (Lento, Acessível pela CPU)
    // Nós lemos aqui (MAP_READ). Nós COPIAMOS para ele (COPY_DST).
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer (RAM)"),
        size: size_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"), source: wgpu::ShaderSource::Wgsl(SHADER_CODE.into()),
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Pipeline"), 
        layout: None, 
        module: &shader, 
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buffer_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buffer_b.as_entire_binding() },
            // IMPORTANTE: O Shader liga-se ao storage_buffer (VRAM), não ao staging
            wgpu::BindGroupEntry { binding: 2, resource: storage_buffer.as_entire_binding() },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Compute Pass"), timestamp_writes: None });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.dispatch_workgroups(N / 16, N / 16, 1);
    }
    
    // 3. COMANDO DE CÓPIA: GPU -> Staging
    // Isso acontece na GPU, ultra rápido.
    encoder.copy_buffer_to_buffer(&storage_buffer, 0, &staging_buffer, 0, size_bytes);

    queue.submit(Some(encoder.finish()));

    log("Comando enviado para GPU... Aguardando hardware.");
    
    let window = web_sys::window().expect("window");
    let performance = window.performance().expect("performance");
    let start = performance.now();

    // 4. Mapear o buffer de STAGING (não o de storage)
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| {
        let _ = tx.send(v);
    });

    device.poll(wgpu::Maintain::Wait);
    
    match rx.await {
        Ok(Ok(())) => log("Dados transferidos para CPU!"),
        _ => return "ERRO FATAL: Ocorreu um erro ao ler a memória da GPU".to_string(),
    }

    let end = performance.now();
    let duration = (end - start) / 1000.0;

    let check_val = {
        let view = buffer_slice.get_mapped_range();
        let floats: &[f32] = bytemuck::cast_slice(&view);
        floats[0]
    };
    
    // Libera o staging para uso futuro
    staging_buffer.unmap();

    format!(
        "✅ SUCESSO!\nHardware: {}\nTempo Total (Calc + Download): {:.4}s\nMatriz: {}x{}\nCheck: {:.2}", 
        info.name, duration, N, N, check_val
    )
}