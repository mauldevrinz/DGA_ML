import re

with open("src/main.rs", "r") as f:
    code = f.read()

# Replace flexpwm1, flexpwm2, with flexpwm1, flexpwm2, flexpwm4,
code = code.replace("flexpwm1,\n        flexpwm2,", "flexpwm1,\n        flexpwm2,\n        flexpwm4,")

parser = """
    pub enum HostCmd {
        Setpoint(f32),
        Phase(&'static str),
        Pump1Pwm(u16),
    }
"""

new_poll = """
    fn poll_cmd(&mut self, rx_buf: &mut [u8], rx_len: &mut usize) -> Option<HostCmd> {
        self.poll();
        let mut chunk = [0u8; 32];
        let n = self.serial.read(&mut chunk).unwrap_or(0);
        let mut result = None;
        for &b in &chunk[..n] {
            if b == b'\\n' || b == b'\\r' {
                if *rx_len > 0 {
                    if let Ok(line) = core::str::from_utf8(&rx_buf[..*rx_len]) {
                        let line = line.trim();
                        if let Some(rest) = line.strip_prefix("SETPOINT=") {
                            if let Ok(val) = rest.trim().parse::<f32>() {
                                result = Some(HostCmd::Setpoint(val));
                            }
                        } else if let Some(rest) = line.strip_prefix("PHASE=") {
                            match rest.trim() {
                                "IDLE" => result = Some(HostCmd::Phase("IDLE")),
                                "INJECT" => result = Some(HostCmd::Phase("INJECT")),
                                "PURGE" => result = Some(HostCmd::Phase("PURGE")),
                                "OFF" => result = Some(HostCmd::Phase("OFF")),
                                _ => {}
                            }
                        } else if let Some(rest) = line.strip_prefix("PWM=") {
                            if let Ok(val) = rest.trim().parse::<u16>() {
                                result = Some(HostCmd::Pump1Pwm(val));
                            }
                        }
                    }
                    *rx_len = 0;
                }
            } else if *rx_len < rx_buf.len() {
                rx_buf[*rx_len] = b;
                *rx_len += 1;
            }
        }
        result
    }
"""

code = code.replace("struct UsbComm<'a> {", parser + "\nstruct UsbComm<'a> {")

# Regex to match the whole poll_setpoint method
regex_poll_setpoint = re.compile(r'fn poll_setpoint.*?result\n    \}', re.DOTALL)
code = regex_poll_setpoint.sub(new_poll.strip(), code)

pump1_orig = """    let mut pump1_l_en = gpio4.output(pins.p5);
    let _ = pump1_l_en.set_low();
    
    let mut pump1_r_en = gpio4.output(pins.p4);
    let _ = pump1_r_en.set_low();
    
    let mut pump1_lpwm = gpio4.output(pins.p3);
    let _ = pump1_lpwm.set_low();
    
    let mut pump1_rpwm = gpio4.output(pins.p2);
    let _ = pump1_rpwm.set_low();"""

pump1_new = """    let mut pump1_l_en = gpio4.output(pins.p5);
    let _ = pump1_l_en.set_high(); // Enable bridge L
    
    let mut pump1_r_en = gpio4.output(pins.p4);
    let _ = pump1_r_en.set_high(); // Enable bridge R
    
    // We'll use PWM for pump1_rpwm (D2) which is FlexPWM4_SM2_A
    // And D3 is FlexPWM4_SM2_B
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
        // Forward only, LPWM always off, RPWM duty cycle
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
"""
code = code.replace(pump1_orig, pump1_new)

phase_vars = """
    let mut phase_mode: &str = "OFF";
    let mut pump1_pwm_val: u16 = 500; // default 50%
"""
code = code.replace("let mut peltier_mode: &str = \"OFF\";", "let mut peltier_mode: &str = \"OFF\";\n" + phase_vars)

loop_start_orig = """        for _gpio_idx in 0..6 {
            // Cek perintah setpoint suhu dari host (tombol setpoint di web app),
            // non-blocking - tidak menunda pembacaan sensor kalau tidak ada data.
            if let Some(val) =
                with_usb_comm(|comm| comm.poll_setpoint(&mut setpoint_rx_buf, &mut setpoint_rx_len))
                    .flatten()
            {
                setpoint_c = val;
                usb_println!("ACK SETPOINT = {:.2} C", setpoint_c);
            }"""
            
loop_start_new = """        for _gpio_idx in 0..6 {
            if let Some(cmd) =
                with_usb_comm(|comm| comm.poll_cmd(&mut setpoint_rx_buf, &mut setpoint_rx_len))
                    .flatten()
            {
                match cmd {
                    HostCmd::Setpoint(val) => {
                        setpoint_c = val;
                        usb_println!("ACK SETPOINT = {:.2} C", setpoint_c);
                    },
                    HostCmd::Phase(p) => {
                        phase_mode = p;
                        usb_println!("ACK PHASE = {}", p);
                    },
                    HostCmd::Pump1Pwm(val) => {
                        let val_permille = (val.min(100) as u16) * 10;
                        pump1_pwm_val = val_permille;
                        usb_println!("ACK PWM = {}%", val.min(100));
                        if phase_mode == "INJECT" {
                            set_pump1_pwm(pump1_pwm_val);
                        }
                    }
                }
            }
            
            // Handle Phases
            match phase_mode {
                "IDLE" => {
                    let _ = p14.set_high(); // Pump 2
                    let _ = p38.set_high(); // SV1
                    let _ = p39.set_high(); // SV2
                    
                    let _ = p40.set_low(); // SV3
                    let _ = p41.set_low(); // Pump 3
                    set_pump1_pwm(0);
                },
                "INJECT" => {
                    set_pump1_pwm(pump1_pwm_val); // Pump 1
                    let _ = p40.set_high(); // SV3
                    
                    let _ = p14.set_low(); // Pump 2
                    let _ = p41.set_low(); // Pump 3
                    let _ = p38.set_low(); // SV1
                    let _ = p39.set_low(); // SV2
                },
                "PURGE" => {
                    let _ = p41.set_high(); // Pump 3
                    let _ = p39.set_high(); // SV2
                    let _ = p40.set_high(); // SV3
                    
                    let _ = p38.set_low(); // SV1
                    let _ = p14.set_low(); // Pump 2
                    set_pump1_pwm(0);
                },
                _ => { // OFF
                    let _ = p14.set_low();
                    let _ = p38.set_low();
                    let _ = p39.set_low();
                    let _ = p40.set_low();
                    let _ = p41.set_low();
                    set_pump1_pwm(0);
                }
            }
"""
code = code.replace(loop_start_orig, loop_start_new)

with open("src/main.rs", "w") as f:
    f.write(code)

