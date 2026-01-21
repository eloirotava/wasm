use std::time::Instant;

// CONFIGURAÇÃO IDÊNTICA À DA GPU
const NUM_PARTICLES: usize = 16384;
const STEPS: usize = 50;

#[derive(Clone, Copy)] // Copy para ser rápido como na GPU
struct Particle {
    pos: [f32; 2],
    vel: [f32; 2],
}

fn main() {
    println!("=== N-BODY CPU BENCHMARK ===");
    println!("Partículas: {}", NUM_PARTICLES);
    println!("Passos: {}", STEPS);
    println!("Modo: Single-Thread (Monocore)");
    println!("Iniciando cálculo massivo (aguarde, isso vai demorar)...");

    // 1. Inicializa Dados (Igual ao Buffer da GPU)
    let mut particles: Vec<Particle> = (0..NUM_PARTICLES).map(|_| Particle {
        pos: [0.5, 0.5],
        vel: [0.0, 0.0],
    }).collect();

    let start = Instant::now();

    // 2. Loop de Simulação
    for step in 0..STEPS {
        if step % 5 == 0 {
            print!("."); // Feedback visual para não parecer travado
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }

        // Precisamos de uma cópia do estado anterior para ler
        // (Na GPU isso era o Buffer A vs Buffer B)
        let old_particles = particles.clone();

        // LOOP O(N^2) - O Teste de Fogo
        for i in 0..NUM_PARTICLES {
            let p = &mut particles[i];
            let mut force_x = 0.0;
            let mut force_y = 0.0;

            for j in 0..NUM_PARTICLES {
                let other = &old_particles[j];
                
                let dx = other.pos[0] - p.pos[0];
                let dy = other.pos[1] - p.pos[1];
                
                // Matemática idêntica ao Shader:
                // let distSq = dot(d, d) + 0.01;
                let dist_sq = dx*dx + dy*dy + 0.01;
                let dist = dist_sq.sqrt();
                
                // let f = d / (distSq * dist) * 0.0001; 
                let f = 0.0001 / (dist_sq * dist);
                
                force_x += dx * f;
                force_y += dy * f;
            }

            p.vel[0] += force_x;
            p.vel[1] += force_y;
            p.pos[0] += p.vel[0];
            p.pos[1] += p.vel[1];
        }
    }

    let duration = start.elapsed();
    println!("\n\n✅ FINALIZADO!");
    println!("Tempo Total: {:.4}s", duration.as_secs_f32());
    
    // Cálculo de GFLOPS
    let interactions = (NUM_PARTICLES as f64) * (NUM_PARTICLES as f64) * (STEPS as f64);
    let gflops = (interactions / 1e9) / duration.as_secs_f64();
    println!("Performance: {:.2} G-Interações/s", gflops);

    // Prova Real (Check)
    println!("Check (Pos X[0]): {:.4}", particles[0].pos[0]);
}