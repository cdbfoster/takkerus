pub fn relu(x: f32) -> f32 {
    x.max(0.0)
}

pub fn relu_prime(x: f32) -> f32 {
    match x > 0.0 {
        true => 1.0,
        false => 0.0,
    }
}

pub fn tanh(x: f32) -> f32 {
    x.tanh()
}

pub fn tanh_prime(x: f32) -> f32 {
    let x_tanh = x.tanh();
    1.0 - x_tanh * x_tanh
}
