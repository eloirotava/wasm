#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

pub fn heavy_computation(size: usize) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let start = std::time::Instant::now();

    let mut matrix_a = vec![vec![1.5f64; size]; size];
    let matrix_b = vec![vec![2.5f64; size]; size];
    // Inicializa com zeros
    let mut result = vec![vec![0.0f64; size]; size];

    // --- A MÁGICA DO PARALELISMO AQUI ---
    
    // VERSÃO NATIVA (Multithread - Rayon)
    #[cfg(not(target_arch = "wasm32"))]
    {
        // par_iter_mut() divide as linhas da matriz entre os núcleos da CPU automaticamente
        result.par_iter_mut().enumerate().for_each(|(i, row)| {
            for j in 0..size {
                for k in 0..size {
                    row[j] += matrix_a[i][k] * matrix_b[k][j];
                }
            }
        });
    }

    // VERSÃO WASM (Single Thread - Loop Normal)
    #[cfg(target_arch = "wasm32")]
    {
        for i in 0..size {
            for j in 0..size {
                for k in 0..size {
                    result[i][j] += matrix_a[i][k] * matrix_b[k][j];
                }
            }
        }
    }
    // ------------------------------------

    #[cfg(not(target_arch = "wasm32"))]
    {
        let duration = start.elapsed();
        // Adicionei uma flag "(MT)" para saber que foi MultiThread
        return format!("Processado matriz {}x{} em {:.2?} (MT - {} cores)", size, size, duration, rayon::current_num_threads());
    }

    #[cfg(target_arch = "wasm32")]
    {
        return format!("Processado matriz {}x{} (WASM Single-Core)", size, size);
    }
}