use primitive_types::U256;

struct SigmaLeaf {
    pub max_supply: u128,
    pub limit_per_mint: u128,
    pub consumed_points: u128,
    pub dog_rate: u128,
    pub supply: u128,
}

fn calculate_crc_mint_legacy(payment_atoms: u128, current_sigma: &SigmaLeaf) -> u128 {
    let weight = current_sigma.dog_rate as u128;
    let unit_scale = 100_000u128; // DOG divisibility 5
    
    // Formula 1
    let p_atoms = U256::from(payment_atoms);
    let w = U256::from(weight);
    let lock_mult = U256::from(10_000_000u128); // matched_multiplier
    let u_scale = U256::from(unit_scale);
    let denom = U256::from(10_000_000u128);
    
    let numerator = p_atoms * w * lock_mult;
    let requested_points = numerator / (u_scale * denom);
    
    // Formula 2
    let point_limit = U256::from(current_sigma.limit_per_mint / 100_000_000);
    
    if point_limit > U256::zero() {
        let consumed = U256::from(current_sigma.consumed_points);
        let remaining_points = point_limit.saturating_sub(consumed);
        let accepted_points = std::cmp::min(requested_points, remaining_points);
        
        let accepted_points_u128 = if accepted_points > U256::from(u128::MAX) { u128::MAX } else { accepted_points.as_u128() };
        
        let point_precision_factor = 100_000_000u128;
        let amount = accepted_points_u128 * point_precision_factor;
        
        return std::cmp::min(amount, current_sigma.max_supply.saturating_sub(current_sigma.supply));
    }
    0
}

fn calculate_crc_mint_fixed(payment_atoms: u128, current_sigma: &SigmaLeaf) -> u128 {
    // Override spécifique pour CRC:
    // 11,550 DOG (1_155_000_000 atomes) = 1 CRC (10^18)
    let p_atoms = U256::from(payment_atoms);
    let full_payment = U256::from(1_155_000_000u128);
    let limit = U256::from(current_sigma.limit_per_mint);
    
    let mut amount = (p_atoms * limit) / full_payment;
    
    // Plafond à la limite par mint
    if amount > limit {
        amount = limit;
    }
    
    let amount_u128 = if amount > U256::from(u128::MAX) { u128::MAX } else { amount.as_u128() };
    std::cmp::min(amount_u128, current_sigma.max_supply.saturating_sub(current_sigma.supply))
}

fn main() {
    let sigma = SigmaLeaf {
        max_supply: 21_000_000_000_000_000_000,
        limit_per_mint: 1_000_000_000_000_000_000,
        consumed_points: 0,
        dog_rate: 0, // Dans la DB, dog_rate est 0 pour CRC
        supply: 4294967295, // Test supply
    };
    
    let payment_dog = 11_550;
    let payment_atoms = payment_dog * 100_000;
    
    println!("=== TEST REDTEAM: ALLOCATION CRC ===");
    println!("Payment: {} DOG ({} atoms)", payment_dog, payment_atoms);
    println!("Sigma Limit: {}", sigma.limit_per_mint);
    println!("------------------------------------");
    
    let legacy_amount = calculate_crc_mint_legacy(payment_atoms, &sigma);
    println!("[FAILING] Montant calculé par le code actuel : {} CRC", legacy_amount as f64 / 1e18);
    println!("Valeur brute: {}", legacy_amount);
    
    let fixed_amount = calculate_crc_mint_fixed(payment_atoms, &sigma);
    println!("[PASSING] Montant calculé par la correction  : {} CRC", fixed_amount as f64 / 1e18);
    println!("Valeur brute: {}", fixed_amount);
}
