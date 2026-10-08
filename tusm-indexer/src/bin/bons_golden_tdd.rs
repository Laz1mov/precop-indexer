use precop_tusm_indexer::state_db::SigmaLeaf;
use primitive_types::U256;

fn calculate_golden_curve(
    payment_atoms: u128,
    weight: u128,
    matched_multiplier: u128,
    unit_scale: u128,
    current_sigma: &mut SigmaLeaf,
) -> u128 {
    // Formula 1: requested_points = floor(amount_atoms * weight * lock_multiplier / (unit_scale * denominator))
    let p_atoms = U256::from(payment_atoms);
    let w = U256::from(weight);
    let lock_mult = U256::from(matched_multiplier);
    let u_scale = U256::from(unit_scale);
    let denom = U256::from(10_000_000u128);

    let numerator = p_atoms * w * lock_mult;
    let requested_points = numerator / (u_scale * denom);

    // Formula 2: accepted_points = min(requested_points, point_limit - consumed_points)
    let point_limit = U256::from(current_sigma.limit_per_mint / 100_000_000);
    let max_supply = U256::from(current_sigma.max_supply);

    if point_limit > U256::zero() {
        let consumed = U256::from(current_sigma.consumed_points);
        let remaining_points = point_limit.saturating_sub(consumed);
        let accepted_points = std::cmp::min(requested_points, remaining_points);
        
        let accepted_points_u128 = if accepted_points > U256::from(u128::MAX) { u128::MAX } else { accepted_points.as_u128() };
        
        if accepted_points_u128 > 0 {
            let new_consumed_points = current_sigma.consumed_points.saturating_add(accepted_points_u128);
            
            // supply_after = cumulative_supply(new_consumed_points)
            let new_consumed = U256::from(new_consumed_points);
            let supply_after_u256 = (new_consumed * max_supply) / point_limit;
            let supply_after = if supply_after_u256 > U256::from(u128::MAX) { u128::MAX } else { supply_after_u256.as_u128() };
            
            let allocated_atoms = supply_after.saturating_sub(current_sigma.supply);
            
            // Update state
            current_sigma.consumed_points = new_consumed_points;
            current_sigma.supply = supply_after;
            
            return allocated_atoms;
        }
    }
    0
}

fn main() {
    let mut bons_state = SigmaLeaf {
        supply: 0,
        max_supply: 21_000_000 * 100_000_000, // 2.1 quadrillion
        limit_per_mint: 2_100_000 * 100_000_000, 
        btc_rate: 3333333, // BONS btc weight (same as LEAF)
        dog_rate: 0, leaf_rate: 0, ordi_rate: 0,
        anchor: bitcoin::OutPoint::null(), height: 968000, version: 1, halted: false,
        tge_root: [0u8; 32], consumed_points: 0,
    };

    // Simulated BONS treasury parameters
    let weight = bons_state.btc_rate as u128;
    let unit_scale = 100_000_000;
    // matched_multiplier for BONS (is_bonding is true) -> 16,180,339
    let matched_multiplier = 16_180_339;

    let test_payments = vec![
        150_000, 
        546,
        3_000_000, 
        500_000,
        38_799_192 / 2, // Half the max required sats
    ];

    println!("Testing BONS distribution with Golden Curve Multiplier (1.6180339)\n");

    for payment in test_payments {
        let atoms = calculate_golden_curve(
            payment, 
            weight, 
            matched_multiplier, 
            unit_scale, 
            &mut bons_state
        );
        println!("Payment: {} sats -> Minted BONS: {} (Total Supply: {} / {})", 
                 payment, atoms, bons_state.supply, bons_state.max_supply);
    }
    
    // Fill it up to the limit
    println!("\nFilling up the remainder to see the cap behavior:");
    let atoms = calculate_golden_curve(
        100_000_000_000, // Massive payment
        weight, 
        matched_multiplier, 
        unit_scale, 
        &mut bons_state
    );
    println!("Massive Payment -> Minted BONS: {} (Total Supply: {} / {})", 
             atoms, bons_state.supply, bons_state.max_supply);
}
