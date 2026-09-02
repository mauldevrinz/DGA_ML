import re

with open("src/main.rs", "r") as f:
    code = f.read()

pump1_new = """
    // ===================================================
    // PUMP 1 - motor driver BTS7960
    // ===================================================
    let mut pump1_l_en = gpio4.output(pins.p5);
    let _ = pump1_l_en.set_high(); // Enable bridge L
    
    let mut pump1_r_en = gpio4.output(pins.p4);
    let _ = pump1_r_en.set_high(); // Enable bridge R
    
    let (mut pwm4, (_pwm4_sm0, _pwm4_sm1, mut pwm4_sm2, _pwm4_sm3)) = flexpwm4;
    pwm4_sm2.set_clock_select(flexpwm::ClockSelect::Ipg);
    pwm4_sm2.set_prescaler(flexpwm::Prescaler::Prescaler1);
    pwm4_sm2.set_pair_operation(flexpwm::PairOperation::Independent);
    pwm4_sm2.set_load_mode(flexpwm::LoadMode::reload_full());
    pwm4_sm2.set_load_frequency(1);
    pwm4_sm2.set_initial_count(&pwm4, i16::MIN);
    pwm4_sm2.set_value(flexpwm::FULL_RELOAD_VALUE_REGISTER, i16::MAX);
    
    let pump1_lpwm_out = flexpwm::Output::new_b(pins.p3);
    let pump1_rpwm_out = flexpwm::Output::new_a(pins.p2);
    pump1_lpwm_out.set_turn_on(&pwm4_sm2, i16::MIN);
    pump1_rpwm_out.set_turn_on(&pwm4_sm2, i16::MIN);
    
    pwm4_sm2.set_load_ok(&mut pwm4);
    pwm4_sm2.set_running(&mut pwm4, true);

    let mut set_pump1_pwm = |permille: u16| {
        pump1_lpwm_out.set_output_enable(&mut pwm4, false);
        if permille == 0 {
            pump1_rpwm_out.set_output_enable(&mut pwm4, false);
        } else {
            pump1_rpwm_out.set_turn_off(&pwm4_sm2, pwm_permille_to_turn_off(permille));
            pwm4_sm2.set_load_ok(&mut pwm4);
            pump1_rpwm_out.set_output_enable(&mut pwm4, true);
        }
    };
    
    set_pump1_pwm(0);

    // ===================================================
    // PELTIER
"""
code = code.replace("    // ===================================================\n    // PELTIER", pump1_new)

with open("src/main.rs", "w") as f:
    f.write(code)

