use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

// --- CONFIGURAÇÃO DO BENCHMARK ---
// 16384 é o número ideal para comparar com o teste de CPU que levou 37s.
// Se quiser testar o limite do notebook, suba para 32768.
const NUM_PARTICLES: u32 = 16384; 
const STEPS: u32 = 50; 

fn get_shader_code() -> String {
    // INJEÇÃO AUTOMÁTICA: O Rust escreve o número dentro do código do Shader.
    // Isso garante que o Loop da GPU bata exatamente com a alocação de memória.
    format!(r#"
        struct Particle {{
            pos: vec2<f32>,
            vel: vec2<f32>,
        }};

        @group(0) @binding(0) var<storage, read_write> particlesSrc : array<Particle>;
        @group(0) @binding(1) var<storage, read_write> particlesDst : array<Particle>;

        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let index = global_id.x;
            
            // Trava de segurança dinâmica
            if (index >= {}u) {{ return; }}

            var p = particlesSrc[index];
            var force = vec2<f32>(0.0, 0.0);

            // LOOP O(N^2) - Gargalo Matemático Puro
            for (var i = 0u; i < {}u; i = i + 1u) {{
                let other = particlesSrc[i];
                let d = other.pos - p.pos;
                let distSq = dot(d, d) + 0.01;
                let dist = sqrt(distSq);
                
                let f = d / (distSq * dist) * 0.0001; 
                force = force + f;
            }}

            p.vel = p.vel + force;
            p.pos = p.pos + p.vel;

            particlesDst[index] = p;
        }}
    "#, NUM_PARTICLES, NUM_PARTICLES)
}

fn log(s: &str) {
    web_sys::console::log_1(&JsValue::from_str(s));
}

#[wasm_bindgen]
pub async fn run_physics_bench() -> String {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    
    log(&format!("=== Iniciando GPU N-Body ({} Partículas, {} Passos) ===", NUM_PARTICLES, STEPS));

    // 1. Setup WebGPU (wgpu 23.0)
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }).await.expect("WebGPU não encontrado");

    let info = adapter.get_info();
    log(&format!("Hardware: {} ({:?})", info.name, info.backend));

    let mut limits = wgpu::Limits::downlevel_webgl2_defaults();
    limits.max_storage_buffer_binding_size = 128 * 1024 * 1024; // 128MB

    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ).await.expect("Erro ao criar Device");

    // 2. Dados Iniciais (Partículas em 0.5, 0.5)
    // Usamos vec! linear para facilitar (pos.x, pos.y, vel.x, vel.y)
    let mut initial_data = Vec::with_capacity((NUM_PARTICLES * 4) as usize);
    for _ in 0..NUM_PARTICLES {
        initial_data.push(0.5f32); // pos.x
        initial_data.push(0.5f32); // pos.y
        initial_data.push(0.0f32); // vel.x
        initial_data.push(0.0f32); // vel.y
    }
    let data_slice = bytemuck::cast_slice(&initial_data);
    let size_bytes = data_slice.len() as u64;

    // 3. Buffers
    // Buffer A (VRAM)
    let buffer_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Buffer A"),
        contents: data_slice,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    
    // Buffer B (VRAM - Ping Pong)
    let buffer_b = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Buffer B"),
        size: size_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Staging (RAM - Leitura)
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging"),
        size: size_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // 4. Pipeline
    let shader_code = get_shader_code();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("NBody Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_code.into()),
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"), // wgpu 23.0 exige Option
        compilation_options: Default::default(),
        cache: None,
    });

    // Bind Groups
    let bind_group_ab = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("A -> B"),
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buffer_a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buffer_b.as_entire_binding() },
        ],
    });

    let bind_group_ba = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("B -> A"),
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: buffer_b.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: buffer_a.as_entire_binding() },
        ],
    });

    // 5. Execução
    log("Iniciando simulação...");
    let window = web_sys::window().unwrap();
    let performance = window.performance().unwrap();
    let start = performance.now();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
        cpass.set_pipeline(&compute_pipeline);

        for i in 0..STEPS {
            if i % 2 == 0 {
                cpass.set_bind_group(0, &bind_group_ab, &[]);
            } else {
                cpass.set_bind_group(0, &bind_group_ba, &[]);
            }
            cpass.dispatch_workgroups(NUM_PARTICLES / 64, 1, 1);
        }
    }

    // Copia resultado final para Staging
    let source = if STEPS % 2 == 0 { &buffer_a } else { &buffer_b };
    encoder.copy_buffer_to_buffer(source, 0, &staging_buffer, 0, size_bytes);

    queue.submit(Some(encoder.finish()));

    // 6. Download Seguro
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| { let _ = tx.send(v); });
    
    device.poll(wgpu::Maintain::Wait);
    rx.await.unwrap().unwrap();

    let end = performance.now();
    let duration_s = (end - start) / 1000.0;
    
    // Leitura com escopo seguro para Unmap
    let check_val = {
        let view = buffer_slice.get_mapped_range();
        let floats: &[f32] = bytemuck::cast_slice(&view);
        floats[0] // Pega Pos X da primeira partícula
    };
    staging_buffer.unmap();

    // Métricas
    let interactions = (NUM_PARTICLES as f64) * (NUM_PARTICLES as f64) * (STEPS as f64);
    let gflops = (interactions / 1e9) / duration_s;

    format!(
        "🔥 RESULTADO FINAL (GPU):\n\nHardware: {}\nPartículas: {}\nPassos: {}\n\n⏱️ TEMPO: {:.4}s\n🚀 Performance: {:.2} G-Interações/s\n\nCheck: {:.4}", 
        info.name, NUM_PARTICLES, STEPS, duration_s, gflops, check_val
    )
}