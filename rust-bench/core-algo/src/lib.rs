pub fn heavy_computation(size: usize) -> String {
    let start = std::time::Instant::now();
    
    // Simula uma matriz "grande" e faz algo pesado (multiplicação ingênua)
    // Inversão real é complexa de implementar do zero, multiplicação O(n^3) 
    // gasta o tempo que precisamos e é fácil.
    let mut matrix_a = vec![vec![1.5f64; size]; size];
    let matrix_b = vec![vec![2.5f64; size]; size];
    let mut result = vec![vec![0.0f64; size]; size];

    // O "loop da morte" para gastar CPU
    for i in 0..size {
        for j in 0..size {
            for k in 0..size {
                result[i][j] += matrix_a[i][k] * matrix_b[k][j];
            }
        }
    }
    
    // Retorna só um hash ou valor para provar que fez
    let duration = start.elapsed();
    format!("Processado matriz {}x{} em {:.2?}", size, size, duration)
}